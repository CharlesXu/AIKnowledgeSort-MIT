import { Icon, type IconName } from "../../ui/Icon";

interface Tool {
  readonly label: string;
  readonly icon: IconName;
  readonly active?: boolean;
  readonly deferred?: boolean;
}

const primaryTools: readonly Tool[] = [
  { label: "Sources", icon: "inbox", active: true },
  { label: "Search — coming later", icon: "search", deferred: true },
  { label: "Graph — coming later", icon: "graph", deferred: true },
  { label: "Classification — coming later", icon: "layers", deferred: true },
  { label: "Archive — coming later", icon: "archive", deferred: true },
];

export function ToolRail() {
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
            aria-current={tool.active ? "page" : undefined}
            aria-label={tool.label}
            className="tool-rail__button"
            disabled={tool.deferred}
            key={tool.label}
            title={tool.label}
            type="button"
          >
            <Icon name={tool.icon} size={17} />
          </button>
        ))}
      </div>
      <button
        aria-label="Settings — coming later"
        className="tool-rail__button tool-rail__settings"
        disabled
        title="Settings — coming later"
        type="button"
      >
        <Icon name="settings" size={17} />
      </button>
    </nav>
  );
}
