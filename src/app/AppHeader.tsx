import { Icon } from "../ui/Icon";

export function AppHeader() {
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
      <div className="app-header__actions">
        <span className="app-header__runtime">
          <i aria-hidden="true" />
          Local
        </span>
        <button
          aria-label="Add source — use drag and drop in Phase 1"
          className="app-header__icon-button"
          disabled
          title="Use native drag and drop in Phase 1"
          type="button"
        >
          +
        </button>
        <button
          aria-label="Settings — coming later"
          className="app-header__icon-button"
          disabled
          title="Settings — coming later"
          type="button"
        >
          <Icon name="settings" size={15} />
        </button>
      </div>
    </header>
  );
}
