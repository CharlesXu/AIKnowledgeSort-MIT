import { useEffect, useState } from "react";
import {
  statusTranslationKey,
  useI18n,
  type TranslationKey,
} from "../../i18n/I18nContext";
import type {
  AgentAccessClient,
  AgentAccessState,
  AgentResourceLimits,
  McpTransportState,
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
  const { t } = useI18n();
  const [state, setState] = useState<AgentAccessState | null>(null);
  const [selection, setSelection] = useState<NativeScopeSelection | null>(null);
  const [agentId, setAgentId] = useState("");
  const [label, setLabel] = useState("");
  const [toolIds, setToolIds] = useState<readonly string[]>([]);
  const [allowedOriginText, setAllowedOriginText] = useState("");
  const [expiryHours, setExpiryHours] = useState("1");
  const [limits, setLimits] = useState<AgentResourceLimits>(defaultLimits);
  const [issuedToken, setIssuedToken] = useState<string | null>(null);
  const [issuedGrantId, setIssuedGrantId] = useState<string | null>(null);
  const [issuedAgentId, setIssuedAgentId] = useState<string | null>(null);
  const [transport, setTransport] = useState<McpTransportState | null>(null);
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

  useEffect(() => {
    let active = true;
    client.inspectTransport()
      .then((next) => {
        if (active) setTransport(next);
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
        allowedHttpOrigins: allowedOriginText
          .split(/\r?\n/)
          .map((origin) => origin.trim())
          .filter(Boolean),
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
      setIssuedGrantId(issued.grant.grantId);
      setIssuedAgentId(issued.grant.agentId);
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
      if (issuedGrantId === grantId) dismissToken();
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function startTransport(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      setTransport(await client.startTransport({ port: 0 }));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function stopTransport(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      setTransport(await client.stopTransport());
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  function dismissToken(): void {
    setIssuedToken(null);
    setIssuedGrantId(null);
    setIssuedAgentId(null);
  }

  const directHttpTemplate = issuedToken && issuedGrantId && issuedAgentId && transport?.url
    ? [
        `URL: ${transport.url}`,
        `Authorization: Bearer ${issuedToken}`,
        `X-AIKS-Agent-Id: ${issuedAgentId}`,
        `X-AIKS-Grant-Id: ${issuedGrantId}`,
      ].join("\n")
    : null;
  const stdioTemplate = issuedToken && issuedGrantId && issuedAgentId
    && transport?.url && transport.executablePath
    ? [
        `command: ${transport.executablePath}`,
        `args: --mcp-stdio-relay --broker-url ${transport.url}`,
        `AIKS_MCP_AGENT_ID=${issuedAgentId}`,
        `AIKS_MCP_GRANT_ID=${issuedGrantId}`,
        `AIKS_MCP_GRANT_TOKEN=${issuedToken}`,
      ].join("\n")
    : null;

  const canIssue = Boolean(selection && agentId && label && toolIds.length);
  const toolTitleKeys: Readonly<Record<string, TranslationKey>> = {
    "capabilities.read": "agentAccess.toolCapabilities",
    "knowledge.read": "agentAccess.toolKnowledge",
    "graph.read": "agentAccess.toolGraph",
    "comparison.run": "agentAccess.toolComparison",
    "classification.propose": "agentAccess.toolClassification",
    "cleanup.suggest": "agentAccess.toolCleanup",
  };

  return (
    <div className="agent-access">
      <section aria-labelledby="agent-grants-heading" className="agent-access__grants">
        <div className="agent-access__section-heading">
          <div>
            <h3 id="agent-grants-heading">{t("agentAccess.grants")}</h3>
            <p>{t("agentAccess.persistence")}</p>
          </div>
          <code>{state?.toolCatalogVersion ?? "agent-tools-v1"}</code>
        </div>
        <section aria-label={t("agentAccess.broker")} className="agent-transport">
          <div>
            <strong>{t("agentAccess.broker")}</strong>
            <span>{transport?.running ? t("agentAccess.running") : t("agentAccess.stopped")}</span>
          </div>
          {transport?.url ? <code>{transport.url}</code> : null}
          {transport?.running ? (
            <button disabled={busy} onClick={() => void stopTransport()} type="button">
              {t("agentAccess.stop")}
            </button>
          ) : (
            <button disabled={busy} onClick={() => void startTransport()} type="button">
              {t("agentAccess.start")}
            </button>
          )}
        </section>
        {state?.grants.length ? state.grants.map((grant) => (
          <article className="agent-grant-row" key={grant.grantId}>
            <div className="agent-grant-row__title">
              <strong>{grant.label}</strong>
              <span className={`agent-grant-status agent-grant-status--${grant.status}`}>
                {statusTranslationKey(grant.status)
                  ? t(statusTranslationKey(grant.status)!)
                  : grant.status.toUpperCase()}
              </span>
            </div>
            <span>{grant.agentId}</span>
            <p>{grant.toolIds.join(" · ")}</p>
            {grant.allowedHttpOrigins.map((origin) => <code key={origin}>{origin}</code>)}
            {grant.scopes.map((scope) => <code key={scope.scopeId}>{scope.displayPath}</code>)}
            <small>{t("agentAccess.expires", {
              time: new Date(grant.expiresAtUnixMs).toLocaleString(),
            })}</small>
            {grant.status === "active" || grant.status === "inactive" ? (
              <button
                disabled={busy}
                onClick={() => void revoke(grant.grantId)}
                type="button"
              >
                {t("agentAccess.revoke", { label: grant.label })}
              </button>
            ) : null}
          </article>
        )) : <p className="model-settings__empty">{t("agentAccess.none")}</p>}
        {issuedToken ? (
          <div className="agent-token" role="status">
            <strong>{t("agentAccess.oneTimeToken")}</strong>
            <p>{t("agentAccess.tokenWarning")}</p>
            <code>{issuedToken}</code>
            {directHttpTemplate && stdioTemplate ? (
              <div className="agent-token__templates">
                <label>
                  {t("agentAccess.httpConfig")}
                  <textarea aria-label={t("agentAccess.httpConfig")} readOnly value={directHttpTemplate} />
                </label>
                <label>
                  {t("agentAccess.stdioConfig")}
                  <textarea aria-label={t("agentAccess.stdioConfig")} readOnly value={stdioTemplate} />
                </label>
              </div>
            ) : null}
            <button onClick={dismissToken} type="button">{t("agentAccess.dismiss")}</button>
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
        <h3>{t("agentAccess.issueTitle")}</h3>
        <label>
          {t("agentAccess.agentId")}
          <input onChange={(event) => setAgentId(event.target.value)} value={agentId} />
        </label>
        <label>
          {t("agentAccess.grantLabel")}
          <input onChange={(event) => setLabel(event.target.value)} value={label} />
        </label>
        <label htmlFor="agent-http-origins">
          {t("agentAccess.origins")}
          <textarea
            aria-describedby="agent-origin-help"
            id="agent-http-origins"
            onChange={(event) => setAllowedOriginText(event.target.value)}
            placeholder={t("agentAccess.originsPlaceholder")}
            value={allowedOriginText}
          />
        </label>
        <small id="agent-origin-help">
          {t("agentAccess.originsHelp")}
        </small>
        <div className="agent-scope-picker">
          <button disabled={busy} onClick={() => void chooseDirectories()} type="button">
            {t("agentAccess.chooseDirectories")}
          </button>
          {selection?.scopes.map((scope) => (
            <code key={scope.scopeId}>{scope.displayPath}</code>
          ))}
          {!selection ? <span>{t("agentAccess.noScope")}</span> : null}
        </div>
        <fieldset className="agent-tool-list">
          <legend>{t("agentAccess.allowedTools")}</legend>
          {state?.tools.map((tool) => {
            const titleKey = toolTitleKeys[tool.toolId];
            const localizedTitle = titleKey ? t(titleKey) : tool.title;
            return (
              <label key={tool.toolId}>
                <input
                  aria-label={localizedTitle}
                  checked={toolIds.includes(tool.toolId)}
                  onChange={() => toggleTool(tool.toolId)}
                  type="checkbox"
                />
                <span>
                  {localizedTitle}
                  <small>{tool.toolId} · {tool.effect === "read"
                    ? t("agentAccess.effectRead")
                    : t("agentAccess.effectAdvice")}</small>
                </span>
              </label>
            );
          })}
        </fieldset>
        <div className="agent-limit-grid">
          <label>
            {t("agentAccess.expiryHours")}
            <input min="0.0166667" max="720" step="any" type="number" value={expiryHours}
              onChange={(event) => setExpiryHours(event.target.value)} />
          </label>
          <label>
            {t("agentAccess.maxRequests")}
            <input min="1" max="100000" type="number" value={limits.maxRequestsPerSession}
              onChange={(event) => updateLimits({ maxRequestsPerSession: Number(event.target.value) })} />
          </label>
          <label>
            {t("agentAccess.requestBytes")}
            <input min="1024" max="1048576" type="number" value={limits.maxRequestBytes}
              onChange={(event) => updateLimits({ maxRequestBytes: Number(event.target.value) })} />
          </label>
          <label>
            {t("agentAccess.responseBytes")}
            <input min="1024" max="4194304" type="number" value={limits.maxResponseBytes}
              onChange={(event) => updateLimits({ maxResponseBytes: Number(event.target.value) })} />
          </label>
        </div>
        <button className="model-settings__save" disabled={busy || !canIssue} type="submit">
          {busy ? t("agentAccess.working") : t("agentAccess.issue")}
        </button>
      </form>
      {error ? <p className="model-settings__error agent-access__error" role="alert">{error}</p> : null}
    </div>
  );
}
