import { Icon } from "../../ui/Icon";
import type { DiscoveryProposal } from "./types";

interface DropProposalPanelProps {
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

export function DropProposalPanel({ proposal }: DropProposalPanelProps) {
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
        <span className="review-badge">Review only</span>
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

      <footer className="proposal__notice">
        <span className="proposal__notice-dot" aria-hidden="true" />
        <span>
          <strong>No files have been changed</strong>
          <small>This in-memory preview does not write, move, or archive files.</small>
        </span>
      </footer>
    </section>
  );
}
