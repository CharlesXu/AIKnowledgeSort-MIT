import { useState } from "react";

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
  const [menuOpen, setMenuOpen] = useState(false);

  function choose(action: () => void): void {
    setMenuOpen(false);
    action();
  }

  return (
    <header
      aria-label="Application header"
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
          Local
        </span>
        <button
          aria-expanded={menuOpen}
          aria-haspopup="menu"
          aria-label="Add source"
          className="app-header__icon-button"
          disabled={addingSource}
          onClick={() => setMenuOpen((current) => !current)}
          title="Add local files or folders"
          type="button"
        >
          +
        </button>
        {menuOpen ? (
          <div
            aria-label="Add source"
            className="app-header__source-menu"
            role="menu"
          >
            <button
              onClick={() => choose(onAddFiles)}
              role="menuitem"
              type="button"
            >
              Add files…
            </button>
            <button
              onClick={() => choose(onAddFolders)}
              role="menuitem"
              type="button"
            >
              Add folders…
            </button>
          </div>
        ) : null}
      </div>
    </header>
  );
}
