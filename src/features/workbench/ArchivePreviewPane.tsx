import { useEffect, useRef, useState } from "react";
import type {
  ArchiveClient,
  ArchiveCommitResult,
  ArchiveItemResult,
  ArchivePlan,
  ArchiveUndoPlan,
  ArchiveUndoResult,
  CleanupPlan,
  CleanupResult,
  VaultSummary,
} from "../archive/types";
import type { DiscoveryProposal } from "../drop/types";
import type {
  NamingBatch,
  NamingClient,
  NamingFact,
  NamingFactKind,
  NamingReviewReason,
} from "../naming/types";
import type {
  ClassificationBatch,
  ProfileClient,
} from "../profiles/types";
import type {
  FileSemanticComparison,
  ModelRuntimeClient,
} from "../models/types";
import { Icon } from "../../ui/Icon";
import { FileSemanticReview } from "./FileSemanticReview";

interface ArchivePreviewPaneProps {
  readonly archiveClient: ArchiveClient;
  readonly modelRuntimeClient?: ModelRuntimeClient;
  readonly namingClient: NamingClient;
  readonly profileClient: ProfileClient;
  readonly onCommittedItems?: (
    items: readonly ArchiveItemResult[],
    vault: VaultSummary,
  ) => void;
  readonly onUndoneOperation?: (operationId: string) => void;
  readonly proposal: DiscoveryProposal;
}

type PendingAction =
  | "vault"
  | "classification"
  | "naming"
  | "plan"
  | "commit"
  | "cleanupPlan"
  | "permanentCleanup"
  | "cleanupCommit"
  | "undoPlan"
  | "undoCommit"
  | null;

interface EvidenceDraft {
  readonly project: string;
  readonly model: string;
  readonly regulation: string;
  readonly version: string;
  readonly subject: string;
  readonly classificationText: string;
  readonly evidenceLocation: string;
}

const emptyEvidence: EvidenceDraft = {
  project: "",
  model: "",
  regulation: "",
  version: "",
  subject: "",
  classificationText: "",
  evidenceLocation: "",
};

const factFields: readonly {
  readonly kind: NamingFactKind;
  readonly label: string;
}[] = [
  { kind: "project", label: "Project" },
  { kind: "model", label: "Model" },
  { kind: "regulation", label: "Regulation" },
  { kind: "version", label: "Version" },
  { kind: "subject", label: "Subject" },
];

const reviewReasonLabels: Record<NamingReviewReason, string> = {
  missingEvidence: "Missing evidence",
  conflictingEvidence: "Conflicting evidence",
  unsafeName: "Unsafe canonical name",
  collision: "Unresolved name collision",
};

const classificationReviewReasonLabels = {
  missingEvidence: "Missing semantic evidence",
  conflictingRules: "Conflicting classification rules",
} as const;

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function ArchivePreviewPane({
  archiveClient,
  modelRuntimeClient,
  namingClient,
  profileClient,
  onCommittedItems,
  onUndoneOperation,
  proposal,
}: ArchivePreviewPaneProps) {
  const proposalId = useRef(proposal.proposalId);
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [vault, setVault] = useState<VaultSummary | null>(null);
  const [evidence, setEvidence] = useState<
    Readonly<Record<string, EvidenceDraft>>
  >({});
  const [namingBatch, setNamingBatch] = useState<NamingBatch | null>(null);
  const [classificationBatch, setClassificationBatch] =
    useState<ClassificationBatch | null>(null);
  const [plan, setPlan] = useState<ArchivePlan | null>(null);
  const [result, setResult] = useState<ArchiveCommitResult | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [pending, setPending] = useState<PendingAction>(null);
  const [error, setError] = useState<string | null>(null);
  const [cleanupEnabled, setCleanupEnabled] = useState(false);
  const [cleanupPlan, setCleanupPlan] = useState<CleanupPlan | null>(null);
  const [cleanupConfirmed, setCleanupConfirmed] = useState(false);
  const [cleanupResult, setCleanupResult] = useState<CleanupResult | null>(null);
  const [undoPlan, setUndoPlan] = useState<ArchiveUndoPlan | null>(null);
  const [undoConfirmed, setUndoConfirmed] = useState(false);
  const [undoResult, setUndoResult] = useState<ArchiveUndoResult | null>(null);
  const [undoneOperationIds, setUndoneOperationIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [semanticResults, setSemanticResults] = useState<
    Readonly<Record<string, FileSemanticComparison>>
  >({});
  const [semanticAdoptions, setSemanticAdoptions] = useState<
    Readonly<Record<string, string>>
  >({});

  useEffect(() => {
    proposalId.current = proposal.proposalId;
    setSelectedIds(new Set());
    setEvidence({});
    setNamingBatch(null);
    setClassificationBatch(null);
    setPlan(null);
    setResult(null);
    setConfirmed(false);
    setPending(null);
    setError(null);
    setCleanupEnabled(false);
    setCleanupPlan(null);
    setCleanupConfirmed(false);
    setCleanupResult(null);
    setUndoPlan(null);
    setUndoConfirmed(false);
    setUndoResult(null);
    setUndoneOperationIds(new Set());
    setSemanticResults({});
    setSemanticAdoptions({});
  }, [proposal.proposalId]);

  function invalidatePlan(): void {
    setPlan(null);
    setResult(null);
    setConfirmed(false);
    setError(null);
  }

  function invalidateNaming(): void {
    setNamingBatch(null);
    invalidatePlan();
  }

  function invalidateClassification(): void {
    setClassificationBatch(null);
    invalidateNaming();
  }

  function toggleItem(itemId: string): void {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(itemId)) {
        next.delete(itemId);
      } else {
        next.add(itemId);
      }
      return next;
    });
    setEvidence((current) =>
      current[itemId] === undefined
        ? { ...current, [itemId]: emptyEvidence }
        : current,
    );
    invalidateClassification();
  }

  function updateEvidence(
    itemId: string,
    field: keyof EvidenceDraft,
    value: string,
  ): void {
    setEvidence((current) => ({
      ...current,
      [itemId]: {
        ...(current[itemId] ?? emptyEvidence),
        [field]: value,
      },
    }));
    setSemanticAdoptions((current) => {
      const { [itemId]: _removed, ...remaining } = current;
      return remaining;
    });
    invalidateClassification();
  }

  function applySemanticSuggestion(
    itemId: string,
    comparison: FileSemanticComparison,
  ): void {
    const suggestion = comparison.resolvedSuggestion;
    if (suggestion === null || suggestion.categoryId === null) {
      return;
    }
    const factValues = new Map<string, string>();
    for (const fact of suggestion.namingFacts) {
      if (factValues.has(fact.kind)) {
        return;
      }
      factValues.set(fact.kind, fact.value);
    }
    const cited = new Set([
      ...suggestion.categoryEvidenceIds,
      ...suggestion.namingFacts.flatMap((fact) => fact.evidenceIds),
    ]);
    const excerpts = comparison.envelope.evidence.excerpts.filter((excerpt) =>
      cited.has(excerpt.evidenceId)
    );
    if (excerpts.length === 0) {
      return;
    }
    setEvidence((current) => {
      const draft = current[itemId] ?? emptyEvidence;
      return {
        ...current,
        [itemId]: {
          ...draft,
          project: factValues.get("project") ?? draft.project,
          model: factValues.get("model") ?? draft.model,
          regulation: factValues.get("regulation") ?? draft.regulation,
          version: factValues.get("version") ?? draft.version,
          subject: factValues.get("subject") ?? draft.subject,
          classificationText: excerpts.map((excerpt) => excerpt.text).join("\n\n"),
          evidenceLocation: excerpts.map((excerpt) => excerpt.location).join(", "),
        },
      };
    });
    setSemanticAdoptions((current) => ({
      ...current,
      [itemId]: comparison.comparisonId,
    }));
    invalidateClassification();
  }

  async function chooseVault(): Promise<void> {
    const activeProposal = proposal.proposalId;
    setPending("vault");
    setError(null);
    try {
      const selected = await archiveClient.chooseVault();
      if (proposalId.current !== activeProposal || selected === null) {
        return;
      }
      setVault(selected);
      invalidateClassification();
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function reviewClassification(): Promise<void> {
    if (!evidenceComplete) {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("classification");
    setError(null);
    setClassificationBatch(null);
    setNamingBatch(null);
    setPlan(null);
    setResult(null);
    try {
      const useSemanticDecisions = selectedItems.every((item) => {
        const comparison = semanticResults[item.itemId];
        const suggestion = comparison?.resolvedSuggestion;
        return (
          suggestion !== null &&
          suggestion !== undefined &&
          suggestion.categoryId !== null &&
          semanticAdoptions[item.itemId] === comparison.comparisonId
        );
      });
      const batch = await profileClient.createClassificationBatch({
        proposalId: proposal.proposalId,
        items: selectedItems.map((item) => {
          const draft = evidence[item.itemId] ?? emptyEvidence;
          const semantic = semanticResults[item.itemId];
          if (useSemanticDecisions && semantic !== undefined) {
            return {
              itemId: item.itemId,
              references: [],
              semanticComparisonId: semantic.comparisonId,
            };
          }
          return {
            itemId: item.itemId,
            references: [
              {
                kind: "documentText",
                location: draft.evidenceLocation.trim(),
                text: draft.classificationText.trim(),
              },
            ],
            semanticComparisonId: null,
          };
        }),
      });
      if (proposalId.current === activeProposal) {
        setClassificationBatch(batch);
        setConfirmed(false);
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function reviewNames(): Promise<void> {
    if (!evidenceComplete || classificationNeedsReview) {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("naming");
    setError(null);
    setResult(null);
    setPlan(null);
    try {
      const batch = await namingClient.createBatch({
        proposalId: proposal.proposalId,
        items: selectedItems.map((item) => {
          const draft = evidence[item.itemId] ?? emptyEvidence;
          const facts = factFields.flatMap<NamingFact>(({ kind }) => {
            const value = draft[kind].trim();
            return value.length === 0
              ? []
              : [
                  {
                    kind,
                    value,
                    evidenceLocation: draft.evidenceLocation.trim(),
                  },
                ];
          });
          return { itemId: item.itemId, facts };
        }),
      });
      if (proposalId.current === activeProposal) {
        setNamingBatch(batch);
        setConfirmed(false);
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function reviewPlan(): Promise<void> {
    if (
      vault === null ||
      classificationBatch === null ||
      classificationNeedsReview ||
      namingBatch === null ||
      namingBatch.proposals.some((item) => item.status !== "proposed")
    ) {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("plan");
    setError(null);
    setResult(null);
    try {
      const reviewed = await archiveClient.createPlan({
        proposalId: proposal.proposalId,
        itemIds: [...selectedIds].sort(),
        classificationBatchId: classificationBatch.batchId,
        namingBatchId: namingBatch.batchId,
      });
      if (proposalId.current === activeProposal) {
        setPlan(reviewed);
        setConfirmed(false);
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setClassificationBatch(null);
        setNamingBatch(null);
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function confirmPlan(): Promise<void> {
    if (plan === null || !confirmed) {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("commit");
    setError(null);
    try {
      const committed = await archiveClient.confirmPlan({
        planId: plan.planId,
        confirmationNonce: plan.confirmationNonce,
      });
      if (proposalId.current === activeProposal) {
        setResult(committed);
        if (vault !== null) {
          const eligible = committed.items.filter(
            (item) => item.status === "committed",
          );
          if (eligible.length > 0) {
            onCommittedItems?.(eligible, vault);
          }
        }
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function reviewCleanup(): Promise<void> {
    if (!cleanupEnabled || vault === null || result === null) {
      return;
    }
    const operationIds = result.items
      .filter(
        (item) =>
          item.status === "committed" &&
          !undoneOperationIds.has(item.operationId),
      )
      .map((item) => item.operationId);
    if (operationIds.length === 0) {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("cleanupPlan");
    setError(null);
    setCleanupResult(null);
    try {
      const reviewed = await archiveClient.createCleanupPlan({
        authorityId: vault.authorityId,
        operationIds,
        cleanupEnabled: true,
      });
      if (proposalId.current === activeProposal) {
        setCleanupPlan(reviewed);
        setCleanupConfirmed(false);
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function requestPermanentCleanup(): Promise<void> {
    if (cleanupPlan === null || cleanupPlan.disposition !== "trash") {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("permanentCleanup");
    setError(null);
    try {
      const permanent = await archiveClient.authorizePermanentCleanup({
        planId: cleanupPlan.planId,
        confirmationNonce: cleanupPlan.confirmationNonce,
      });
      if (proposalId.current === activeProposal) {
        setCleanupPlan(permanent);
        setCleanupConfirmed(false);
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function confirmCleanup(): Promise<void> {
    if (cleanupPlan === null || !cleanupConfirmed) {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("cleanupCommit");
    setError(null);
    try {
      const committedCleanup = await archiveClient.confirmCleanupPlan({
        planId: cleanupPlan.planId,
        confirmationNonce: cleanupPlan.confirmationNonce,
      });
      if (proposalId.current === activeProposal) {
        setCleanupResult(committedCleanup);
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function reviewArchiveUndo(operationId: string): Promise<void> {
    if (
      undoneOperationIds.has(operationId) ||
      cleanupPlan !== null ||
      cleanupResult !== null
    ) {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("undoPlan");
    setError(null);
    setUndoResult(null);
    try {
      const reviewed = await archiveClient.createArchiveUndoPlan({
        operationId,
      });
      if (proposalId.current === activeProposal) {
        setUndoPlan(reviewed);
        setUndoConfirmed(false);
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  async function confirmArchiveUndo(): Promise<void> {
    if (undoPlan === null || !undoConfirmed) {
      return;
    }
    const activeProposal = proposal.proposalId;
    setPending("undoCommit");
    setError(null);
    try {
      const completed = await archiveClient.confirmArchiveUndoPlan({
        undoId: undoPlan.undoId,
        confirmationNonce: undoPlan.confirmationNonce,
      });
      if (proposalId.current === activeProposal) {
        setUndoResult(completed);
        if (completed.status === "committed") {
          setUndoneOperationIds((current) => {
            const next = new Set(current);
            next.add(completed.operationId);
            return next;
          });
          setCleanupPlan(null);
          setCleanupConfirmed(false);
          setCleanupResult(null);
          onUndoneOperation?.(completed.operationId);
        }
      }
    } catch (nextError) {
      if (proposalId.current === activeProposal) {
        setError(errorText(nextError));
      }
    } finally {
      if (proposalId.current === activeProposal) {
        setPending(null);
      }
    }
  }

  const committed = result?.status === "committed";
  const activeCommittedItems =
    result?.items.filter(
      (item) =>
        item.status === "committed" &&
        !undoneOperationIds.has(item.operationId),
    ) ?? [];
  const selectedItems = proposal.items.filter((item) =>
    selectedIds.has(item.itemId),
  );
  const evidenceComplete =
    selectedItems.length > 0 &&
    selectedItems.every((item) => {
      const draft = evidence[item.itemId] ?? emptyEvidence;
      return (
        draft.subject.trim().length > 0 &&
        draft.classificationText.trim().length > 0 &&
        draft.evidenceLocation.trim().length > 0
      );
    });
  const classificationNeedsReview =
    classificationBatch?.items.some(
      (item) =>
        item.proposal.status !== "proposed" ||
        !item.proposal.committable ||
        item.proposal.destination === null,
    ) ?? true;
  const namingNeedsReview =
    namingBatch?.proposals.some((item) => item.status !== "proposed") ?? true;
  const statusLabel =
    result === null ? "Uncommitted" : committed ? "Committed" : "Attention";

  return (
    <section aria-label="Archive preview" className="archive-preview">
      <header className="archive-preview__header">
        <h2>Archive Preview</h2>
        <span>{statusLabel}</span>
      </header>

      <div className="archive-preview__vault">
        <div>
          <strong>Vault</strong>
          <span title={vault?.displayPath}>
            {vault?.displayPath ?? "No Vault selected"}
          </span>
        </div>
        <button
          disabled={pending !== null}
          onClick={() => void chooseVault()}
          type="button"
        >
          {pending === "vault" ? "Choosing…" : "Choose Vault"}
        </button>
      </div>

      <ul aria-label="Proposed archive tree" role="tree">
        <li role="none">
          <div aria-expanded="true" aria-level={1} role="treeitem">
            <Icon name="chevron" size={12} />
            <Icon name="folder" size={14} />
            <span>Reviewed sources</span>
          </div>
          <ul role="group">
            {proposal.items.map((item) => (
              <li key={item.itemId} role="none">
                <label className="archive-preview__item">
                  <input
                    aria-label={`Include ${item.name}`}
                    checked={selectedIds.has(item.itemId)}
                    disabled={pending !== null || committed}
                    onChange={() => toggleItem(item.itemId)}
                    type="checkbox"
                  />
                  <Icon name="document" size={13} />
                  <span title={item.path}>{item.name}</span>
                </label>
              </li>
            ))}
          </ul>
        </li>
      </ul>

      {modelRuntimeClient === undefined ? null : (
        <FileSemanticReview
          adoptedComparisonIds={semanticAdoptions}
          client={modelRuntimeClient}
          disabled={pending !== null || committed}
          items={selectedItems}
          onApply={applySemanticSuggestion}
          onResult={(itemId, comparison) => {
            setSemanticResults((current) => ({
              ...current,
              [itemId]: comparison,
            }));
            setSemanticAdoptions((current) => {
              const { [itemId]: _removed, ...remaining } = current;
              return remaining;
            });
          }}
          proposalId={proposal.proposalId}
          results={semanticResults}
          vaultSelected={vault !== null}
        />
      )}

      {selectedItems.length === 0 ? null : (
        <section
          aria-label="Local naming evidence"
          className="archive-preview__evidence"
        >
          <header>
            <strong>Local evidence</strong>
            <span>cited facts</span>
          </header>
          {selectedItems.map((item) => {
            const draft = evidence[item.itemId] ?? emptyEvidence;
            return (
              <fieldset key={item.itemId}>
                <legend>{item.name}</legend>
                {factFields.map(({ kind, label }) => (
                  <label key={kind}>
                    <span>{label}</span>
                    <input
                      aria-label={`${label} for ${item.name}`}
                      disabled={pending !== null || committed}
                      onChange={(event) =>
                        updateEvidence(item.itemId, kind, event.target.value)
                      }
                      required={kind === "subject"}
                      type="text"
                      value={draft[kind]}
                    />
                  </label>
                ))}
                <label>
                  <span>Classification evidence</span>
                  <textarea
                    aria-label={`Classification evidence for ${item.name}`}
                    disabled={pending !== null || committed}
                    onChange={(event) =>
                      updateEvidence(
                        item.itemId,
                        "classificationText",
                        event.target.value,
                      )
                    }
                    placeholder="Paste the source passage that supports its primary category"
                    required
                    value={draft.classificationText}
                  />
                </label>
                <label>
                  <span>Evidence location</span>
                  <input
                    aria-label={`Evidence location for ${item.name}`}
                    disabled={pending !== null || committed}
                    onChange={(event) =>
                      updateEvidence(
                        item.itemId,
                        "evidenceLocation",
                        event.target.value,
                      )
                    }
                    placeholder="page:1 / section"
                    required
                    type="text"
                    value={draft.evidenceLocation}
                  />
                </label>
              </fieldset>
            );
          })}
        </section>
      )}

      <div className="archive-preview__actions">
        <button
          disabled={!evidenceComplete || pending !== null || committed}
          onClick={() => void reviewClassification()}
          type="button"
        >
          {pending === "classification"
            ? "Checking classification…"
            : "Review classification"}
        </button>
        <button
          disabled={classificationNeedsReview || pending !== null || committed}
          onClick={() => void reviewNames()}
          type="button"
        >
          {pending === "naming" ? "Checking names…" : "Review canonical names"}
        </button>
        <button
          disabled={
            vault === null ||
            classificationBatch === null ||
            classificationNeedsReview ||
            namingBatch === null ||
            namingNeedsReview ||
            pending !== null ||
            committed
          }
          onClick={() => void reviewPlan()}
          type="button"
        >
          {pending === "plan" ? "Building plan…" : "Review archive plan"}
        </button>
      </div>

      {classificationBatch === null ? null : (
        <section
          aria-label="Classification review"
          className="archive-preview__classification-review"
        >
          <header>
            <strong>Primary classification</strong>
            <span>
              {classificationBatch.profileId} ·{" "}
              {classificationBatch.profileVersion}
            </span>
          </header>
          {classificationBatch.items.map((item) => (
            <article key={item.itemId}>
              <p>
                {item.proposal.destination?.join(" / ") ?? "Review required"}
              </p>
              {item.proposal.reviewReason === null ? null : (
                <strong>
                  {classificationReviewReasonLabels[item.proposal.reviewReason]}
                </strong>
              )}
              <dl>
                <dt>Rules</dt>
                <dd>
                  {item.proposal.ruleIds.join(", ") ||
                    item.proposal.semanticDecisionId ||
                    "None"}
                </dd>
                <dt>Evidence</dt>
                <dd>
                  {item.proposal.evidence
                    .map((citation) => citation.location)
                    .join(", ") || "None"}
                </dd>
                <dt>SHA-256</dt>
                <dd>{item.proposal.sourceIdentity.digest}</dd>
              </dl>
            </article>
          ))}
        </section>
      )}

      {namingBatch === null ? null : (
        <section
          aria-label="Canonical name review"
          className="archive-preview__naming-review"
        >
          <header>
            <strong>Canonical names</strong>
            <span>
              {namingBatch.policyId} · {namingBatch.policyVersion}
            </span>
          </header>
          {namingBatch.proposals.map((item) => (
            <article key={item.itemId}>
              <p>
                {item.originalName} → {item.canonicalName ?? "Review required"}
              </p>
              {item.reviewReason === null ? null : (
                <strong>{reviewReasonLabels[item.reviewReason]}</strong>
              )}
              <dl>
                <dt>SHA-256</dt>
                <dd>{item.identity.digest}</dd>
              </dl>
            </article>
          ))}
        </section>
      )}

      {plan === null ? null : (
        <section
          aria-label="Exact archive plan"
          className="archive-preview__plan"
        >
          <header>
            <strong>Exact plan · {plan.items.length}</strong>
            <span>Expires {new Date(plan.expiresAtUnixMs).toLocaleTimeString()}</span>
          </header>
          <p className="archive-preview__invariant">
            Source file remains in place. A verified original is added to the Vault.
          </p>
          {plan.items.map((item) => (
            <details key={item.itemId}>
              <summary>{item.sourcePath.split(/[\\/]/).pop()}</summary>
              <dl>
                <dt>Source</dt>
                <dd>{item.sourcePath}</dd>
                <dt>Destination</dt>
                <dd>{item.destinationPath}</dd>
                <dt>Canonical name</dt>
                <dd>
                  {item.originalName} → {item.canonicalName}
                </dd>
                <dt>Naming policy</dt>
                <dd>
                  {item.naming.policyId} · {item.naming.policyVersion}
                </dd>
                {item.classification === undefined ? null : (
                  <>
                    <dt>Primary category</dt>
                    <dd>{item.classification.destination?.join(" / ")}</dd>
                    <dt>Classification profile</dt>
                    <dd>
                      {item.classification.profileId} ·{" "}
                      {item.classification.profileVersion}
                    </dd>
                  </>
                )}
                <dt>SHA-256</dt>
                <dd>{item.identity.digest}</dd>
              </dl>
            </details>
          ))}
          <label className="archive-preview__confirmation">
            <input
              checked={confirmed}
              disabled={pending !== null || committed}
              onChange={(event) => setConfirmed(event.target.checked)}
              type="checkbox"
            />
            <span>I reviewed every source, destination, and SHA-256.</span>
          </label>
          <button
            disabled={!confirmed || pending !== null || result !== null}
            onClick={() => void confirmPlan()}
            type="button"
          >
            {pending === "commit" ? "Verifying…" : "Confirm verified archive"}
          </button>
        </section>
      )}

      {error === null ? null : (
        <p className="archive-preview__error" role="alert">
          {error}
        </p>
      )}

      {result === null ? (
        <p className="archive-preview__notice">
          No file changes until an exact plan is confirmed.
        </p>
      ) : (
        <section
          aria-label="Archive result"
          className={`archive-preview__result archive-preview__result--${result.status}`}
        >
          <strong>
            {committed ? "Archive committed" : "Archive needs attention"}
          </strong>
          <span>
            {result.items.filter((item) => item.status === "committed").length}
            /{result.items.length} verified · source preserved
          </span>
        </section>
      )}

      {activeCommittedItems.length > 0 ? (
        <section
          aria-label="Archive undo"
          className="archive-preview__cleanup"
        >
          <header>
            <strong>Archive undo</strong>
            <span>Bounded · Trash</span>
          </header>
          <p className="archive-preview__invariant">
            Available only while the matching source remains a verified
            original and no authoritative knowledge depends on the archive.
          </p>
          {activeCommittedItems.map((item) => (
            <button
              disabled={
                pending !== null ||
                undoPlan !== null ||
                cleanupPlan !== null ||
                cleanupResult !== null
              }
              key={item.operationId}
              onClick={() => void reviewArchiveUndo(item.operationId)}
              type="button"
            >
              {pending === "undoPlan"
                ? "Rechecking undo…"
                : `Review archive undo · ${
                    item.destinationPath.split(/[\\/]/).pop() ??
                    item.operationId
                  }`}
            </button>
          ))}
        </section>
      ) : null}

      {undoPlan === null ? null : (
        <section
          aria-label="Exact archive undo plan"
          className="archive-preview__plan archive-preview__cleanup-plan"
        >
          <header>
            <strong>Exact archive undo plan</strong>
            <span>Operating-system trash</span>
          </header>
          <p className="archive-preview__invariant">
            The external source is independently reverified before and after
            the archived copy changes. An unsafe execution restores the Vault
            original from transaction staging.
          </p>
          <dl>
            <dt>Source original</dt>
            <dd>{undoPlan.sourcePath}</dd>
            <dt>Archived original</dt>
            <dd>{undoPlan.archivedPath}</dd>
            <dt>SHA-256</dt>
            <dd>{undoPlan.identity.digest}</dd>
          </dl>
          <label className="archive-preview__confirmation">
            <input
              checked={undoConfirmed}
              disabled={pending !== null || undoResult !== null}
              onChange={(event) => setUndoConfirmed(event.target.checked)}
              type="checkbox"
            />
            <span>I reviewed the source, archive path, and SHA-256.</span>
          </label>
          <button
            disabled={!undoConfirmed || pending !== null || undoResult !== null}
            onClick={() => void confirmArchiveUndo()}
            type="button"
          >
            {pending === "undoCommit"
              ? "Reverifying undo…"
              : "Confirm archive undo"}
          </button>
        </section>
      )}

      {undoResult === null ? null : (
        <section
          aria-label="Archive undo result"
          className={`archive-preview__result archive-preview__result--${undoResult.status}`}
        >
          <strong>
            {undoResult.status === "committed"
              ? "Archive undo committed"
              : "Archive undo failed"}
          </strong>
          <span>
            {undoResult.status === "committed"
              ? "Source original preserved · archived registration deactivated"
              : undoResult.failureReason}
          </span>
        </section>
      )}

      {activeCommittedItems.length > 0 ? (
        <section
          aria-label="Source cleanup"
          className="archive-preview__cleanup"
        >
          <header>
            <strong>Source cleanup</strong>
            <span>Off by default</span>
          </header>
          <label className="archive-preview__confirmation">
            <input
              checked={cleanupEnabled}
              disabled={pending !== null || cleanupResult?.status === "committed"}
              onChange={(event) => {
                setCleanupEnabled(event.target.checked);
                setCleanupPlan(null);
                setCleanupConfirmed(false);
                setCleanupResult(null);
              }}
              type="checkbox"
            />
            <span>Enable cleanup for these archived sources.</span>
          </label>
          <button
            disabled={
              !cleanupEnabled ||
              pending !== null ||
              cleanupPlan !== null ||
              cleanupResult !== null
            }
            onClick={() => void reviewCleanup()}
            type="button"
          >
            {pending === "cleanupPlan" ? "Rechecking originals…" : "Review source cleanup"}
          </button>
        </section>
      ) : null}

      {cleanupPlan === null ? null : (
        <section
          aria-label="Exact cleanup plan"
          className="archive-preview__plan archive-preview__cleanup-plan"
        >
          <header>
            <strong>
              Exact cleanup plan · {cleanupPlan.items.length}
            </strong>
            <span>
              {cleanupPlan.disposition === "trash"
                ? "Operating-system trash"
                : "Permanent deletion"}
            </span>
          </header>
          <p className="archive-preview__invariant">
            A freshly verified retained original remains in the Vault for every
            selected source.
          </p>
          {cleanupPlan.items.map((item) => (
            <details key={item.operationId}>
              <summary>{item.sourcePath.split(/[\\/]/).pop()}</summary>
              <dl>
                <dt>Source copy</dt>
                <dd>{item.sourcePath}</dd>
                <dt>Retained original</dt>
                <dd>{item.retainedPath}</dd>
                <dt>SHA-256</dt>
                <dd>{item.identity.digest}</dd>
              </dl>
            </details>
          ))}
          {cleanupPlan.disposition === "permanentDelete" ? (
            <p className="archive-preview__error">
              Permanent deletion cannot be undone. This is the separate,
              second confirmation.
            </p>
          ) : null}
          <label className="archive-preview__confirmation">
            <input
              checked={cleanupConfirmed}
              disabled={pending !== null || cleanupResult !== null}
              onChange={(event) => setCleanupConfirmed(event.target.checked)}
              type="checkbox"
            />
            <span>I reviewed every cleanup path and SHA-256.</span>
          </label>
          <div className="archive-preview__actions">
            <button
              disabled={!cleanupConfirmed || pending !== null || cleanupResult !== null}
              onClick={() => void confirmCleanup()}
              type="button"
            >
              {pending === "cleanupCommit"
                ? "Reverifying…"
                : cleanupPlan.disposition === "trash"
                  ? "Confirm move to trash"
                  : "Confirm permanent deletion"}
            </button>
            {cleanupPlan.disposition === "trash" ? (
              <button
                disabled={pending !== null || cleanupResult !== null}
                onClick={() => void requestPermanentCleanup()}
                type="button"
              >
                {pending === "permanentCleanup"
                  ? "Preparing separate confirmation…"
                  : "Request permanent deletion"}
              </button>
            ) : null}
          </div>
        </section>
      )}

      {cleanupResult === null ? null : (
        <section
          aria-label="Cleanup result"
          className={`archive-preview__result archive-preview__result--${cleanupResult.status}`}
        >
          <strong>
            {cleanupResult.status === "committed"
              ? "Source cleanup committed"
              : "Source cleanup failed"}
          </strong>
          <span>
            {cleanupResult.removedPaths.length} source
            {cleanupResult.removedPaths.length === 1 ? "" : "s"} handled ·
            retained original preserved
          </span>
        </section>
      )}
    </section>
  );
}
