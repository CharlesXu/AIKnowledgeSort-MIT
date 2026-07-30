import { useEffect, useState } from "react";
import {
  statusTranslationKey,
  useI18n,
} from "../../i18n/I18nContext";
import type { KnowledgeDocument } from "../knowledge/types";
import type {
  ComparisonRecord,
  ModelConfigSummary,
  ModelRuntimeClient,
  ModelRuntimeState,
  ProviderOutcome,
} from "./types";

interface AgentReviewPaneProps {
  readonly client: ModelRuntimeClient;
  readonly document: KnowledgeDocument | null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function ProviderCard({
  config,
  configId,
  outcome,
  side,
}: {
  readonly config: ModelConfigSummary | undefined;
  readonly configId: string;
  readonly outcome: ProviderOutcome;
  readonly side: "Desktop" | "Agent";
}) {
  const { t } = useI18n();
  return (
    <article className="agent-provider-card">
      <header>
        <span>{side === "Desktop" ? t("agentReview.desktop") : t("agentReview.agent")}</span>
        <strong>{config?.label ?? configId}</strong>
      </header>
      <p className={`agent-provider-card__status agent-provider-card__status--${outcome.status}`}>
        {statusTranslationKey(outcome.status)
          ? t(statusTranslationKey(outcome.status)!)
          : outcome.status.toUpperCase()} · {outcome.model ?? t("agentReview.noResponse")}
      </p>
      {outcome.proposal ? (
        <>
          <p>{outcome.proposal.summary}</p>
          <ul>
            {outcome.proposal.relations.map((relation, index) => (
              <li key={`${relation.source}-${relation.relationType}-${relation.target}-${index}`}>
                <code>{relation.source}</code>
                <span>{relation.relationType}</span>
                <code>{relation.target}</code>
                <small>{relation.evidenceIds.join(", ")}</small>
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {outcome.failureReason ? (
        <p className="agent-review__failure">{outcome.failureReason}</p>
      ) : null}
    </article>
  );
}

export function AgentReviewPane({ client, document }: AgentReviewPaneProps) {
  const { t } = useI18n();
  const [runtime, setRuntime] = useState<ModelRuntimeState | null>(null);
  const [desktopConfigId, setDesktopConfigId] = useState("");
  const [agentConfigId, setAgentConfigId] = useState("");
  const [startLine, setStartLine] = useState("1");
  const [endLine, setEndLine] = useState("1");
  const [result, setResult] = useState<ComparisonRecord | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    client.inspect()
      .then((state) => {
        if (!active) return;
        setRuntime(state);
        setDesktopConfigId((current) => current || state.configs[0]?.configId || "");
        setAgentConfigId((current) => current || state.configs[1]?.configId || "");
      })
      .catch((reason: unknown) => {
        if (active) setError(errorMessage(reason));
      });
    return () => {
      active = false;
    };
  }, [client]);

  const saved = document !== null
    && document.revision > 0
    && document.markdownPath !== null
    && document.markdownIdentity !== null;
  const configs = runtime?.configs ?? [];
  const canRun = saved
    && configs.length >= 2
    && desktopConfigId !== ""
    && agentConfigId !== ""
    && desktopConfigId !== agentConfigId;

  async function run(): Promise<void> {
    if (!document || !canRun) return;
    setBusy(true);
    setError(null);
    try {
      const start = Number(startLine);
      const end = Number(endLine);
      setResult(await client.runComparison({
        authorityId: document.authorityId,
        operationId: document.operationId,
        knowledgeRevision: document.revision,
        evidenceRanges: [{ startLine: start, endLine: end }],
        desktopConfigId,
        agentConfigId,
      }));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  if (!saved) {
    return (
      <section className="agent-review agent-review--empty">
        <h3>{t("agentReview.title")}</h3>
        <p>{t("agentReview.savedRequired")}</p>
        <span>{t("agentReview.draftWarning")}</span>
        {error ? <p className="agent-review__failure" role="alert">{error}</p> : null}
      </section>
    );
  }

  return (
    <div className="agent-review">
      <section className="context-section agent-review__controls" aria-labelledby="agent-review-run">
        <div className="agent-review__heading">
          <div>
            <h3 id="agent-review-run">{t("agentReview.comparison")}</h3>
            <p>{t("agentReview.revision", { revision: document.revision })}</p>
          </div>
          <span>{t("agentReview.readOnly")}</span>
        </div>
        {configs.length < 2 ? (
          <p className="agent-review__notice">{t("agentReview.configureTwo")}</p>
        ) : (
          <form
            className="agent-review__form"
            onSubmit={(event) => {
              event.preventDefault();
              void run();
            }}
          >
            <label>
              {t("agentReview.desktopModel")}
              <select value={desktopConfigId} onChange={(event) => setDesktopConfigId(event.target.value)}>
                {configs.map((config) => (
                  <option key={config.configId} value={config.configId}>{config.label}</option>
                ))}
              </select>
            </label>
            <label>
              {t("agentReview.agentModel")}
              <select value={agentConfigId} onChange={(event) => setAgentConfigId(event.target.value)}>
                {configs.map((config) => (
                  <option key={config.configId} value={config.configId}>{config.label}</option>
                ))}
              </select>
            </label>
            <div className="agent-review__range">
              <label>
                {t("agentReview.startLine")}
                <input min="1" onChange={(event) => setStartLine(event.target.value)} type="number" value={startLine} />
              </label>
              <label>
                {t("agentReview.endLine")}
                <input min="1" onChange={(event) => setEndLine(event.target.value)} type="number" value={endLine} />
              </label>
            </div>
            <button disabled={!canRun || busy} type="submit">
              {busy ? t("agentReview.comparing") : t("agentReview.run")}
            </button>
          </form>
        )}
        {error ? <p className="agent-review__failure" role="alert">{error}</p> : null}
      </section>

      {result ? (
        <section aria-label={t("agentReview.result")} className="agent-review__result">
          <header className="agent-review__result-header">
            <div>
              <span>{t("agentReview.envelope")}</span>
              <code>{result.envelopeIdentity.digest}</code>
            </div>
            <strong className={`agent-review__status agent-review__status--${result.status}`}>
              {statusTranslationKey(result.status)
                ? t(statusTranslationKey(result.status)!)
                : result.status.toUpperCase()}
            </strong>
          </header>
          <div className="agent-review__providers">
            <ProviderCard
              config={configs.find((config) => config.configId === result.desktopConfigId)}
              configId={result.desktopConfigId}
              outcome={result.desktopOutcome}
              side="Desktop"
            />
            <ProviderCard
              config={configs.find((config) => config.configId === result.agentConfigId)}
              configId={result.agentConfigId}
              outcome={result.agentOutcome}
              side="Agent"
            />
          </div>
          <article className="agent-adjudication">
            <span>{t("agentReview.decision")}</span>
            {result.adjudication ? (
              <>
                <strong>
                  {statusTranslationKey(result.adjudication.decision)
                    ? t(statusTranslationKey(result.adjudication.decision)!)
                    : result.adjudication.decision.toUpperCase()}
                </strong>
                <p>{result.adjudication.reason}</p>
                <small>{t("agentReview.evidence", {
                  ids: result.adjudication.evidenceIds.join(", "),
                })}</small>
              </>
            ) : (
              <p className="agent-review__failure">
                {result.adjudicationFailure ?? t("agentReview.adjudicationSkipped")}
              </p>
            )}
            <em>{t("agentReview.semanticAdvice")}</em>
          </article>
          <section className="agent-review__evidence" aria-label={t("agentReview.authoritativeEvidence")}>
            <h4>{t("agentReview.authoritativeEvidence")}</h4>
            {result.envelope.evidence.map((evidence) => (
              <article key={evidence.evidenceId}>
                <span>{evidence.evidenceId}</span>
                <pre>{evidence.text}</pre>
              </article>
            ))}
          </section>
        </section>
      ) : null}
    </div>
  );
}
