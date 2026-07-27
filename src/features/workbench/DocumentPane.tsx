import type { DiscoveryProposal } from "../drop/types";
import { DropProposalPanel } from "../drop/DropProposalPanel";
import type { NativeDropStatus } from "../drop/useNativeDrop";

interface DocumentPaneProps {
  readonly isDemo: boolean;
  readonly proposal: DiscoveryProposal;
  readonly status: NativeDropStatus;
  readonly statusMessage: string;
}

export function DocumentPane({
  isDemo,
  proposal,
  status,
  statusMessage,
}: DocumentPaneProps) {
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
            {isDemo
              ? "A deterministic browser fixture for reviewing the read-only discovery layout."
              : "A trusted native discovery proposal generated from an opaque local drop grant."}
          </p>
          <span>{isDemo ? "Browser preview" : "Native grant result"}</span>
        </div>
        {status === "loading" ||
        status === "error" ||
        status === "ignored" ||
        status === "ready" ? (
          <p
            aria-label="Drop status"
            className={`drop-status drop-status--${status}`}
            role="status"
          >
            {statusMessage}
          </p>
        ) : null}
        <DropProposalPanel isDemo={isDemo} proposal={proposal} />
      </div>
    </section>
  );
}
