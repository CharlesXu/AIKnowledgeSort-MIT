import { useEffect, useState } from "react";
import type {
  AgentAccessClient,
  AgentAccessState,
  AgentResourceLimits,
  NativeScopeSelection,
} from "./types";

const defaultLimits: AgentResourceLimits = {
  maxRequestsPerSession: 1_000,
  maxRequestBytes: 128 * 1024,
  maxResponseBytes: 256 * 1024,
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function AgentAccessPanel({ client }: { readonly client: AgentAccessClient }) {
  const [state, setState] = useState<AgentAccessState | null>(null);
  const [selection, setSelection] = useState<NativeScopeSelection | null>(null);
  const [agentId, setAgentId] = useState("");
  const [label, setLabel] = useState("");
  const [toolIds, setToolIds] = useState<readonly string[]>([]);
  const [expiryHours, setExpiryHours] = useState("1");
  const [limits, setLimits] = useState<AgentResourceLimits>(defaultLimits);
  const [issuedToken, setIssuedToken] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    client.inspect()
      .then((next) => {
        if (active) setState(next);
      })
      .catch((reason: unknown) => {
        if (active) setError(errorMessage(reason));
      });
    return () => {
      active = false;
    };
  }, [client]);

  async function chooseDirectories(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      setSelection(await client.selectDirectories());
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  function toggleTool(toolId: string): void {
    setToolIds((current) => current.includes(toolId)
      ? current.filter((id) => id !== toolId)
      : [...current, toolId]);
  }

  function updateLimits(changes: Partial<AgentResourceLimits>): void {
    setLimits((current) => ({ ...current, ...changes }));
  }

  async function issueGrant(): Promise<void> {
    if (!selection) return;
    setBusy(true);
    setError(null);
    setIssuedToken(null);
    try {
      const issued = await client.createGrant({
        selectionId: selection.selectionId,
        agentId,
        label,
        toolIds,
        expiresInSeconds: Math.round(Number(expiryHours) * 3_600),
        limits,
      });
      setState((current) => current ? {
        ...current,
        grants: [
          ...current.grants.filter((grant) => grant.grantId !== issued.grant.grantId),
          issued.grant,
        ],
      } : current);
      setIssuedToken(issued.grantToken);
      setSelection(null);
      setToolIds([]);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function revoke(grantId: string): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      setState(await client.revokeGrant({ grantId }));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  const canIssue = Boolean(selection && agentId && label && toolIds.length);

  return (
    <div className="agent-access">
      <section aria-labelledby="agent-grants-heading" className="agent-access__grants">
        <div className="agent-access__section-heading">
          <div>
            <h3 id="agent-grants-heading">Agent grants</h3>
            <p>Persistent metadata · runtime capabilities never reopen silently</p>
          </div>
          <code>{state?.toolCatalogVersion ?? "agent-tools-v1"}</code>
        </div>
        {state?.grants.length ? state.grants.map((grant) => (
          <article className="agent-grant-row" key={grant.grantId}>
            <div className="agent-grant-row__title">
              <strong>{grant.label}</strong>
              <span className={`agent-grant-status agent-grant-status--${grant.status}`}>
                {grant.status.toUpperCase()}
              </span>
            </div>
            <span>{grant.agentId}</span>
            <p>{grant.toolIds.join(" · ")}</p>
            {grant.scopes.map((scope) => <code key={scope.scopeId}>{scope.displayPath}</code>)}
            <small>Expires {new Date(grant.expiresAtUnixMs).toLocaleString()}</small>
            {grant.status === "active" || grant.status === "inactive" ? (
              <button
                disabled={busy}
                onClick={() => void revoke(grant.grantId)}
                type="button"
              >
                Revoke {grant.label}
              </button>
            ) : null}
          </article>
        )) : <p className="model-settings__empty">No Agent grants.</p>}
        {issuedToken ? (
          <div className="agent-token" role="status">
            <strong>One-time grant token</strong>
            <p>This token cannot be recovered after dismissal. Configure it only in the intended local Agent runtime.</p>
            <code>{issuedToken}</code>
            <button onClick={() => setIssuedToken(null)} type="button">Dismiss token</button>
          </div>
        ) : null}
      </section>

      <form
        className="agent-access__form model-settings__form"
        onSubmit={(event) => {
          event.preventDefault();
          void issueGrant();
        }}
      >
        <h3>Issue bounded access</h3>
        <label>
          Agent ID
          <input onChange={(event) => setAgentId(event.target.value)} value={agentId} />
        </label>
        <label>
          Grant label
          <input onChange={(event) => setLabel(event.target.value)} value={label} />
        </label>
        <div className="agent-scope-picker">
          <button disabled={busy} onClick={() => void chooseDirectories()} type="button">
            Choose directories
          </button>
          {selection?.scopes.map((scope) => (
            <code key={scope.scopeId}>{scope.displayPath}</code>
          ))}
          {!selection ? <span>No native scope selected.</span> : null}
        </div>
        <fieldset className="agent-tool-list">
          <legend>Allowed tools</legend>
          {state?.tools.map((tool) => (
            <label key={tool.toolId}>
              <input
                aria-label={tool.title}
                checked={toolIds.includes(tool.toolId)}
                onChange={() => toggleTool(tool.toolId)}
                type="checkbox"
              />
              <span>
                {tool.title}
                <small>{tool.toolId} · {tool.effect === "read" ? "Read" : "Semantic advice"}</small>
              </span>
            </label>
          ))}
        </fieldset>
        <div className="agent-limit-grid">
          <label>
            Expiry (hours)
            <input min="0.0166667" max="720" step="any" type="number" value={expiryHours}
              onChange={(event) => setExpiryHours(event.target.value)} />
          </label>
          <label>
            Maximum requests
            <input min="1" max="100000" type="number" value={limits.maxRequestsPerSession}
              onChange={(event) => updateLimits({ maxRequestsPerSession: Number(event.target.value) })} />
          </label>
          <label>
            Request bytes
            <input min="1024" max="1048576" type="number" value={limits.maxRequestBytes}
              onChange={(event) => updateLimits({ maxRequestBytes: Number(event.target.value) })} />
          </label>
          <label>
            Response bytes
            <input min="1024" max="4194304" type="number" value={limits.maxResponseBytes}
              onChange={(event) => updateLimits({ maxResponseBytes: Number(event.target.value) })} />
          </label>
        </div>
        <button className="model-settings__save" disabled={busy || !canIssue} type="submit">
          {busy ? "Working…" : "Issue Agent grant"}
        </button>
      </form>
      {error ? <p className="model-settings__error agent-access__error" role="alert">{error}</p> : null}
    </div>
  );
}
