import { useEffect, useState } from "react";
import { demoDiscoveryProposal, demoSources } from "../data/demoSources";
import type { DiscoveryClient } from "../features/drop/discoveryClient";
import {
  useNativeDrop,
  type NativeDropBridge,
} from "../features/drop/useNativeDrop";
import { ScanReport } from "../features/drop/ScanReport";
import { ToolRail } from "../features/sources/ToolRail";
import { SourceTree } from "../features/sources/SourceTree";
import { ContextPane } from "../features/workbench/ContextPane";
import { DocumentPane } from "../features/workbench/DocumentPane";
import { ArchivePreviewPane } from "../features/workbench/ArchivePreviewPane";
import type { ArchiveClient } from "../features/archive/types";
import type { ProfileClient } from "../features/profiles/types";
import type { NamingClient } from "../features/naming/types";
import type { KnowledgeClient, KnowledgeTarget } from "../features/knowledge/types";
import type { KnowledgeDocument } from "../features/knowledge/types";
import type { GraphClient } from "../features/graph/types";
import { Icon } from "../ui/Icon";
import { AppHeader } from "./AppHeader";
import {
  CONTEXT_WIDTH_MAX,
  CONTEXT_WIDTH_MIN,
  NAVIGATION_WIDTH_MAX,
  NAVIGATION_WIDTH_MIN,
  persistPaneLayout,
  readPaneLayout,
  type PaneLayout,
} from "./paneLayout";

interface PaneSeparatorProps {
  readonly label: string;
  readonly side: "source" | "context";
  readonly value: number;
  readonly min: number;
  readonly max: number;
  readonly direction: 1 | -1;
  readonly onChange: (value: number) => void;
}

function PaneSeparator({
  label,
  side,
  value,
  min,
  max,
  direction,
  onChange,
}: PaneSeparatorProps) {
  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>): void {
    const step = event.shiftKey ? 32 : 8;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      const physicalDelta = event.key === "ArrowRight" ? step : -step;
      onChange(Math.min(max, Math.max(min, value + physicalDelta * direction)));
      event.preventDefault();
    }
  }

  return (
    <div
      aria-label={label}
      aria-orientation="vertical"
      aria-valuemax={max}
      aria-valuemin={min}
      aria-valuenow={value}
      className={`pane-separator pane-separator--${side}`}
      onKeyDown={handleKeyDown}
      role="separator"
      tabIndex={0}
    >
      <span aria-hidden="true" />
    </div>
  );
}

interface AppShellProps {
  readonly archiveClient: ArchiveClient;
  readonly discoveryClient: DiscoveryClient;
  readonly dropBridge: NativeDropBridge;
  readonly namingClient: NamingClient;
  readonly knowledgeClient: KnowledgeClient;
  readonly graphClient: GraphClient;
  readonly profileClient: ProfileClient;
}

export function AppShell({
  archiveClient,
  discoveryClient,
  dropBridge,
  namingClient,
  knowledgeClient,
  graphClient,
  profileClient,
}: AppShellProps) {
  const [layout, setLayout] = useState<PaneLayout>(readPaneLayout);
  const [knowledgeTargets, setKnowledgeTargets] = useState<readonly KnowledgeTarget[]>([]);
  const [activeDocument, setActiveDocument] = useState<KnowledgeDocument | null>(null);
  const drop = useNativeDrop({
    bridge: dropBridge,
    discoveryClient,
    initialProposal: demoDiscoveryProposal,
  });
  const proposal = drop.proposal ?? demoDiscoveryProposal;
  const layoutStyle = {
    "--source-width": `${layout.navigationCollapsed ? 34 : layout.navigationWidth}px`,
    "--source-separator-width": layout.navigationCollapsed ? "0px" : "5px",
    "--context-width": `${layout.contextCollapsed ? 34 : layout.contextWidth}px`,
    "--context-separator-width": layout.contextCollapsed ? "0px" : "5px",
  } as React.CSSProperties;

  useEffect(() => {
    persistPaneLayout(layout);
  }, [layout]);

  function updateLayout(changes: Partial<PaneLayout>): void {
    setLayout((current) => ({ ...current, ...changes }));
  }

  return (
    <main
      aria-label="Source workbench"
      className={`workbench${drop.status === "hovering" ? " workbench--drop-hovering" : ""}`}
      onDragOver={drop.onDomDragOver}
      onDrop={drop.onDomDrop}
      style={layoutStyle}
    >
      <AppHeader />
      <ToolRail />
      <section
        aria-label="Sources"
        className={`source-panel${layout.navigationCollapsed ? " source-panel--collapsed" : ""}`}
        data-collapse-at="760"
      >
        {layout.navigationCollapsed ? (
          <button
            aria-label="Expand Sources panel"
            className="pane-restore-control"
            onClick={() => updateLayout({ navigationCollapsed: false })}
            title="Expand Sources panel"
            type="button"
          >
            <Icon name="chevron" size={14} />
          </button>
        ) : (
          <>
            <header className="source-panel__header">
              <div>
                <p className="section-kicker">LOCAL</p>
                <h2>IndexedSource</h2>
              </div>
              <div className="pane-header__actions">
                <span className="source-panel__count">6 FILES</span>
                <button
                  aria-label="Collapse Sources panel"
                  className="pane-collapse-control pane-collapse-control--left"
                  onClick={() => updateLayout({ navigationCollapsed: true })}
                  title="Collapse Sources panel"
                  type="button"
                >
                  <Icon name="chevron" size={13} />
                </button>
              </div>
            </header>
            <SourceTree tree={demoSources} />
            <ScanReport
              isDemo={drop.isDemo}
              proposal={proposal}
              status={drop.status}
              statusMessage={drop.message}
            />
          </>
        )}
      </section>
      {layout.navigationCollapsed ? null : (
        <PaneSeparator
          direction={1}
          label="Resize Sources panel"
          max={NAVIGATION_WIDTH_MAX}
          min={NAVIGATION_WIDTH_MIN}
          onChange={(navigationWidth) => updateLayout({ navigationWidth })}
          side="source"
          value={layout.navigationWidth}
        />
      )}
      <section aria-label="Knowledge workspace" className="knowledge-workspace">
        <ArchivePreviewPane
          archiveClient={archiveClient}
          namingClient={namingClient}
          onCommittedItems={(items, vault) => {
            const next = items.map((item) => ({
              authorityId: vault.authorityId,
              operationId: item.operationId,
              itemId: item.itemId,
              destinationPath: item.destinationPath,
              originalIdentity: item.identity,
            }));
            setKnowledgeTargets((current) => [
              ...current.filter((existing) =>
                !next.some((target) => target.operationId === existing.operationId)
              ),
              ...next,
            ]);
          }}
          proposal={proposal}
        />
        <DocumentPane
          client={knowledgeClient}
          onDocumentChange={setActiveDocument}
          targets={knowledgeTargets}
        />
      </section>
      {layout.contextCollapsed ? null : (
        <PaneSeparator
          direction={-1}
          label="Resize import review context"
          max={CONTEXT_WIDTH_MAX}
          min={CONTEXT_WIDTH_MIN}
          onChange={(contextWidth) => updateLayout({ contextWidth })}
          side="context"
          value={layout.contextWidth}
        />
      )}
      <ContextPane
        collapsed={layout.contextCollapsed}
        document={activeDocument}
        graphClient={graphClient}
        onCollapsedChange={(contextCollapsed) =>
          updateLayout({ contextCollapsed })
        }
        profileClient={profileClient}
        proposal={proposal}
      />
      {drop.status === "hovering" ? (
        <div
          aria-label="Native drop target"
          className="native-drop-overlay"
          role="status"
        >
          <strong>Release to review</strong>
          <span>Paths stay native; discovery starts only after a trusted grant.</span>
        </div>
      ) : null}
      <footer className="status-bar">
        <span>
          <i className="status-bar__dot" aria-hidden="true" />
          {drop.isDemo ? "Local demo workspace" : "Trusted local proposal"}
        </span>
        <span>Read-only scan report</span>
        <span className="status-bar__right">
          {proposal.counts.included} eligible · 0 changes
        </span>
      </footer>
    </main>
  );
}
