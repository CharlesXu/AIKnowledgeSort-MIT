import { useEffect, useState } from "react";
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
  return (
    <article className="agent-provider-card">
      <header>
        <span>{side}</span>
        <strong>{config?.label ?? configId}</strong>
      </header>
      <p className={`agent-provider-card__status agent-provider-card__status--${outcome.status}`}>
        {outcome.status.toUpperCase()} · {outcome.model ?? "No model response"}
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
        <h3>Agent Review</h3>
        <p>A saved Vault revision is required before evidence can be compared.</p>
        <span>Draft editor text is never submitted as authoritative evidence.</span>
        {error ? <p className="agent-review__failure" role="alert">{error}</p> : null}
      </section>
    );
  }

  return (
    <div className="agent-review">
      <section className="context-section agent-review__controls" aria-labelledby="agent-review-run">
        <div className="agent-review__heading">
          <div>
            <h3 id="agent-review-run">Evidence comparison</h3>
            <p>Vault revision {document.revision} · exact line ranges only</p>
          </div>
          <span>READ ONLY</span>
        </div>
        {configs.length < 2 ? (
          <p className="agent-review__notice">Configure two distinct models in Settings.</p>
        ) : (
          <form
            className="agent-review__form"
            onSubmit={(event) => {
              event.preventDefault();
              void run();
            }}
          >
            <label>
              Desktop model
              <select value={desktopConfigId} onChange={(event) => setDesktopConfigId(event.target.value)}>
                {configs.map((config) => (
                  <option key={config.configId} value={config.configId}>{config.label}</option>
                ))}
              </select>
            </label>
            <label>
              Agent model
              <select value={agentConfigId} onChange={(event) => setAgentConfigId(event.target.value)}>
                {configs.map((config) => (
                  <option key={config.configId} value={config.configId}>{config.label}</option>
                ))}
              </select>
            </label>
            <div className="agent-review__range">
              <label>
                Start line
                <input min="1" onChange={(event) => setStartLine(event.target.value)} type="number" value={startLine} />
              </label>
              <label>
                End line
                <input min="1" onChange={(event) => setEndLine(event.target.value)} type="number" value={endLine} />
              </label>
            </div>
            <button disabled={!canRun || busy} type="submit">
              {busy ? "Comparing…" : "Run comparison"}
            </button>
          </form>
        )}
        {error ? <p className="agent-review__failure" role="alert">{error}</p> : null}
      </section>

      {result ? (
        <section aria-label="Model comparison result" className="agent-review__result">
          <header className="agent-review__result-header">
            <div>
              <span>ENVELOPE SHA-256</span>
              <code>{result.envelopeIdentity.digest}</code>
            </div>
            <strong className={`agent-review__status agent-review__status--${result.status}`}>
              {result.status.toUpperCase()}
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
            <span>AGENT DECISION</span>
            {result.adjudication ? (
              <>
                <strong>{result.adjudication.decision.toUpperCase()}</strong>
                <p>{result.adjudication.reason}</p>
                <small>Evidence: {result.adjudication.evidenceIds.join(", ")}</small>
              </>
            ) : (
              <p className="agent-review__failure">
                {result.adjudicationFailure ?? "Adjudication did not run because a proposal failed."}
              </p>
            )}
            <em>Semantic advice · no operation authorized</em>
          </article>
          <section className="agent-review__evidence" aria-label="Comparison evidence">
            <h4>Authoritative evidence</h4>
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
