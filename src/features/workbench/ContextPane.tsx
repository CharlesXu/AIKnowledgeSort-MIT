import { useState } from "react";
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

interface ContextPaneProps {
  readonly collapsed: boolean;
  readonly onCollapsedChange: (collapsed: boolean) => void;
  readonly profileClient: ProfileClient;
  readonly graphClient: GraphClient;
  readonly document: KnowledgeDocument | null;
  readonly proposal: DiscoveryProposal;
  readonly modelRuntimeClient: ModelRuntimeClient;
}

export function ContextPane({
  collapsed,
  onCollapsedChange,
  profileClient,
  graphClient,
  document,
  proposal,
  modelRuntimeClient,
}: ContextPaneProps) {
  const [mode, setMode] = useState<"graph" | "review" | "agent">("graph");

  return (
    <aside
      aria-label="Import review context"
      className={`context-pane${collapsed ? " context-pane--collapsed" : ""}`}
      data-collapse-at="1440"
    >
      {collapsed ? (
        <button
          aria-label="Expand Import review context"
          className="pane-restore-control pane-restore-control--context"
          onClick={() => onCollapsedChange(false)}
          title="Expand Import review context"
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
                onClick={() => setMode("graph")}
                role="tab"
                type="button"
              >
                Knowledge Graph
              </button>
              <button
                aria-selected={mode === "review"}
                onClick={() => setMode("review")}
                role="tab"
                type="button"
              >
                Import Review
              </button>
              <button
                aria-selected={mode === "agent"}
                onClick={() => setMode("agent")}
                role="tab"
                type="button"
              >
                Agent Review
              </button>
            </div>
            <button
              aria-label="Collapse Import review context"
              className="pane-collapse-control"
              onClick={() => onCollapsedChange(true)}
              title="Collapse Import review context"
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
              <AgentReviewPane client={modelRuntimeClient} document={document} />
            )}
          </div>
        </>
      )}
    </aside>
  );
}
