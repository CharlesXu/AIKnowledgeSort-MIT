import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ModelConfigInput,
  ModelConfigSummary,
  ModelRuntimeClient,
  ModelRuntimeState,
} from "./types";
import { useI18n } from "../../i18n/I18nContext";

interface ModelSettingsDialogProps {
  readonly client: ModelRuntimeClient;
  readonly onClose: () => void;
  readonly triggerRef: React.RefObject<HTMLButtonElement | null>;
}

interface ModelSettingsPanelProps {
  readonly client: ModelRuntimeClient;
}

const emptyConfig: ModelConfigInput = {
  configId: "",
  label: "",
  location: "local",
  endpointUrl: "http://127.0.0.1:11434/v1/chat/completions",
  model: "",
  timeoutMs: 30_000,
  authenticated: false,
  providerProtocol: "openAi",
  credentialSource: "environment",
  apiKey: null,
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function credentialEnvironment(configId: string): string {
  return `AIKS_MODEL_API_KEY_${configId.toUpperCase().replaceAll("-", "_")}`;
}

function configInput(config: ModelConfigSummary): ModelConfigInput {
  return {
    configId: config.configId,
    label: config.label,
    location: config.location,
    endpointUrl: config.endpointUrl,
    model: config.model,
    timeoutMs: config.timeoutMs,
    authenticated: config.authenticated,
    providerProtocol: config.providerProtocol,
    credentialSource: config.credentialSource,
    apiKey: null,
  };
}

export function ModelSettingsPanel({ client }: ModelSettingsPanelProps) {
  const { t } = useI18n();
  const [state, setState] = useState<ModelRuntimeState | null>(null);
  const [draft, setDraft] = useState<ModelConfigInput>(emptyConfig);
  const [timeoutSeconds, setTimeoutSeconds] = useState("30");
  const [busy, setBusy] = useState(false);
  const [discoveryBusy, setDiscoveryBusy] = useState(false);
  const [discoveredModels, setDiscoveredModels] = useState<readonly string[]>([]);
  const [detectedProtocol, setDetectedProtocol] = useState<string | null>(null);
  const [storedCredential, setStoredCredential] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const firstFieldRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    firstFieldRef.current?.focus();
  }, []);

  useEffect(() => {
    let active = true;
    client.inspect()
      .then((nextState) => {
        if (active) setState(nextState);
      })
      .catch((reason: unknown) => {
        if (active) setError(errorMessage(reason));
      });
    return () => {
      active = false;
    };
  }, [client]);

  function update(changes: Partial<ModelConfigInput>): void {
    setDraft((current) => ({ ...current, ...changes }));
    if (
      "endpointUrl" in changes
      || "credentialSource" in changes
      || "apiKey" in changes
    ) {
      setDiscoveredModels([]);
      setDetectedProtocol(null);
    }
  }

  function edit(config: ModelConfigSummary): void {
    setDraft(configInput(config));
    setStoredCredential(config.credentialStored);
    setDiscoveredModels([]);
    setDetectedProtocol(config.providerProtocol === "anthropic" ? "Anthropic" : "OpenAI compatible");
    setTimeoutSeconds(String(config.timeoutMs / 1_000));
    setError(null);
    firstFieldRef.current?.focus();
  }

  const refreshModels = useCallback(async (): Promise<void> => {
    const seconds = Number(timeoutSeconds);
    setDiscoveryBusy(true);
    setError(null);
    try {
      const discovered = await client.discoverModels({
        configId: draft.configId || null,
        location: draft.location,
        endpointUrl: draft.endpointUrl,
        timeoutMs: Number.isFinite(seconds) ? Math.round(seconds * 1_000) : 0,
        authenticated: draft.authenticated,
        credentialSource: draft.credentialSource,
        apiKey: draft.apiKey?.trim() || null,
      });
      setDiscoveredModels(discovered.models);
      setDetectedProtocol(
        discovered.providerProtocol === "anthropic"
          ? t("models.anthropic")
          : t("models.openai"),
      );
      setDraft((current) => ({
        ...current,
        endpointUrl: discovered.completionEndpointUrl,
        providerProtocol: discovered.providerProtocol,
        model: discovered.models.includes(current.model)
          ? current.model
          : (discovered.models[0] ?? current.model),
      }));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setDiscoveryBusy(false);
    }
  }, [
    client,
    draft.apiKey,
    draft.authenticated,
    draft.configId,
    draft.credentialSource,
    draft.endpointUrl,
    draft.location,
    timeoutSeconds,
    t,
  ]);

  useEffect(() => {
    const hasCredential = !draft.authenticated
      || (draft.credentialSource === "environment" && Boolean(draft.configId))
      || Boolean(draft.apiKey?.trim())
      || storedCredential;
    if (!draft.endpointUrl.trim() || !hasCredential) return undefined;
    const timer = window.setTimeout(() => {
      void refreshModels();
    }, 700);
    return () => window.clearTimeout(timer);
  }, [
    draft.apiKey,
    draft.authenticated,
    draft.credentialSource,
    draft.endpointUrl,
    refreshModels,
    storedCredential,
  ]);

  async function save(): Promise<void> {
    const seconds = Number(timeoutSeconds);
    setBusy(true);
    setError(null);
    try {
      setState(await client.upsert({
        ...draft,
        apiKey: draft.apiKey?.trim() || null,
        timeoutMs: Number.isFinite(seconds) ? Math.round(seconds * 1_000) : 0,
      }));
      setDraft((current) => ({ ...current, apiKey: null }));
      setStoredCredential(
        draft.authenticated && draft.credentialSource === "keychain",
      );
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function remove(configId: string): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      setState(await client.remove({ configId }));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="model-settings__body">
          <section aria-labelledby="configured-models" className="model-settings__list">
            <h3 id="configured-models">{t("models.configured")}</h3>
            {state?.configs.length ? state.configs.map((config) => (
              <article className="model-config-row" key={config.configId}>
                <div>
                  <strong>{config.label}</strong>
                  <span>{config.location.toUpperCase()} · {config.model}</span>
                  <code>{config.endpointUrl}</code>
                  <p>
                    {t("models.protocol")} <strong>
                      {config.providerProtocol === "anthropic"
                        ? t("models.anthropic")
                        : t("models.openai")}
                    </strong>
                  </p>
                  {config.credentialEnvironment ? (
                    <p>
                      {t("models.credentialEnvironment")} <code>{config.credentialEnvironment}</code>
                    </p>
                  ) : null}
                  {config.credentialStored ? <p>{t("models.credentialStored")}</p> : null}
                </div>
                <div className="model-config-row__actions">
                  <button disabled={busy} onClick={() => edit(config)} type="button">
                    {t("models.edit")} {config.label}
                  </button>
                  <button disabled={busy} onClick={() => void remove(config.configId)} type="button">
                    {t("models.remove")} {config.label}
                  </button>
                </div>
              </article>
            )) : (
              <p className="model-settings__empty">{t("models.none")}</p>
            )}
          </section>

          <form
            className="model-settings__form"
            onSubmit={(event) => {
              event.preventDefault();
              void save();
            }}
          >
            <h3>{t("models.addEdit")}</h3>
            <label>
              {t("models.configId")}
              <input
                onChange={(event) => update({ configId: event.target.value })}
                ref={firstFieldRef}
                value={draft.configId}
              />
            </label>
            <label>
              {t("models.label")}
              <input
                onChange={(event) => update({ label: event.target.value })}
                value={draft.label}
              />
            </label>
            <label>
              {t("models.location")}
              <select
                onChange={(event) => update({
                  location: event.target.value === "remote" ? "remote" : "local",
                })}
                value={draft.location}
              >
                <option value="local">{t("models.local")}</option>
                <option value="remote">{t("models.remote")}</option>
              </select>
            </label>
            <label>
              {t("models.endpoint")}
              <input
                onChange={(event) => update({ endpointUrl: event.target.value })}
                type="url"
                value={draft.endpointUrl}
              />
            </label>
            <label>
              {t("models.model")}
              {discoveredModels.length ? (
                <select
                  onChange={(event) => update({ model: event.target.value })}
                  value={draft.model}
                >
                  {discoveredModels.map((model) => (
                    <option key={model} value={model}>{model}</option>
                  ))}
                </select>
              ) : (
                <input
                  onChange={(event) => update({ model: event.target.value })}
                  value={draft.model}
                />
              )}
            </label>
            <div className="model-settings__discovery">
              <button
                disabled={discoveryBusy || !draft.endpointUrl.trim()}
                onClick={() => void refreshModels()}
                type="button"
              >
                {discoveryBusy ? t("models.refreshing") : t("models.refresh")}
              </button>
              {detectedProtocol ? <span>{t("models.detected")}: {detectedProtocol}</span> : null}
            </div>
            <label>
              {t("models.timeout")}
              <input
                max="120"
                min="1"
                onChange={(event) => setTimeoutSeconds(event.target.value)}
                type="number"
                value={timeoutSeconds}
              />
            </label>
            <label className="model-settings__checkbox">
              <input
                checked={draft.authenticated}
                onChange={(event) => update({
                  authenticated: event.target.checked,
                  credentialSource: event.target.checked
                    ? draft.credentialSource
                    : "environment",
                  apiKey: event.target.checked ? draft.apiKey : null,
                })}
                type="checkbox"
              />
              {t("models.useAuth")}
            </label>
            {draft.authenticated ? (
              <>
                <label>
                  {t("models.credentialSource")}
                  <select
                    onChange={(event) => update({
                      credentialSource: event.target.value === "keychain"
                        ? "keychain"
                        : "environment",
                      apiKey: null,
                    })}
                    value={draft.credentialSource}
                  >
                    <option value="environment">{t("models.environment")}</option>
                    <option value="keychain">{t("models.keychain")}</option>
                  </select>
                </label>
                {draft.credentialSource === "environment" ? (
                  <>
                    <label>
                      {t("models.credentialVariable")}
                      <input
                        placeholder={t("models.configIdPlaceholder")}
                        readOnly
                        value={
                          draft.configId ? credentialEnvironment(draft.configId) : ""
                        }
                      />
                    </label>
                    <p className="model-settings__credential">
                      {t("models.environmentHint")}
                    </p>
                  </>
                ) : (
                  <>
                    <label>
                      {t("models.apiKey")}
                      <input
                        autoComplete="off"
                        onChange={(event) => update({ apiKey: event.target.value })}
                        placeholder={
                          storedCredential
                            ? t("models.keyStoredPlaceholder")
                            : t("models.keyPlaceholder")
                        }
                        type="password"
                        value={draft.apiKey ?? ""}
                      />
                    </label>
                    <p className="model-settings__credential">
                      {t("models.keyHint")}
                    </p>
                  </>
                )}
              </>
            ) : null}
            <button className="model-settings__save" disabled={busy} type="submit">
              {busy ? t("models.saving") : t("models.save")}
            </button>
          </form>
      </div>
      {error ? <p className="model-settings__error" role="alert">{error}</p> : null}
    </>
  );
}

export function ModelSettingsDialog({
  client,
  onClose,
  triggerRef,
}: ModelSettingsDialogProps) {
  const { t } = useI18n();
  const close = useCallback(() => {
    onClose();
    queueMicrotask(() => triggerRef.current?.focus());
  }, [onClose, triggerRef]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close();
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [close]);

  return (
    <div className="settings-overlay" role="presentation">
      <section
        aria-labelledby="model-settings-title"
        aria-modal="true"
        className="model-settings"
        role="dialog"
      >
        <header className="model-settings__header">
          <div>
            <span className="section-kicker">{t("models.runtimeKicker")}</span>
            <h2 id="model-settings-title">{t("models.runtimeTitle")}</h2>
          </div>
          <button aria-label={t("models.close")} onClick={close} type="button">×</button>
        </header>
        <ModelSettingsPanel client={client} />
      </section>
    </div>
  );
}
