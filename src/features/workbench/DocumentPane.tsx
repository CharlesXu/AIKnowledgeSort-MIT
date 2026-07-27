import type { DiscoveryProposal } from "../drop/types";
import { DropProposalPanel } from "../drop/DropProposalPanel";

interface DocumentPaneProps {
  readonly proposal: DiscoveryProposal;
}

export function DocumentPane({ proposal }: DocumentPaneProps) {
  return (
    <section aria-label="Discovery review" className="document-pane">
      <header className="pane-header document-pane__header">
        <div>
          <p className="pane-header__path">Local workspace / Incoming</p>
          <h1>Discovery review</h1>
        </div>
        <span className="pane-header__mode">READ-ONLY PROPOSAL</span>
      </header>
      <div className="document-pane__body">
        <div className="document-pane__intro">
          <p>
            A deterministic preview of eligible local sources. Native drop
            capture is not connected in this phase.
          </p>
          <span>Generated demo data</span>
        </div>
        <DropProposalPanel proposal={proposal} />
      </div>
    </section>
  );
}
