import { useEffect, useState } from "react";
import { demoDiscoveryProposal, demoSources } from "../data/demoSources";
import { ToolRail } from "../features/sources/ToolRail";
import { SourceTree } from "../features/sources/SourceTree";
import { ContextPane } from "../features/workbench/ContextPane";
import { DocumentPane } from "../features/workbench/DocumentPane";
import { Icon } from "../ui/Icon";
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

export function AppShell() {
  const [layout, setLayout] = useState<PaneLayout>(readPaneLayout);
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
      className="workbench"
      style={layoutStyle}
    >
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
                <h2>Sources</h2>
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
      <DocumentPane proposal={demoDiscoveryProposal} />
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
        onCollapsedChange={(contextCollapsed) =>
          updateLayout({ contextCollapsed })
        }
        proposal={demoDiscoveryProposal}
      />
      <footer className="status-bar">
        <span>
          <i className="status-bar__dot" aria-hidden="true" />
          Local demo workspace
        </span>
        <span>Read-only discovery proposal</span>
        <span className="status-bar__right">3 eligible · 0 changes</span>
      </footer>
    </main>
  );
}
