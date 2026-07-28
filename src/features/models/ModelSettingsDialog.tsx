import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ModelConfigInput,
  ModelConfigSummary,
  ModelRuntimeClient,
  ModelRuntimeState,
} from "./types";

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
  };
}

export function ModelSettingsPanel({ client }: ModelSettingsPanelProps) {
  const [state, setState] = useState<ModelRuntimeState | null>(null);
  const [draft, setDraft] = useState<ModelConfigInput>(emptyConfig);
  const [timeoutSeconds, setTimeoutSeconds] = useState("30");
  const [busy, setBusy] = useState(false);
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
  }

  function edit(config: ModelConfigSummary): void {
    setDraft(configInput(config));
    setTimeoutSeconds(String(config.timeoutMs / 1_000));
    setError(null);
    firstFieldRef.current?.focus();
  }

  async function save(): Promise<void> {
    const seconds = Number(timeoutSeconds);
    setBusy(true);
    setError(null);
    try {
      setState(await client.upsert({
        ...draft,
        timeoutMs: Number.isFinite(seconds) ? Math.round(seconds * 1_000) : 0,
      }));
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
            <h3 id="configured-models">Configured models</h3>
            {state?.configs.length ? state.configs.map((config) => (
              <article className="model-config-row" key={config.configId}>
                <div>
                  <strong>{config.label}</strong>
                  <span>{config.location.toUpperCase()} · {config.model}</span>
                  <code>{config.endpointUrl}</code>
                  {config.credentialEnvironment ? (
                    <p>
                      Credential environment <code>{config.credentialEnvironment}</code>
                    </p>
                  ) : null}
                </div>
                <div className="model-config-row__actions">
                  <button disabled={busy} onClick={() => edit(config)} type="button">
                    Edit {config.label}
                  </button>
                  <button disabled={busy} onClick={() => void remove(config.configId)} type="button">
                    Remove {config.label}
                  </button>
                </div>
              </article>
            )) : (
              <p className="model-settings__empty">No model configurations.</p>
            )}
          </section>

          <form
            className="model-settings__form"
            onSubmit={(event) => {
              event.preventDefault();
              void save();
            }}
          >
            <h3>Add or edit</h3>
            <label>
              Configuration ID
              <input
                onChange={(event) => update({ configId: event.target.value })}
                ref={firstFieldRef}
                value={draft.configId}
              />
            </label>
            <label>
              Label
              <input
                onChange={(event) => update({ label: event.target.value })}
                value={draft.label}
              />
            </label>
            <label>
              Location
              <select
                onChange={(event) => update({
                  location: event.target.value === "remote" ? "remote" : "local",
                })}
                value={draft.location}
              >
                <option value="local">Local</option>
                <option value="remote">Remote</option>
              </select>
            </label>
            <label>
              Endpoint URL
              <input
                onChange={(event) => update({ endpointUrl: event.target.value })}
                type="url"
                value={draft.endpointUrl}
              />
            </label>
            <label>
              Model
              <input
                onChange={(event) => update({ model: event.target.value })}
                value={draft.model}
              />
            </label>
            <label>
              Timeout (seconds)
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
                onChange={(event) => update({ authenticated: event.target.checked })}
                type="checkbox"
              />
              Use bearer authentication from an environment variable
            </label>
            {draft.authenticated && draft.configId ? (
              <p className="model-settings__credential">
                Set the credential in <code>{credentialEnvironment(draft.configId)}</code>.
                The value is never accepted or displayed here.
              </p>
            ) : null}
            <button className="model-settings__save" disabled={busy} type="submit">
              {busy ? "Saving…" : "Save model config"}
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
            <span className="section-kicker">LOCAL RUNTIME</span>
            <h2 id="model-settings-title">Model runtime settings</h2>
          </div>
          <button aria-label="Close model settings" onClick={close} type="button">×</button>
        </header>
        <ModelSettingsPanel client={client} />
      </section>
    </div>
  );
}
