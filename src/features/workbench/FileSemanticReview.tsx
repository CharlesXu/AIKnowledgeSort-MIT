import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n/I18nContext";
import type { DiscoveredItem } from "../drop/types";
import type {
  FileSemanticComparison,
  FileSemanticProviderOutcome,
  ModelRuntimeClient,
  ModelRuntimeState,
} from "../models/types";

interface FileSemanticReviewProps {
  readonly adoptedComparisonIds: Readonly<Record<string, string>>;
  readonly client: ModelRuntimeClient;
  readonly disabled: boolean;
  readonly items: readonly DiscoveredItem[];
  readonly onApply: (
    itemId: string,
    comparison: FileSemanticComparison,
  ) => void;
  readonly onResult: (
    itemId: string,
    comparison: FileSemanticComparison,
  ) => void;
  readonly proposalId: string;
  readonly results: Readonly<Record<string, FileSemanticComparison>>;
  readonly vaultSelected: boolean;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function Provider({
  label,
  outcome,
}: {
  readonly label: string;
  readonly outcome: FileSemanticProviderOutcome;
}) {
  const { t } = useI18n();
  return (
    <section>
      <strong>{label}</strong>
      <span>{outcome.model ?? outcome.status}</span>
      <p>
        {outcome.suggestion?.summary ??
          outcome.failureReason ??
          t("semantic.noSuggestion")}
      </p>
    </section>
  );
}

export function FileSemanticReview({
  adoptedComparisonIds,
  client,
  disabled,
  items,
  onApply,
  onResult,
  proposalId,
  results,
  vaultSelected,
}: FileSemanticReviewProps) {
  const { t } = useI18n();
  const [runtime, setRuntime] = useState<ModelRuntimeState | null>(null);
  const [desktopConfigId, setDesktopConfigId] = useState("");
  const [agentConfigId, setAgentConfigId] = useState("");
  const [pendingItemId, setPendingItemId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const currentProposalId = useRef(proposalId);

  useEffect(() => {
    currentProposalId.current = proposalId;
    setPendingItemId(null);
    setError(null);
    let active = true;
    client.inspect()
      .then((state) => {
        if (!active) {
          return;
        }
        setRuntime(state);
        setDesktopConfigId(state.configs[0]?.configId ?? "");
        setAgentConfigId(state.configs[1]?.configId ?? "");
      })
      .catch(() => {
        if (active) {
          setRuntime(null);
        }
      });
    return () => {
      active = false;
    };
  }, [client, proposalId]);

  async function compare(itemId: string): Promise<void> {
    if (
      desktopConfigId === "" ||
      agentConfigId === "" ||
      desktopConfigId === agentConfigId
    ) {
      return;
    }
    const activeProposalId = proposalId;
    setPendingItemId(itemId);
    setError(null);
    try {
      const comparison = await client.runFileSemanticComparison({
        proposalId,
        itemId,
        desktopConfigId,
        agentConfigId,
      });
      if (currentProposalId.current === activeProposalId) {
        onResult(itemId, comparison);
      }
    } catch (nextError) {
      if (currentProposalId.current === activeProposalId) {
        setError(errorText(nextError));
      }
    } finally {
      if (currentProposalId.current === activeProposalId) {
        setPendingItemId(null);
      }
    }
  }

  const configs = runtime?.configs ?? [];
  function adjudicationLabel(comparison: FileSemanticComparison): string {
    const adjudication = comparison.adjudication;
    if (adjudication === null) {
      return t("semantic.reviewUnavailable");
    }
    if (adjudication.decision === "accept" && adjudication.selectedSide !== null) {
      return adjudication.selectedSide === "desktop"
        ? t("semantic.acceptedDesktop")
        : t("semantic.acceptedAgent");
    }
    return t("semantic.decision", { decision: adjudication.decision });
  }

  if (items.length === 0) {
    return null;
  }

  return (
    <section
      aria-label={t("semantic.label")}
      className="archive-preview__semantic-review"
    >
      <header>
        <strong>{t("semantic.title")}</strong>
        <span>{t("semantic.readOnly")}</span>
      </header>
      {configs.length < 2 ? (
        <p>{t("semantic.configureTwo")}</p>
      ) : (
        <div className="archive-preview__model-pair">
          <label>
            <span>{t("semantic.desktopModel")}</span>
            <select
              aria-label={t("semantic.desktopSelect")}
              disabled={pendingItemId !== null || disabled}
              onChange={(event) => setDesktopConfigId(event.target.value)}
              value={desktopConfigId}
            >
              {configs.map((config) => (
                <option key={config.configId} value={config.configId}>
                  {config.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>{t("semantic.agentModel")}</span>
            <select
              aria-label={t("semantic.agentSelect")}
              disabled={pendingItemId !== null || disabled}
              onChange={(event) => setAgentConfigId(event.target.value)}
              value={agentConfigId}
            >
              {configs.map((config) => (
                <option key={config.configId} value={config.configId}>
                  {config.label}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
      {items.map((item) => {
        const comparison = results[item.itemId];
        const resolved = comparison?.resolvedSuggestion ?? null;
        const category = comparison?.envelope.profile.categories.find(
          (candidate) => candidate.categoryId === resolved?.categoryId,
        );
        const ambiguousFacts =
          resolved !== null &&
          new Set(resolved.namingFacts.map((fact) => fact.kind)).size !==
            resolved.namingFacts.length;
        return (
          <article key={item.itemId}>
            <header>
              <strong>{item.name}</strong>
              <button
                aria-label={t("semantic.compareLabel", { name: item.name })}
                disabled={
                  disabled ||
                  !vaultSelected ||
                  pendingItemId !== null ||
                  configs.length < 2 ||
                  desktopConfigId === agentConfigId
                }
                onClick={() => void compare(item.itemId)}
                type="button"
              >
                {pendingItemId === item.itemId
                  ? t("semantic.comparing")
                  : t("semantic.compare")}
              </button>
            </header>
            {comparison === undefined ? null : (
              <div className="archive-preview__semantic-controls">
                <strong>{adjudicationLabel(comparison)}</strong>
                <p>
                  {category?.path.join(" / ") ??
                    resolved?.uncertaintyReason ??
                    t("semantic.noCategory")}
                </p>
                <div className="archive-preview__semantic-providers">
                  <Provider label={t("semantic.desktop")} outcome={comparison.desktopOutcome} />
                  <Provider label={t("semantic.agent")} outcome={comparison.agentOutcome} />
                </div>
                <button
                  aria-label={t("semantic.applyLabel", { name: item.name })}
                  disabled={
                    resolved === null ||
                    resolved.categoryId === null ||
                    ambiguousFacts ||
                    adoptedComparisonIds[item.itemId] === comparison.comparisonId
                  }
                  onClick={() => onApply(item.itemId, comparison)}
                  type="button"
                >
                  {adoptedComparisonIds[item.itemId] === comparison.comparisonId
                    ? t("semantic.applied")
                    : t("semantic.apply")}
                </button>
              </div>
            )}
          </article>
        );
      })}
      {error === null ? null : (
        <p className="archive-preview__semantic-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
