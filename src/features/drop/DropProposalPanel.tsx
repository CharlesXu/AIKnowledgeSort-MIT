import { Icon } from "../../ui/Icon";
import type { DiscoveryProposal } from "./types";

interface DropProposalPanelProps {
  readonly isDemo?: boolean;
  readonly proposal: DiscoveryProposal;
}

const countDefinitions = [
  ["Included", "included"],
  ["Excluded", "excluded"],
  ["Unreadable", "unreadable"],
  ["Symlinks", "symlink"],
  ["Out of scope", "outOfScope"],
] as const;

function formatBytes(byteSize: number): string {
  return `${(byteSize / 1024).toFixed(1)} KB`;
}

export function DropProposalPanel({
  isDemo = false,
  proposal,
}: DropProposalPanelProps) {
  return (
    <section
      aria-label="Discovery proposal"
      className="proposal"
      role="region"
    >
      <header className="proposal__header">
        <div>
          <p className="section-kicker">DISCOVERY PROPOSAL</p>
          <h2>Review discovered sources</h2>
        </div>
        <div className="proposal__badges">
          <span className="review-badge">
            {isDemo ? "Demo proposal" : "Live proposal"}
          </span>
          <span className="review-badge">Review only</span>
        </div>
      </header>

      <div aria-label="Discovery counts" className="proposal__counts">
        {countDefinitions.map(([label, key]) => (
          <div
            aria-label={label}
            className="proposal__count"
            key={key}
            role="status"
          >
            <strong>{proposal.counts[key]}</strong>
            <span>{label}</span>
          </div>
        ))}
      </div>

      <div className="proposal__table" role="table" aria-label="Included source preview">
        <div className="proposal__table-header" role="row">
          <span role="columnheader">Name</span>
          <span role="columnheader">Size</span>
          <span role="columnheader">Status</span>
        </div>
        {proposal.items.map((item) => (
          <div className="proposal__file-row" key={item.path} role="row">
            <span className="proposal__file-name" role="cell">
              <Icon name="document" size={15} />
              {item.name}
            </span>
            <span role="cell">{formatBytes(item.byteSize)}</span>
            <span className="proposal__included" role="cell">
              Included
            </span>
          </div>
        ))}
      </div>

      <div
        aria-label="Passive drop surface"
        className="proposal__drop-surface"
        role="region"
      >
        <Icon name="inbox" size={20} />
        <strong>Drop files or folders anywhere in this window</strong>
        <span>A trusted, review-only proposal will appear here</span>
      </div>

      <footer className="proposal__notice">
        <span className="proposal__notice-dot" aria-hidden="true" />
        <span>
          <strong>No files have been changed</strong>
          <small>This preview does not write, move, or archive files.</small>
        </span>
      </footer>
    </section>
  );
}
