import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { AgentAccessPanel } from "./AgentAccessPanel";
import type {
  AgentAccessClient,
  AgentAccessState,
  AgentGrantSummary,
} from "./types";

const scope = { scopeId: "scope-1", displayPath: "/Users/charles/Documents" };
const grant: AgentGrantSummary = {
  grantId: "grant-1",
  agentId: "codex-desktop",
  label: "Codex Desktop",
  toolIds: ["capabilities.read"],
  scopes: [scope],
  createdAtUnixMs: 2_000_000_000_000,
  expiresAtUnixMs: 2_000_003_600_000,
  revokedAtUnixMs: null,
  status: "active",
  limits: {
    maxRequestsPerSession: 1_000,
    maxRequestBytes: 128 * 1024,
    maxResponseBytes: 256 * 1024,
  },
};
const state: AgentAccessState = {
  schemaVersion: 1,
  toolCatalogVersion: "agent-tools-v1",
  tools: [
    { toolId: "capabilities.read", title: "Read capability catalog", effect: "read" },
    { toolId: "knowledge.read", title: "Read authoritative knowledge", effect: "read" },
    { toolId: "graph.read", title: "Read evidence graph", effect: "read" },
    { toolId: "comparison.run", title: "Run semantic comparison", effect: "semanticAdvice" },
    { toolId: "classification.propose", title: "Propose classification", effect: "semanticAdvice" },
    { toolId: "cleanup.suggest", title: "Suggest duplicate cleanup", effect: "semanticAdvice" },
  ],
  grants: [],
};

function client(): AgentAccessClient {
  return {
    selectDirectories: vi.fn().mockResolvedValue({ selectionId: "selection-1", scopes: [scope] }),
    inspect: vi.fn().mockResolvedValue(state),
    createGrant: vi.fn().mockResolvedValue({ grant, grantToken: "a".repeat(64) }),
    revokeGrant: vi.fn().mockResolvedValue({ ...state, grants: [{ ...grant, status: "revoked" }] }),
  };
}

describe("AgentAccessPanel", () => {
  test("creates a bounded grant from an opaque native selection and shows its token once", async () => {
    const agentClient = client();
    render(<AgentAccessPanel client={agentClient} />);

    expect(await screen.findByText("Read capability catalog")).toBeInTheDocument();
    expect(screen.getByText("Suggest duplicate cleanup")).toBeInTheDocument();
    expect(screen.queryByText(/cleanup execution|delete|move|rename|archive commit/i)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Choose directories" }));
    expect(await screen.findByText(scope.displayPath)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Agent ID"), {
      target: { value: "codex-desktop" },
    });
    fireEvent.change(screen.getByLabelText("Grant label"), {
      target: { value: "Codex Desktop" },
    });
    fireEvent.click(screen.getByLabelText("Read capability catalog"));
    fireEvent.click(screen.getByRole("button", { name: "Issue Agent grant" }));

    await waitFor(() => expect(agentClient.createGrant).toHaveBeenCalledWith({
      selectionId: "selection-1",
      agentId: "codex-desktop",
      label: "Codex Desktop",
      toolIds: ["capabilities.read"],
      expiresInSeconds: 3_600,
      limits: {
        maxRequestsPerSession: 1_000,
        maxRequestBytes: 128 * 1024,
        maxResponseBytes: 256 * 1024,
      },
    }));
    expect(screen.getByRole("status")).toHaveTextContent("a".repeat(64));
    expect(screen.getByRole("status")).toHaveTextContent(/cannot be recovered/i);
    fireEvent.click(screen.getByRole("button", { name: "Dismiss token" }));
    expect(screen.queryByText("a".repeat(64))).toBeNull();
  });

  test("shows lifecycle and revokes the exact grant", async () => {
    const agentClient = client();
    agentClient.inspect = vi.fn().mockResolvedValue({ ...state, grants: [grant] });
    render(<AgentAccessPanel client={agentClient} />);
    expect(await screen.findByText("ACTIVE")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Revoke Codex Desktop" }));
    await waitFor(() => expect(agentClient.revokeGrant).toHaveBeenCalledWith({ grantId: "grant-1" }));
    expect(screen.getByText("REVOKED")).toBeInTheDocument();
  });

  test("keeps browser failure visible and fabricates no grant", async () => {
    const agentClient = client();
    agentClient.inspect = vi.fn().mockRejectedValue(
      new Error("Desktop runtime is required for Agent access operations."),
    );
    render(<AgentAccessPanel client={agentClient} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Desktop runtime is required");
    expect(screen.queryByText("ACTIVE")).toBeNull();
    expect(screen.queryByText(/cannot be recovered/i)).toBeNull();
  });
});
