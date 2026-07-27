import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type {
  ArchiveClient,
  ArchivePlan,
  VaultSummary,
} from "../archive/types";
import type { DiscoveryProposal } from "../drop/types";
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
  planVersion: 1,
  proposalId: "proposal-1",
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
        "Originals/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/notes.md",
      byteSize: 12,
      identity: proposal.items[0].identity,
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
  };
}

describe("archive preview", () => {
  test("requires an exact reviewed plan before a source-preserving commit", async () => {
    const archiveClient = client();
    render(
      <ArchivePreviewPane archiveClient={archiveClient} proposal={proposal} />,
    );

    expect(screen.getByText("No Vault selected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Review archive plan" })).toBeDisabled();

    fireEvent.click(screen.getByRole("checkbox", { name: /notes\.md/i }));
    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));
    await screen.findByText("/Knowledge Vault");
    fireEvent.click(screen.getByRole("button", { name: "Review archive plan" }));

    const review = await screen.findByRole("region", {
      name: "Exact archive plan",
    });
    expect(review).toHaveTextContent("/inbox/notes.md");
    expect(review).toHaveTextContent(
      "Originals/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/notes.md",
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
  });

  test("surfaces native errors without claiming any change", async () => {
    const archiveClient = client();
    vi.mocked(archiveClient.chooseVault).mockRejectedValue(
      new Error("Desktop runtime is required for archive operations."),
    );
    render(
      <ArchivePreviewPane archiveClient={archiveClient} proposal={proposal} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      /desktop runtime is required/i,
    );
    expect(screen.getByText("Uncommitted")).toBeInTheDocument();
    expect(screen.queryByText("Archive committed")).toBeNull();
  });
});
