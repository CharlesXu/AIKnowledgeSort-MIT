import { useCallback, useEffect, useState } from "react";
import { AgentAccessPanel } from "../agentAccess/AgentAccessPanel";
import type { AgentAccessClient } from "../agentAccess/types";
import { ModelSettingsPanel } from "../models/ModelSettingsDialog";
import type { ModelRuntimeClient } from "../models/types";
import { useI18n } from "../../i18n/I18nContext";

interface SettingsDialogProps {
  readonly agentAccessClient: AgentAccessClient;
  readonly modelRuntimeClient: ModelRuntimeClient;
  readonly onClose: () => void;
  readonly triggerRef: React.RefObject<HTMLButtonElement | null>;
}

type SettingsTab = "models" | "agents" | "language";

export function SettingsDialog({
  agentAccessClient,
  modelRuntimeClient,
  onClose,
  triggerRef,
}: SettingsDialogProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("models");
  const { language, setLanguage, t } = useI18n();
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
        aria-labelledby="settings-title"
        aria-modal="true"
        className="model-settings settings-dialog"
        role="dialog"
      >
        <header className="model-settings__header">
          <div>
            <span className="section-kicker">{t("settings.kicker")}</span>
            <h2 id="settings-title">{t("settings.title")}</h2>
          </div>
          <button aria-label={t("settings.close")} onClick={close} type="button">×</button>
        </header>
        <div aria-label={t("settings.sections")} className="settings-tabs" role="tablist">
          <button
            aria-controls="model-runtime-panel"
            aria-selected={activeTab === "models"}
            id="model-runtime-tab"
            onClick={() => setActiveTab("models")}
            role="tab"
            type="button"
          >
            {t("settings.models")}
          </button>
          <button
            aria-controls="agent-access-panel"
            aria-selected={activeTab === "agents"}
            id="agent-access-tab"
            onClick={() => setActiveTab("agents")}
            role="tab"
            type="button"
          >
            {t("settings.agents")}
          </button>
          <button
            aria-controls="language-panel"
            aria-selected={activeTab === "language"}
            id="language-tab"
            onClick={() => setActiveTab("language")}
            role="tab"
            type="button"
          >
            {t("settings.language")}
          </button>
        </div>
        <div
          aria-labelledby={
            activeTab === "models"
              ? "model-runtime-tab"
              : activeTab === "agents"
                ? "agent-access-tab"
                : "language-tab"
          }
          className="settings-dialog__panel"
          id={
            activeTab === "models"
              ? "model-runtime-panel"
              : activeTab === "agents"
                ? "agent-access-panel"
                : "language-panel"
          }
          role="tabpanel"
        >
          {activeTab === "models" ? (
            <ModelSettingsPanel client={modelRuntimeClient} />
          ) : activeTab === "agents" ? (
            <AgentAccessPanel client={agentAccessClient} />
          ) : (
            <section className="language-settings">
              <h3>{t("language.title")}</h3>
              <p>{t("language.description")}</p>
              <label>
                {t("language.title")}
                <select
                  onChange={(event) => setLanguage(
                    event.target.value === "zh-CN" ? "zh-CN" : "en",
                  )}
                  value={language}
                >
                  <option value="en">{t("language.english")}</option>
                  <option value="zh-CN">{t("language.chinese")}</option>
                </select>
              </label>
            </section>
          )}
        </div>
      </section>
    </div>
  );
}
