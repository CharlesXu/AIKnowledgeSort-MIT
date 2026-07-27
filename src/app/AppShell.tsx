import { useState } from "react";
import { demoDiscoveryProposal, demoSources } from "../data/demoSources";
import { ToolRail } from "../features/sources/ToolRail";
import { SourceTree } from "../features/sources/SourceTree";
import { ContextPane } from "../features/workbench/ContextPane";
import { DocumentPane } from "../features/workbench/DocumentPane";

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
  const [sourceWidth, setSourceWidth] = useState(286);
  const [contextWidth, setContextWidth] = useState(300);
  const layoutStyle = {
    "--source-width": `${sourceWidth}px`,
    "--context-width": `${contextWidth}px`,
  } as React.CSSProperties;

  return (
    <main
      aria-label="Source workbench"
      className="workbench"
      style={layoutStyle}
    >
      <ToolRail />
      <section
        aria-label="Sources"
        className="source-panel"
        data-collapse-at="760"
      >
        <header className="source-panel__header">
          <div>
            <p className="section-kicker">LOCAL</p>
            <h2>Sources</h2>
          </div>
          <span className="source-panel__count">6 FILES</span>
        </header>
        <SourceTree tree={demoSources} />
      </section>
      <PaneSeparator
        direction={1}
        label="Resize Sources panel"
        max={380}
        min={220}
        onChange={setSourceWidth}
        side="source"
        value={sourceWidth}
      />
      <DocumentPane proposal={demoDiscoveryProposal} />
      <PaneSeparator
        direction={-1}
        label="Resize import review context"
        max={420}
        min={260}
        onChange={setContextWidth}
        side="context"
        value={contextWidth}
      />
      <ContextPane proposal={demoDiscoveryProposal} />
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
