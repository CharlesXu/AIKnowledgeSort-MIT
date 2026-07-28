import { useCallback, useEffect, useState } from "react";
import { AgentAccessPanel } from "../agentAccess/AgentAccessPanel";
import type { AgentAccessClient } from "../agentAccess/types";
import { ModelSettingsPanel } from "../models/ModelSettingsDialog";
import type { ModelRuntimeClient } from "../models/types";

interface SettingsDialogProps {
  readonly agentAccessClient: AgentAccessClient;
  readonly modelRuntimeClient: ModelRuntimeClient;
  readonly onClose: () => void;
  readonly triggerRef: React.RefObject<HTMLButtonElement | null>;
}

type SettingsTab = "models" | "agents";

export function SettingsDialog({
  agentAccessClient,
  modelRuntimeClient,
  onClose,
  triggerRef,
}: SettingsDialogProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("models");
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
            <span className="section-kicker">LOCAL AUTHORITY</span>
            <h2 id="settings-title">Settings</h2>
          </div>
          <button aria-label="Close settings" onClick={close} type="button">×</button>
        </header>
        <div aria-label="Settings sections" className="settings-tabs" role="tablist">
          <button
            aria-controls="model-runtime-panel"
            aria-selected={activeTab === "models"}
            id="model-runtime-tab"
            onClick={() => setActiveTab("models")}
            role="tab"
            type="button"
          >
            Model runtime
          </button>
          <button
            aria-controls="agent-access-panel"
            aria-selected={activeTab === "agents"}
            id="agent-access-tab"
            onClick={() => setActiveTab("agents")}
            role="tab"
            type="button"
          >
            Agent access
          </button>
        </div>
        <div
          aria-labelledby={activeTab === "models" ? "model-runtime-tab" : "agent-access-tab"}
          className="settings-dialog__panel"
          id={activeTab === "models" ? "model-runtime-panel" : "agent-access-panel"}
          role="tabpanel"
        >
          {activeTab === "models" ? (
            <ModelSettingsPanel client={modelRuntimeClient} />
          ) : (
            <AgentAccessPanel client={agentAccessClient} />
          )}
        </div>
      </section>
    </div>
  );
}
