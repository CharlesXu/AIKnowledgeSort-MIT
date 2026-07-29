import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { DiscoveryProposal } from "../drop/types";
import type { GraphClient } from "../graph/types";
import type { ProfileClient } from "../profiles/types";
import type { ModelRuntimeClient } from "../models/types";
import { ContextPane } from "./ContextPane";

const proposal: DiscoveryProposal = {
  proposalId: "context-proposal",
  items: [{
    itemId: "context-guide",
    path: "/review/guide.md",
    name: "guide.md",
    byteSize: 1024,
    identity: {
      algorithm: "SHA-256",
      digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0",
    },
  }],
  counts: {
    included: 2,
    excluded: 3,
    unreadable: 1,
    symlink: 4,
    outOfScope: 5,
  },
  diagnostics: [],
};

describe("ContextPane", () => {
  test("keeps graph and governed profile review as separate right-pane tabs", async () => {
    const profileClient: ProfileClient = {
      inspect: vi.fn().mockResolvedValue({
        installed: [],
        active: null,
        candidates: [],
      }),
      importLocalCandidate: vi.fn(),
      importUrlCandidate: vi.fn(),
      compileLocalCandidate: vi.fn(),
      decideCandidate: vi.fn(),
      createClassificationBatch: vi.fn(),
    };
    const graphClient: GraphClient = {
      inspect: vi.fn(),
      propose: vi.fn(),
      decide: vi.fn(),
    };
    const modelRuntimeClient: ModelRuntimeClient = {
      inspect: vi.fn().mockResolvedValue({ schemaVersion: 1, configs: [] }),
      upsert: vi.fn(),
      remove: vi.fn(),
      runComparison: vi.fn(),
      runFileSemanticComparison: vi.fn(),
    };
    render(
      <ContextPane
        collapsed={false}
        document={null}
        graphClient={graphClient}
        modelRuntimeClient={modelRuntimeClient}
        onCollapsedChange={vi.fn()}
        profileClient={profileClient}
        proposal={proposal}
      />,
    );

    expect(screen.getByRole("tab", { name: "Knowledge Graph" }))
      .toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("tab", { name: "Import Review" }));
    expect(
      await screen.findByText(
        "Ninebot document and electronic archive classification",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("No candidate awaiting review"))
      .toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Agent Review" }));
    expect(await screen.findByText(/saved Vault revision is required/i))
      .toBeInTheDocument();
  });
});
