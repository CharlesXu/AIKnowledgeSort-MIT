import { Icon, type IconName } from "../../ui/Icon";

interface Tool {
  readonly id?: WorkbenchTool;
  readonly label: string;
  readonly icon: IconName;
  readonly deferred?: boolean;
}

export type WorkbenchTool =
  | "sources"
  | "search"
  | "graph"
  | "classification"
  | "archive";

const primaryTools: readonly Tool[] = [
  { id: "sources", label: "Sources", icon: "inbox" },
  { id: "search", label: "Search", icon: "search" },
  { id: "graph", label: "Graph", icon: "graph" },
  { id: "classification", label: "Classification", icon: "layers" },
  { id: "archive", label: "Archive", icon: "archive" },
];

interface ToolRailProps {
  readonly activeTool: WorkbenchTool | null;
  readonly onOpenSettings: () => void;
  readonly onSelectTool: (tool: WorkbenchTool) => void;
  readonly settingsButtonRef: React.RefObject<HTMLButtonElement | null>;
}

export function ToolRail({
  activeTool,
  onOpenSettings,
  onSelectTool,
  settingsButtonRef,
}: ToolRailProps) {
  return (
    <nav
      aria-label="Workbench tools"
      className="tool-rail"
      data-width="44"
      role="toolbar"
    >
      <div className="tool-rail__tools">
        {primaryTools.map((tool) => (
          <button
            aria-current={tool.id === activeTool ? "page" : undefined}
            aria-label={tool.label}
            className="tool-rail__button"
            disabled={tool.deferred}
            key={tool.label}
            onClick={
              tool.id === undefined ? undefined : () => onSelectTool(tool.id!)
            }
            title={tool.label}
            type="button"
          >
            <Icon name={tool.icon} size={17} />
          </button>
        ))}
      </div>
      <button
        aria-label="Settings"
        className="tool-rail__button tool-rail__settings"
        onClick={onOpenSettings}
        ref={settingsButtonRef}
        title="Settings"
        type="button"
      >
        <Icon name="settings" size={17} />
      </button>
    </nav>
  );
}
