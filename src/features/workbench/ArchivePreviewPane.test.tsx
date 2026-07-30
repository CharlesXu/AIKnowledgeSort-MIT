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
import type {
  ClassificationBatch,
  ProfileClient,
} from "../profiles/types";
import type {
  FileSemanticComparison,
  ModelRuntimeClient,
} from "../models/types";
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
  planVersion: 3,
  proposalId: "proposal-1",
  namingBatchId: "naming-batch-1",
  classificationBatchId: "classification-batch-1",
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
      classification: {
        proposalId: "classification-proposal-1",
        sourceIdentity: proposal.items[0].identity,
        profileId: "ninebot",
        profileVersion: "1.0.0",
        status: "proposed",
        ruleIds: ["semiconductor-reliability"],
        evidence: [{ kind: "documentText", location: "page:1" }],
        destination: ["Research", "Semiconductors", "Reliability"],
        reviewReason: null,
        committable: true,
      },
      byteSize: 12,
      identity: proposal.items[0].identity,
    },
  ],
};

const classificationBatch: ClassificationBatch = {
  batchId: "classification-batch-1",
  discoveryProposalId: "proposal-1",
  profileId: "ninebot",
  profileVersion: "1.0.0",
  expiresAtUnixMs: Date.now() + 300_000,
  items: [
    {
      itemId: "item-1",
      proposal: plan.items[0].classification!,
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
    createArchiveUndoPlan: vi.fn().mockResolvedValue({
      undoId: "undo-1",
      planVersion: 1,
      operationId: "operation-1",
      authorityId: "vault-1",
      sourcePath: "/inbox/notes.md",
      archivedPath:
        "/Knowledge Vault/Originals/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/Reset-reliability.md",
      archivedRelativePath:
        "Originals/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/Reset-reliability.md",
      byteSize: 12,
      identity: proposal.items[0].identity,
      expiresAtUnixMs: Date.now() + 300_000,
      confirmationNonce: "undo-confirmation",
      confirmationBindingSha256: "c".repeat(64),
    }),
    confirmArchiveUndoPlan: vi.fn().mockResolvedValue({
      undoId: "undo-1",
      operationId: "operation-1",
      status: "committed",
      failureReason: null,
    }),
  };
}

function namingClient(batch: NamingBatch = namingBatch): NamingClient {
  return {
    createBatch: vi.fn().mockResolvedValue(batch),
  };
}

function profileClient(
  batch: ClassificationBatch = classificationBatch,
): ProfileClient {
  return {
    inspect: vi.fn(),
    importLocalCandidate: vi.fn(),
    importUrlCandidate: vi.fn(),
    compileLocalCandidate: vi.fn(),
    decideCandidate: vi.fn(),
    createClassificationBatch: vi.fn().mockResolvedValue(batch),
  };
}

function modelRuntimeClient(): ModelRuntimeClient {
  const comparison = {
    schemaVersion: 1,
    comparisonId: "comparison-1",
    envelope: {
      itemId: "item-1",
      originalName: "notes.md",
      profile: {
        profileId: "ninebot",
        version: "0.9.0",
        categories: [
          {
            categoryId: "research.reliability",
            label: "Reliability",
            depth: 2,
            parentId: "research",
            path: ["1-Research", "Reliability"],
            aliases: [],
          },
        ],
      },
      evidence: {
        excerpts: [
          {
            evidenceId: "evidence-1",
            location: "line:1-3",
            text: "MCU reset reliability validation",
          },
        ],
      },
    },
    desktopConfigId: "desktop",
    agentConfigId: "agent",
    desktopOutcome: {
      status: "succeeded",
      model: "desktop-model",
      suggestion: {
        summary: "Reliability research",
        categoryId: "research.reliability",
        categoryEvidenceIds: ["evidence-1"],
        namingFacts: [
          {
            kind: "subject",
            value: "MCU reset reliability",
            evidenceIds: ["evidence-1"],
          },
        ],
        uncertaintyReason: null,
      },
      failureReason: null,
    },
    agentOutcome: {
      status: "succeeded",
      model: "agent-model",
      suggestion: {
        summary: "Reliability research",
        categoryId: "research.reliability",
        categoryEvidenceIds: ["evidence-1"],
        namingFacts: [],
        uncertaintyReason: null,
      },
      failureReason: null,
    },
    adjudication: {
      decision: "accept",
      reason: "The desktop result preserves the supported subject fact.",
      evidenceIds: ["evidence-1"],
      selectedSide: "desktop",
      revisedSuggestion: null,
    },
    adjudicationFailure: null,
    resolvedSuggestion: {
      summary: "Reliability research",
      categoryId: "research.reliability",
      categoryEvidenceIds: ["evidence-1"],
      namingFacts: [
        {
          kind: "subject",
          value: "MCU reset reliability",
          evidenceIds: ["evidence-1"],
        },
      ],
      uncertaintyReason: null,
    },
    status: "completed",
  } as unknown as FileSemanticComparison;
  return {
    inspect: vi.fn().mockResolvedValue({
      schemaVersion: 1,
      configs: [
        {
          configId: "desktop",
          label: "Desktop",
          location: "local",
          endpointUrl: "http://127.0.0.1/v1/chat/completions",
          model: "desktop-model",
          timeoutMs: 30_000,
          authenticated: false,
          credentialEnvironment: null,
        },
        {
          configId: "agent",
          label: "Agent",
          location: "remote",
          endpointUrl: "https://example.test/v1/chat/completions",
          model: "agent-model",
          timeoutMs: 30_000,
          authenticated: true,
          credentialEnvironment: "AIKS_MODEL_API_KEY_AGENT",
        },
      ],
    }),
    upsert: vi.fn(),
    remove: vi.fn(),
    runComparison: vi.fn(),
    runFileSemanticComparison: vi.fn().mockResolvedValue(comparison),
  };
}

describe("archive preview", () => {
  test("applies only an Agent-resolved semantic suggestion to the editable review form", async () => {
    const models = modelRuntimeClient();
    const archiveClient = client();
    const profiles = profileClient();
    render(
      <ArchivePreviewPane
        archiveClient={archiveClient}
        modelRuntimeClient={models}
        namingClient={namingClient()}
        profileClient={profiles}
        proposal={proposal}
      />,
    );

    fireEvent.click(screen.getByRole("checkbox", { name: /notes\.md/i }));
    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));
    await screen.findByText("/Knowledge Vault");
    expect(
      screen.getByRole("textbox", { name: "Subject for notes.md" }),
    ).toHaveValue("");
    fireEvent.click(
      await screen.findByRole("button", { name: "Compare notes.md with two models" }),
    );

    expect(await screen.findByText("Agent accepted Desktop")).toBeInTheDocument();
    expect(screen.getByText("1-Research / Reliability")).toBeInTheDocument();
    expect(
      screen.getByRole("textbox", { name: "Subject for notes.md" }),
    ).toHaveValue("");

    fireEvent.click(
      screen.getByRole("button", { name: "Apply reviewed suggestion for notes.md" }),
    );

    expect(
      screen.getByRole("textbox", { name: "Subject for notes.md" }),
    ).toHaveValue("MCU reset reliability");
    expect(
      screen.getByRole("textbox", { name: "Classification evidence for notes.md" }),
    ).toHaveValue("MCU reset reliability validation");
    fireEvent.click(screen.getByRole("button", { name: "Review classification" }));
    await screen.findByRole("region", { name: "Classification review" });
    expect(profiles.createClassificationBatch).toHaveBeenCalledWith({
      proposalId: "proposal-1",
      items: [
        {
          itemId: "item-1",
          references: [],
          semanticComparisonId: "comparison-1",
        },
      ],
    });
    expect(archiveClient.createPlan).not.toHaveBeenCalled();
    expect(archiveClient.confirmPlan).not.toHaveBeenCalled();
  });

  test("requires an exact reviewed plan before a source-preserving commit", async () => {
    const archiveClient = client();
    const names = namingClient();
    const onCommittedItems = vi.fn();
    render(
      <ArchivePreviewPane
        archiveClient={archiveClient}
        namingClient={names}
        profileClient={profileClient()}
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
    fireEvent.change(
      screen.getByRole("textbox", {
        name: "Classification evidence for notes.md",
      }),
      { target: { value: "MCU semiconductor reset reliability" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));
    await screen.findByText("/Knowledge Vault");
    fireEvent.click(
      screen.getByRole("button", { name: "Review classification" }),
    );
    const classificationReview = await screen.findByRole("region", {
      name: "Classification review",
    });
    expect(classificationReview).toHaveTextContent(
      "Research / Semiconductors / Reliability",
    );
    expect(classificationReview).toHaveTextContent("ninebot · 1.0.0");
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
        classificationBatchId: "classification-batch-1",
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

  test("requires a separately reviewed bounded plan before archive undo", async () => {
    const archiveClient = client();
    const onUndoneOperation = vi.fn();
    render(
      <ArchivePreviewPane
        archiveClient={archiveClient}
        namingClient={namingClient()}
        profileClient={profileClient()}
        onUndoneOperation={onUndoneOperation}
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
    fireEvent.change(
      screen.getByRole("textbox", {
        name: "Classification evidence for notes.md",
      }),
      { target: { value: "MCU semiconductor reset reliability" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));
    await screen.findByText("/Knowledge Vault");
    fireEvent.click(
      screen.getByRole("button", { name: "Review classification" }),
    );
    await screen.findByRole("region", { name: "Classification review" });
    fireEvent.click(
      screen.getByRole("button", { name: "Review canonical names" }),
    );
    await screen.findByRole("region", { name: "Canonical name review" });
    fireEvent.click(screen.getByRole("button", { name: "Review archive plan" }));
    await screen.findByRole("region", { name: "Exact archive plan" });
    fireEvent.click(
      screen.getByRole("checkbox", {
        name: /i reviewed every source, destination, and sha-256/i,
      }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Confirm verified archive" }),
    );
    await screen.findByText("Archive committed");

    fireEvent.click(
      screen.getByRole("button", {
        name: /review archive undo.*reset-reliability\.md/i,
      }),
    );
    const review = await screen.findByRole("region", {
      name: "Exact archive undo plan",
    });
    expect(review).toHaveTextContent("/inbox/notes.md");
    expect(review).toHaveTextContent("/Knowledge Vault/Originals/");
    expect(review).toHaveTextContent(/transaction staging/i);
    expect(
      screen.getByRole("button", { name: "Confirm archive undo" }),
    ).toBeDisabled();
    fireEvent.click(
      screen.getByRole("checkbox", {
        name: /i reviewed the source, archive path, and sha-256/i,
      }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Confirm archive undo" }),
    );

    await waitFor(() =>
      expect(archiveClient.confirmArchiveUndoPlan).toHaveBeenCalledWith({
        undoId: "undo-1",
        confirmationNonce: "undo-confirmation",
      }),
    );
    expect(screen.getByText("Archive undo committed")).toBeInTheDocument();
    expect(onUndoneOperation).toHaveBeenCalledWith("operation-1");
    expect(
      screen.queryByRole("region", { name: "Source cleanup" }),
    ).toBeNull();
  });

  test("blocks naming and archive planning when classification needs review", async () => {
    const reviewBatch: ClassificationBatch = {
      ...classificationBatch,
      items: [
        {
          ...classificationBatch.items[0],
          proposal: {
            ...classificationBatch.items[0].proposal,
            status: "classificationReview",
            destination: null,
            reviewReason: "missingEvidence",
            committable: false,
          },
        },
      ],
    };
    const archiveClient = client();
    render(
      <ArchivePreviewPane
        archiveClient={archiveClient}
        namingClient={namingClient()}
        profileClient={profileClient(reviewBatch)}
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
        name: "Classification evidence for notes.md",
      }),
      { target: { value: "Unmatched semantic evidence" } },
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
      screen.getByRole("button", { name: "Review classification" }),
    );

    expect(
      await screen.findByText("Missing semantic evidence"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Review canonical names" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Review archive plan" }),
    ).toBeDisabled();
    expect(archiveClient.createPlan).not.toHaveBeenCalled();
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
        profileClient={profileClient()}
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
    fireEvent.change(
      screen.getByRole("textbox", {
        name: "Classification evidence for notes.md",
      }),
      { target: { value: "MCU semiconductor reset reliability" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));
    await screen.findByText("/Knowledge Vault");
    fireEvent.click(
      screen.getByRole("button", { name: "Review classification" }),
    );
    await screen.findByRole("region", { name: "Classification review" });
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
        profileClient={profileClient()}
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

  test("notifies the workbench after the authoritative Vault is selected", async () => {
    const onVaultSelected = vi.fn().mockResolvedValue(undefined);

    render(
      <ArchivePreviewPane
        archiveClient={client()}
        namingClient={namingClient()}
        onVaultSelected={onVaultSelected}
        profileClient={profileClient()}
        proposal={proposal}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Choose Vault" }));

    await waitFor(() => expect(onVaultSelected).toHaveBeenCalledWith(vault));
  });
});
