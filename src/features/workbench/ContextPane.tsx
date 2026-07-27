import type { DiscoveryProposal } from "../drop/types";
import { Icon } from "../../ui/Icon";

interface ContextPaneProps {
  readonly collapsed: boolean;
  readonly isDemo: boolean;
  readonly onCollapsedChange: (collapsed: boolean) => void;
  readonly proposal: DiscoveryProposal;
}

const statusDefinitions = [
  ["Included", "included"],
  ["Excluded", "excluded"],
  ["Unreadable", "unreadable"],
  ["Symlink", "symlink"],
  ["Out of scope", "outOfScope"],
] as const;

export function ContextPane({
  collapsed,
  isDemo,
  onCollapsedChange,
  proposal,
}: ContextPaneProps) {
  const firstItem = proposal.items[0];

  return (
    <aside
      aria-label="Import review context"
      className={`context-pane${collapsed ? " context-pane--collapsed" : ""}`}
      data-collapse-at="1120"
    >
      {collapsed ? (
        <button
          aria-label="Expand Import review context"
          className="pane-restore-control pane-restore-control--context"
          onClick={() => onCollapsedChange(false)}
          title="Expand Import review context"
          type="button"
        >
          <Icon name="chevron" size={14} />
        </button>
      ) : (
        <>
          <header className="pane-header">
            <div>
              <p className="section-kicker">IMPORT REVIEW</p>
              <h2>Proposal context</h2>
            </div>
            <button
              aria-label="Collapse Import review context"
              className="pane-collapse-control"
              onClick={() => onCollapsedChange(true)}
              title="Collapse Import review context"
              type="button"
            >
              <Icon name="chevron" size={13} />
            </button>
          </header>
          <div className="context-pane__body">
            <section className="context-section" aria-labelledby="review-scope">
              <h3 id="review-scope">Review scope</h3>
              <dl className="detail-list">
                <div>
                  <dt>Mode</dt>
                  <dd>{isDemo ? "Demo fixture" : "Trusted local preview"}</dd>
                </div>
                <div>
                  <dt>Eligible</dt>
                  <dd>{proposal.counts.included} files</dd>
                </div>
                <div>
                  <dt>Mutation</dt>
                  <dd>None</dd>
                </div>
              </dl>
            </section>
            <section
              className="context-section context-section--statuses"
              aria-labelledby="proposal-statuses"
            >
              <h3 id="proposal-statuses">Proposal status</h3>
              <ul
                aria-label="Proposal status counts"
                className="context-status-list"
              >
                {statusDefinitions.map(([label, key]) => (
                  <li
                    aria-label={label}
                    className={`context-status-row context-status-row--${key}`}
                    key={key}
                  >
                    <span className="context-status-row__label">
                      <i aria-hidden="true" />
                      {label}
                    </span>
                    <strong>{proposal.counts[key]}</strong>
                  </li>
                ))}
              </ul>
            </section>
            <section className="context-section" aria-labelledby="first-source">
              <h3 id="first-source">First included source</h3>
              <p className="context-file">
                {firstItem?.name ?? "No included source"}
              </p>
              <p className="context-path">{firstItem?.path ?? "—"}</p>
            </section>
            <section
              className="context-section context-section--deferred"
              aria-labelledby="later-tools"
            >
              <h3 id="later-tools">Later workflows</h3>
              <p>
                Markdown editing, graph views, classification, archive
                workflows, and MCP connections are not available in Phase 1.
              </p>
            </section>
          </div>
        </>
      )}
    </aside>
  );
}
