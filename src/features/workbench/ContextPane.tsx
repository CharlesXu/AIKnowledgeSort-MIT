import type { DiscoveryProposal } from "../drop/types";
import type { ProfileClient } from "../profiles/types";
import { ProfileReview } from "../profiles/ProfileReview";
import { Icon } from "../../ui/Icon";
import { ProposalTopology } from "./ProposalTopology";
import { KnowledgeGraphPane } from "./KnowledgeGraphPane";
import type { GraphClient } from "../graph/types";
import type { KnowledgeDocument } from "../knowledge/types";
import type { ModelRuntimeClient } from "../models/types";
import { AgentReviewPane } from "../models/AgentReviewPane";
import { useI18n } from "../../i18n/I18nContext";

interface ContextPaneProps {
  readonly collapsed: boolean;
  readonly mode: ContextMode;
  readonly onCollapsedChange: (collapsed: boolean) => void;
  readonly onModeChange: (mode: ContextMode) => void;
  readonly profileClient: ProfileClient;
  readonly graphClient: GraphClient;
  readonly document: KnowledgeDocument | null;
  readonly proposal: DiscoveryProposal;
  readonly modelRuntimeClient: ModelRuntimeClient;
}

export type ContextMode = "graph" | "review" | "agent";

export function ContextPane({
  collapsed,
  mode,
  onCollapsedChange,
  onModeChange,
  profileClient,
  graphClient,
  document,
  proposal,
  modelRuntimeClient,
}: ContextPaneProps) {
  const { t } = useI18n();
  return (
    <aside
      aria-label={t("context.label")}
      className={`context-pane${collapsed ? " context-pane--collapsed" : ""}`}
      data-collapse-at="1440"
    >
      {collapsed ? (
        <button
          aria-label={t("context.expand")}
          className="pane-restore-control pane-restore-control--context"
          onClick={() => onCollapsedChange(false)}
          title={t("context.expand")}
          type="button"
        >
          <Icon name="chevron" size={14} />
        </button>
      ) : (
        <>
          <header className="pane-header context-pane__header">
            <div className="context-pane__tabs" role="tablist">
              <button
                aria-selected={mode === "graph"}
                onClick={() => onModeChange("graph")}
                role="tab"
                type="button"
              >
                {t("context.graph")}
              </button>
              <button
                aria-selected={mode === "review"}
                onClick={() => onModeChange("review")}
                role="tab"
                type="button"
              >
                {t("context.importReview")}
              </button>
              <button
                aria-selected={mode === "agent"}
                onClick={() => onModeChange("agent")}
                role="tab"
                type="button"
              >
                {t("context.agentReview")}
              </button>
            </div>
            <button
              aria-label={t("context.collapse")}
              className="pane-collapse-control"
              onClick={() => onCollapsedChange(true)}
              title={t("context.collapse")}
              type="button"
            >
              <Icon name="chevron" size={13} />
            </button>
          </header>
          <div className="context-pane__body">
            {mode === "graph" ? (
              document === null ? (
                <ProposalTopology proposal={proposal} />
              ) : (
                <KnowledgeGraphPane client={graphClient} document={document} />
              )
            ) : mode === "review" ? (
              <ProfileReview client={profileClient} />
            ) : (
              <AgentReviewPane
                client={modelRuntimeClient}
                document={document}
                graphClient={graphClient}
                onGraphImported={() => onModeChange("graph")}
              />
            )}
          </div>
        </>
      )}
    </aside>
  );
}
