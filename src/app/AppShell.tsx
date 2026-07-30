import { useEffect, useMemo, useRef, useState } from "react";
import { demoDiscoveryProposal, demoSources } from "../data/demoSources";
import type { DiscoveryClient } from "../features/drop/discoveryClient";
import {
  useNativeDrop,
  type NativeDropBridge,
} from "../features/drop/useNativeDrop";
import { ScanReport } from "../features/drop/ScanReport";
import {
  ToolRail,
  type WorkbenchTool,
} from "../features/sources/ToolRail";
import { SourceTree } from "../features/sources/SourceTree";
import { sourceTreeFromProposal } from "../features/sources/sourceTreeFromProposal";
import {
  ContextPane,
  type ContextMode,
} from "../features/workbench/ContextPane";
import { DocumentPane } from "../features/workbench/DocumentPane";
import { ArchivePreviewPane } from "../features/workbench/ArchivePreviewPane";
import type { ArchiveClient } from "../features/archive/types";
import type { ProfileClient } from "../features/profiles/types";
import type { NamingClient } from "../features/naming/types";
import type { KnowledgeClient, KnowledgeTarget } from "../features/knowledge/types";
import type { KnowledgeDocument } from "../features/knowledge/types";
import type { GraphClient } from "../features/graph/types";
import type { ModelRuntimeClient } from "../features/models/types";
import type { AgentAccessClient } from "../features/agentAccess/types";
import { SettingsDialog } from "../features/settings/SettingsDialog";
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
  const dragStart = useRef<{
    readonly pointerId: number;
    readonly clientX: number;
    readonly value: number;
  } | null>(null);

  function clamp(nextValue: number): number {
    return Math.min(max, Math.max(min, nextValue));
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>): void {
    const step = event.shiftKey ? 32 : 8;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      const physicalDelta = event.key === "ArrowRight" ? step : -step;
      onChange(clamp(value + physicalDelta * direction));
      event.preventDefault();
    }
  }

  function handlePointerDown(event: React.PointerEvent<HTMLDivElement>): void {
    if (event.button !== 0) {
      return;
    }
    dragStart.current = {
      pointerId: event.pointerId,
      clientX: event.clientX,
      value,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }

  function handlePointerMove(event: React.PointerEvent<HTMLDivElement>): void {
    const start = dragStart.current;
    if (start === null || start.pointerId !== event.pointerId) {
      return;
    }
    onChange(clamp(start.value + (event.clientX - start.clientX) * direction));
  }

  function finishPointerDrag(event: React.PointerEvent<HTMLDivElement>): void {
    if (dragStart.current?.pointerId !== event.pointerId) {
      return;
    }
    dragStart.current = null;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
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
      onLostPointerCapture={() => {
        dragStart.current = null;
      }}
      onPointerCancel={finishPointerDrag}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={finishPointerDrag}
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
  readonly modelRuntimeClient: ModelRuntimeClient;
  readonly agentAccessClient: AgentAccessClient;
}

export function AppShell({
  archiveClient,
  discoveryClient,
  dropBridge,
  namingClient,
  knowledgeClient,
  graphClient,
  profileClient,
  modelRuntimeClient,
  agentAccessClient,
}: AppShellProps) {
  const [layout, setLayout] = useState<PaneLayout>(readPaneLayout);
  const [knowledgeTargets, setKnowledgeTargets] = useState<readonly KnowledgeTarget[]>([]);
  const [activeDocument, setActiveDocument] = useState<KnowledgeDocument | null>(null);
  const [selectedItemIds, setSelectedItemIds] = useState<readonly string[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [activeTool, setActiveTool] = useState<WorkbenchTool | null>("sources");
  const [contextMode, setContextMode] = useState<ContextMode>("graph");
  const settingsButtonRef = useRef<HTMLButtonElement>(null);
  const sourcesRef = useRef<HTMLElement>(null);
  const archiveRef = useRef<HTMLElement>(null);
  const drop = useNativeDrop({
    bridge: dropBridge,
    discoveryClient,
    initialProposal: demoDiscoveryProposal,
  });
  const proposal = drop.proposal ?? demoDiscoveryProposal;
  const sourceTree = useMemo(
    () => drop.isDemo ? demoSources : sourceTreeFromProposal(proposal),
    [drop.isDemo, proposal],
  );
  const layoutStyle = {
    "--source-width": `${layout.navigationCollapsed ? 34 : layout.navigationWidth}px`,
    "--source-separator-width": layout.navigationCollapsed ? "0px" : "5px",
    "--context-width": `${layout.contextCollapsed ? 34 : layout.contextWidth}px`,
    "--context-separator-width": layout.contextCollapsed ? "0px" : "5px",
  } as React.CSSProperties;

  useEffect(() => {
    persistPaneLayout(layout);
  }, [layout]);

  useEffect(() => {
    setSelectedItemIds([]);
  }, [proposal.proposalId]);

  function updateLayout(changes: Partial<PaneLayout>): void {
    setLayout((current) => ({ ...current, ...changes }));
  }

  function selectTool(tool: WorkbenchTool): void {
    setActiveTool(tool);
    if (tool === "sources") {
      updateLayout({ navigationCollapsed: false });
      sourcesRef.current?.focus();
    } else if (tool === "archive") {
      archiveRef.current?.focus();
    } else {
      setContextMode(tool === "graph" ? "graph" : "review");
      updateLayout({ contextCollapsed: false });
    }
  }

  function changeContextMode(mode: ContextMode): void {
    setContextMode(mode);
    setActiveTool(
      mode === "graph" ? "graph" : mode === "review" ? "classification" : null,
    );
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
      <ToolRail
        activeTool={activeTool}
        onOpenSettings={() => setSettingsOpen(true)}
        onSelectTool={selectTool}
        settingsButtonRef={settingsButtonRef}
      />
      <section
        aria-label="Sources"
        className={`source-panel${layout.navigationCollapsed ? " source-panel--collapsed" : ""}`}
        data-collapse-at="760"
        ref={sourcesRef}
        tabIndex={-1}
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
                <span className="source-panel__count">
                  {proposal.items.length} FILES
                </span>
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
            <SourceTree
              key={sourceTree.id}
              onSelectedFileIdsChange={setSelectedItemIds}
              selectedFileIds={selectedItemIds}
              tree={sourceTree}
            />
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
          focusRef={archiveRef}
          modelRuntimeClient={modelRuntimeClient}
          namingClient={namingClient}
          profileClient={profileClient}
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
          onUndoneOperation={(operationId) => {
            setKnowledgeTargets((current) =>
              current.filter((target) => target.operationId !== operationId),
            );
          }}
          onSelectedItemIdsChange={setSelectedItemIds}
          proposal={proposal}
          selectedItemIds={selectedItemIds}
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
        mode={contextMode}
        modelRuntimeClient={modelRuntimeClient}
        onCollapsedChange={(contextCollapsed) =>
          updateLayout({ contextCollapsed })
        }
        onModeChange={changeContextMode}
        profileClient={profileClient}
        proposal={proposal}
      />
      {settingsOpen ? (
        <SettingsDialog
          agentAccessClient={agentAccessClient}
          modelRuntimeClient={modelRuntimeClient}
          onClose={() => setSettingsOpen(false)}
          triggerRef={settingsButtonRef}
        />
      ) : null}
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
