import type { DiscoveryProposal } from "../drop/types";

interface ContextPaneProps {
  readonly proposal: DiscoveryProposal;
}

export function ContextPane({ proposal }: ContextPaneProps) {
  const firstItem = proposal.items[0];

  return (
    <aside
      aria-label="Import review context"
      className="context-pane"
      data-collapse-at="1120"
    >
      <header className="pane-header">
        <div>
          <p className="section-kicker">IMPORT REVIEW</p>
          <h2>Proposal context</h2>
        </div>
      </header>
      <div className="context-pane__body">
        <section className="context-section" aria-labelledby="review-scope">
          <h3 id="review-scope">Review scope</h3>
          <dl className="detail-list">
            <div>
              <dt>Mode</dt>
              <dd>Local preview</dd>
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
        <section className="context-section" aria-labelledby="first-source">
          <h3 id="first-source">First included source</h3>
          <p className="context-file">{firstItem?.name ?? "No included source"}</p>
          <p className="context-path">{firstItem?.path ?? "—"}</p>
        </section>
        <section className="context-section context-section--deferred" aria-labelledby="later-tools">
          <h3 id="later-tools">Later workflows</h3>
          <p>
            Markdown editing, graph views, classification, archive workflows,
            and MCP connections are not available in Phase 1.
          </p>
        </section>
      </div>
    </aside>
  );
}
