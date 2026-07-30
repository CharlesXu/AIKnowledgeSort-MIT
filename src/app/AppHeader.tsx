import { useState } from "react";
import { useI18n } from "../i18n/I18nContext";

interface AppHeaderProps {
  readonly addingSource: boolean;
  readonly onAddFiles: () => void;
  readonly onAddFolders: () => void;
}

export function AppHeader({
  addingSource,
  onAddFiles,
  onAddFolders,
}: AppHeaderProps) {
  const { t } = useI18n();
  const [menuOpen, setMenuOpen] = useState(false);

  function choose(action: () => void): void {
    setMenuOpen(false);
    action();
  }

  return (
    <header
      aria-label={t("app.header")}
      className="app-header"
      role="banner"
    >
      <div className="app-header__brand">
        <span aria-hidden="true" className="app-header__mark">
          AK
        </span>
        <strong>AI Knowledge Sort</strong>
      </div>
      <div
        className="app-header__actions"
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            setMenuOpen(false);
          }
        }}
      >
        <span className="app-header__runtime">
          <i aria-hidden="true" />
          {t("app.local")}
        </span>
        <button
          aria-expanded={menuOpen}
          aria-haspopup="menu"
          aria-label={t("app.addSource")}
          className="app-header__icon-button"
          disabled={addingSource}
          onClick={() => setMenuOpen((current) => !current)}
          title={t("app.addSourceHint")}
          type="button"
        >
          +
        </button>
        {menuOpen ? (
          <div
            aria-label={t("app.addSource")}
            className="app-header__source-menu"
            role="menu"
          >
            <button
              onClick={() => choose(onAddFiles)}
              role="menuitem"
              type="button"
            >
              {t("app.addFiles")}
            </button>
            <button
              onClick={() => choose(onAddFolders)}
              role="menuitem"
              type="button"
            >
              {t("app.addFolders")}
            </button>
          </div>
        ) : null}
      </div>
    </header>
  );
}
