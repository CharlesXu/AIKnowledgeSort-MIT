import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type {
  ArchiveClient,
  ArchivePlan,
  VaultSummary,
} from "../archive/types";
import type { DiscoveryProposal } from "../drop/types";
import type {
  NamingBatch,
  NamingClient,
} from "../naming/types";
import { ArchivePreviewPane } from "./ArchivePreviewPane";

const proposal: DiscoveryProposal = {
  proposalId: "proposal-1",
  items: [
    {
      itemId: "item-1",
      path: "/inbox/notes.md",
      name: "notes.md",
      byteSize: 12,
      identity: {
        algorithm: "SHA-256",
        digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      },
    },
  ],
  counts: {
    included: 1,
    excluded: 0,
    unreadable: 0,
    symlink: 0,
    outOfScope: 0,
  },
  diagnostics: [],
};

const vault: VaultSummary = {
  authorityId: "vault-1",
  displayPath: "/Knowledge Vault",
  status: "authoritative",
};

const plan: ArchivePlan = {
  planId: "plan-1",
  planVersion: 2,
  proposalId: "proposal-1",
  namingBatchId: "naming-batch-1",
  authorityId: "vault-1",
  vaultPath: "/Knowledge Vault",
  expiresAtUnixMs: Date.now() + 300_000,
  confirmationNonce: "secret-confirmation",
  sourcePreserved: true,
  items: [
    {
      itemId: "item-1",
      sourcePath: "/inbox/notes.md",
      destinationPath:
        "Originals/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/Reset-reliability.md",
      originalName: "notes.md",
      canonicalName: "Reset-reliability.md",
      naming: {
        namingProposalId: "naming-proposal-1",
        originalName: "notes.md",
        canonicalName: "Reset-reliability.md",
        policyId: "canonical-v1",
        policyVersion: "1.0.0",
        appliedRule: "ordered-cited-facts-v1",
        facts: [
          {
            kind: "subject",
            value: "Reset reliability",
            evidenceLocation: "page:1",
          },
        ],
      },
      byteSize: 12,
      identity: proposal.items[0].identity,
    },
  ],
};

const namingBatch: NamingBatch = {
  batchId: "naming-batch-1",
  discoveryProposalId: "proposal-1",
  policyId: "canonical-v1",
  policyVersion: "1.0.0",
  expiresAtUnixMs: Date.now() + 300_000,
  proposals: [
    {
      proposalId: "naming-proposal-1",
      itemId: "item-1",
      originalName: "notes.md",
      canonicalName: "Reset-reliability.md",
      identity: proposal.items[0].identity,
      policyId: "canonical-v1",
      policyVersion: "1.0.0",
      appliedRule: "ordered-cited-facts-v1",
      status: "proposed",
      reviewReason: null,
      facts: [
        {
          kind: "subject",
          value: "Reset reliability",
          evidenceLocation: "page:1",
        },
      ],
    },
  ],
};

function client(): ArchiveClient {
  return {
    chooseVault: vi.fn().mockResolvedValue(vault),
    createPlan: vi.fn().mockResolvedValue(plan),
    confirmPlan: vi.fn().mockResolvedValue({
      planId: "plan-1",
      status: "committed",
      items: [
        {
          operationId: "operation-1",
          itemId: "item-1",
          destinationPath: plan.items[0].destinationPath,
          identity: plan.items[0].identity,
          status: "committed",
          failureReason: null,
        },
      ],
    }),
    createCleanupPlan: vi.fn().mockResolvedValue({
      planId: "cleanup-plan-1",
      planVersion: 1,
      authorityId: "vault-1",
      disposition: "trash",
      items: [
        {
          operationId: "operation-1",
          sourcePath: "/inbox/notes.md",
          retainedPath:
            "/Knowledge Vault/Originals/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/Reset-reliability.md",
          identity: proposal.items[0].identity,
        },
      ],
      expiresAtUnixMs: Date.now() + 300_000,
      confirmationNonce: "cleanup-confirmation",
      confirmationBindingSha256: "b".repeat(64),
    }),
    authorizePermanentCleanup: vi.fn(),
    confirmCleanupPlan: vi.fn().mockResolvedValue({
      planId: "cleanup-plan-1",
      status: "committed",
      disposition: "trash",
      removedPaths: ["/inbox/notes.md"],
      failureReason: null,
    }),
  };
}

function namingClient(batch: NamingBatch = namingBatch): NamingClient {
  return {
    createBatch: vi.fn().mockResolvedValue(batch),
  };
}

describe("archive preview", () => {
  test("requires an exact reviewed plan before a source-preserving commit", async () => {
    const archiveClient = client();
    const names = namingClient();
    const onCommittedItems = vi.fn();
    render(
      <ArchivePreviewPane
        archiveClient={archiveClient}
        namingClient={names}
        onCommittedItems={onCommittedItems}
        proposal={proposal}
      />,
    );

    expect(screen.getByText("No Vault selected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Review archive plan" })).toBeDisabled();

    fireEvent.click(screen.getByRole("checkbox", { name: /notes\.md/i }));
    expect(
      screen.getByRole("textbox", { name: "Project for notes.md" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("textbox", { name: "Model for notes.md" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("textbox", { name: "Regulation for notes.md" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("textbox", { name: "Version for notes.md" }),
    ).toBeInTheDocument();
    fireEvent.change(
      screen.getByRole("textbox", { name: "Subject for notes.md" }),
      { target: { value: "Reset reliability" } },
    );
    expect(
      screen.getByRole("button", { name: "Review canonical names" }),
    ).toBeDisabled();
    fireEvent.change(
      screen.getByRole("textbox", {
        name: "Evidence location for notes.md",
      }),
      { target: { value: "page:1" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));
    await screen.findByText("/Knowledge Vault");
    fireEvent.click(
      screen.getByRole("button", { name: "Review canonical names" }),
    );
    const namingReview = await screen.findByRole("region", {
      name: "Canonical name review",
    });
    expect(namingReview).toHaveTextContent("notes.md → Reset-reliability.md");
    expect(namingReview).toHaveTextContent("canonical-v1 · 1.0.0");
    expect(namingReview).toHaveTextContent(
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    fireEvent.click(screen.getByRole("button", { name: "Review archive plan" }));

    await waitFor(() =>
      expect(archiveClient.createPlan).toHaveBeenCalledWith({
        proposalId: "proposal-1",
        itemIds: ["item-1"],
        namingBatchId: "naming-batch-1",
      }),
    );
    const review = await screen.findByRole("region", {
      name: "Exact archive plan",
    });
    expect(review).toHaveTextContent("/inbox/notes.md");
    expect(review).toHaveTextContent(
      "Originals/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/Reset-reliability.md",
    );
    expect(review).toHaveTextContent(/SHA-256/i);
    expect(review).toHaveTextContent(/source file remains/i);
    expect(review).not.toHaveTextContent("secret-confirmation");
    expect(
      screen.getByRole("button", { name: "Confirm verified archive" }),
    ).toBeDisabled();

    fireEvent.click(
      screen.getByRole("checkbox", {
        name: /i reviewed every source, destination, and sha-256/i,
      }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Confirm verified archive" }),
    );

    await waitFor(() =>
      expect(archiveClient.confirmPlan).toHaveBeenCalledWith({
        planId: "plan-1",
        confirmationNonce: "secret-confirmation",
      }),
    );
    expect(screen.getByText("Archive committed")).toBeInTheDocument();
    expect(
      within(screen.getByRole("region", { name: "Archive result" })).getByText(
        /source preserved/i,
      ),
    ).toBeInTheDocument();
    expect(onCommittedItems).toHaveBeenCalledWith(
      [expect.objectContaining({
        operationId: "operation-1",
        status: "committed",
      })],
      vault,
    );

    expect(
      screen.getByRole("button", { name: "Review source cleanup" }),
    ).toBeDisabled();
    fireEvent.click(
      screen.getByRole("checkbox", { name: /enable cleanup for these archived sources/i }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Review source cleanup" }),
    );
    const cleanupReview = await screen.findByRole("region", {
      name: "Exact cleanup plan",
    });
    expect(cleanupReview).toHaveTextContent("/inbox/notes.md");
    expect(cleanupReview).toHaveTextContent(/operating-system trash/i);
    expect(cleanupReview).toHaveTextContent(/retained original/i);
    expect(
      screen.getByRole("button", { name: "Confirm move to trash" }),
    ).toBeDisabled();
    fireEvent.click(
      screen.getByRole("checkbox", { name: /i reviewed every cleanup path and sha-256/i }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Confirm move to trash" }),
    );
    await waitFor(() =>
      expect(archiveClient.confirmCleanupPlan).toHaveBeenCalledWith({
        planId: "cleanup-plan-1",
        confirmationNonce: "cleanup-confirmation",
      }),
    );
    expect(screen.getByText("Source cleanup committed")).toBeInTheDocument();
  });

  test("blocks archive planning when canonical naming needs review", async () => {
    const reviewBatch: NamingBatch = {
      ...namingBatch,
      proposals: [
        {
          ...namingBatch.proposals[0],
          canonicalName: null,
          status: "namingReview",
          reviewReason: "conflictingEvidence",
        },
      ],
    };
    const archiveClient = client();
    render(
      <ArchivePreviewPane
        archiveClient={archiveClient}
        namingClient={namingClient(reviewBatch)}
        proposal={proposal}
      />,
    );

    fireEvent.click(screen.getByRole("checkbox", { name: /notes\.md/i }));
    fireEvent.change(
      screen.getByRole("textbox", { name: "Subject for notes.md" }),
      { target: { value: "Reset reliability" } },
    );
    fireEvent.change(
      screen.getByRole("textbox", {
        name: "Evidence location for notes.md",
      }),
      { target: { value: "page:1" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));
    await screen.findByText("/Knowledge Vault");
    fireEvent.click(
      screen.getByRole("button", { name: "Review canonical names" }),
    );

    expect(await screen.findByText("Conflicting evidence")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Review archive plan" }),
    ).toBeDisabled();
    expect(archiveClient.createPlan).not.toHaveBeenCalled();
  });

  test("surfaces native errors without claiming any change", async () => {
    const archiveClient = client();
    vi.mocked(archiveClient.chooseVault).mockRejectedValue(
      new Error("Desktop runtime is required for archive operations."),
    );
    render(
      <ArchivePreviewPane
        archiveClient={archiveClient}
        namingClient={namingClient()}
        proposal={proposal}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      /desktop runtime is required/i,
    );
    expect(screen.getByText("Uncommitted")).toBeInTheDocument();
    expect(screen.queryByText("Archive committed")).toBeNull();
  });
});
