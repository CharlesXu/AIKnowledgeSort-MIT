import type { DiscoveryProposal } from "../drop/types";
import { Icon } from "../../ui/Icon";

interface ArchivePreviewPaneProps {
  readonly proposal: DiscoveryProposal;
}

export function ArchivePreviewPane({ proposal }: ArchivePreviewPaneProps) {
  return (
    <section aria-label="Archive preview" className="archive-preview">
      <header className="archive-preview__header">
        <h2>Archive Preview</h2>
        <span>Uncommitted</span>
      </header>
      <div className="archive-preview__search">
        <Icon name="search" size={13} />
        <input
          aria-label="Search archive preview"
          disabled
          placeholder="Available after classification"
          type="search"
        />
      </div>
      <ul aria-label="Proposed archive tree" role="tree">
        <li role="none">
          <div aria-expanded="true" aria-level={1} role="treeitem">
            <Icon name="chevron" size={12} />
            <Icon name="folder" size={14} />
            <span>Pending review</span>
          </div>
          <ul role="group">
            {proposal.items.map((item) => (
              <li key={item.path} role="none">
                <div aria-level={2} role="treeitem">
                  <span aria-hidden="true" className="archive-preview__spacer" />
                  <Icon name="document" size={13} />
                  <span title={item.path}>{item.name}</span>
                </div>
              </li>
            ))}
          </ul>
        </li>
      </ul>
      <p className="archive-preview__notice">
        Preview only. No archive path has been created.
      </p>
    </section>
  );
}
