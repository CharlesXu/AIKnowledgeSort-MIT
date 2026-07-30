import { Icon, type IconName } from "../../ui/Icon";
import {
  useI18n,
  type TranslationKey,
} from "../../i18n/I18nContext";

interface Tool {
  readonly id?: WorkbenchTool;
  readonly labelKey: TranslationKey;
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
  { id: "sources", labelKey: "tools.sources", icon: "inbox" },
  { id: "search", labelKey: "tools.search", icon: "search" },
  { id: "graph", labelKey: "tools.graph", icon: "graph" },
  { id: "classification", labelKey: "tools.classification", icon: "layers" },
  { id: "archive", labelKey: "tools.archive", icon: "archive" },
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
  const { t } = useI18n();
  return (
    <nav
      aria-label={t("tools.label")}
      className="tool-rail"
      data-width="44"
      role="toolbar"
    >
      <div className="tool-rail__tools">
        {primaryTools.map((tool) => (
          <button
            aria-current={tool.id === activeTool ? "page" : undefined}
            aria-label={t(tool.labelKey)}
            className="tool-rail__button"
            disabled={tool.deferred}
            key={tool.labelKey}
            onClick={
              tool.id === undefined ? undefined : () => onSelectTool(tool.id!)
            }
            title={t(tool.labelKey)}
            type="button"
          >
            <Icon name={tool.icon} size={17} />
          </button>
        ))}
      </div>
      <button
        aria-label={t("tools.settings")}
        className="tool-rail__button tool-rail__settings"
        onClick={onOpenSettings}
        ref={settingsButtonRef}
        title={t("tools.settings")}
        type="button"
      >
        <Icon name="settings" size={17} />
      </button>
    </nav>
  );
}
