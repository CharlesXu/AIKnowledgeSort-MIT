import { describe, expect, test, vi } from "vitest";
import {
  createBrowserAgentAccessClient,
  createTauriAgentAccessClient,
} from "./agentAccessClient";
import type { CreateAgentGrantRequest } from "./types";

const request: CreateAgentGrantRequest = {
  selectionId: "selection-1",
  agentId: "codex-desktop",
  label: "Codex Desktop",
  toolIds: ["capabilities.read", "graph.read"],
  expiresInSeconds: 3_600,
  limits: {
    maxRequestsPerSession: 1_000,
    maxRequestBytes: 128 * 1024,
    maxResponseBytes: 256 * 1024,
  },
};

describe("Agent access client", () => {
  test("invokes only the four explicit native Agent access boundaries", async () => {
    const invoke = vi.fn().mockResolvedValue({ schemaVersion: 1, grants: [] });
    const client = createTauriAgentAccessClient(invoke);

    await client.selectDirectories();
    await client.inspect();
    await client.createGrant(request);
    await client.revokeGrant({ grantId: "grant-1" });

    expect(invoke.mock.calls).toEqual([
      ["select_agent_grant_directories"],
      ["inspect_agent_access"],
      ["create_agent_grant", { request }],
      ["revoke_agent_grant", { request: { grantId: "grant-1" } }],
    ]);
  });

  test("never simulates Agent grants or one-time tokens in a browser", async () => {
    const client = createBrowserAgentAccessClient();
    const expected = "Desktop runtime is required for Agent access operations.";

    await expect(client.selectDirectories()).rejects.toThrow(expected);
    await expect(client.inspect()).rejects.toThrow(expected);
    await expect(client.createGrant(request)).rejects.toThrow(expected);
    await expect(client.revokeGrant({ grantId: "grant-1" })).rejects.toThrow(expected);
  });
});
