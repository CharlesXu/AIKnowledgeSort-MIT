import { useEffect, useRef, useState } from "react";
import type {
  ArchiveClient,
  ArchiveCommitResult,
  ArchivePlan,
  VaultSummary,
} from "../archive/types";
import type { DiscoveryProposal } from "../drop/types";
import { Icon } from "../../ui/Icon";

interface ArchivePreviewPaneProps {
  readonly archiveClient: ArchiveClient;
  readonly proposal: DiscoveryProposal;
}

type PendingAction = "vault" | "plan" | "commit" | null;

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function ArchivePreviewPane({
  archiveClient,
  proposal,
}: ArchivePreviewPaneProps) {
  const proposalId = useRef(proposal.proposalId);
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [vault, setVault] = useState<VaultSummary | null>(null);
  const [plan, setPlan] = useState<ArchivePlan | null>(null);
  const [result, setResult] = useState<ArchiveCommitResult | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [pending, setPending] = useState<PendingAction>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    proposalId.current = proposal.proposalId;
    setSelectedIds(new Set());
    setPlan(null);
    setResult(null);
    setConfirmed(false);
    setPending(null);
    setError(null);
  }, [proposal.proposalId]);

  function invalidatePlan(): void {
    setPlan(null);
    setResult(null);
    setConfirmed(false);
    setError(null);
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
    invalidatePlan();
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
      invalidatePlan();
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
    if (vault === null || selectedIds.size === 0) {
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
      });
      if (proposalId.current === activeProposal) {
        setPlan(reviewed);
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

      <div className="archive-preview__actions">
        <button
          disabled={vault === null || selectedIds.size === 0 || pending !== null || committed}
          onClick={() => void reviewPlan()}
          type="button"
        >
          {pending === "plan" ? "Building plan…" : "Review archive plan"}
        </button>
      </div>

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
    </section>
  );
}
