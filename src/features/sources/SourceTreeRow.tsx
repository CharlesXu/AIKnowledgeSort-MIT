import { useEffect, useRef } from "react";
import { Icon } from "../../ui/Icon";
import type { SelectionState, SourceNode } from "./types";

interface SourceTreeRowProps {
  readonly node: SourceNode;
  readonly depth: number;
  readonly expanded: boolean;
  readonly selectionState: SelectionState;
  readonly onToggleExpanded: (id: string) => void;
  readonly onToggleSelection: (id: string, checked: boolean) => void;
}

export function SourceTreeRow({
  node,
  depth,
  expanded,
  selectionState,
  onToggleExpanded,
  onToggleSelection,
}: SourceTreeRowProps) {
  const checkboxRef = useRef<HTMLInputElement>(null);
  const isDirectory = node.kind === "directory";
  const checkboxLabel = `Select ${node.name} ${node.kind}`;
  const disclosureLabel = `${expanded ? "Collapse" : "Expand"} ${node.name} directory`;

  useEffect(() => {
    if (checkboxRef.current) {
      checkboxRef.current.indeterminate = selectionState === "indeterminate";
    }
  }, [selectionState]);

  return (
    <div
      aria-level={depth + 1}
      className="source-tree-row"
      role="treeitem"
      style={{ "--tree-depth": depth } as React.CSSProperties}
    >
      {isDirectory ? (
        <button
          aria-expanded={expanded}
          aria-label={disclosureLabel}
          className="source-tree-row__disclosure"
          onClick={() => onToggleExpanded(node.id)}
          type="button"
        >
          <Icon name="chevron" size={13} />
        </button>
      ) : (
        <span className="source-tree-row__disclosure-spacer" aria-hidden="true" />
      )}
      <input
        aria-label={checkboxLabel}
        checked={selectionState === "checked"}
        className="source-tree-row__checkbox"
        disabled={!isDirectory && !node.eligible}
        onChange={(event) => onToggleSelection(node.id, event.target.checked)}
        ref={checkboxRef}
        type="checkbox"
      />
      <Icon name={isDirectory ? "folder" : "document"} size={15} />
      <span className="source-tree-row__name">{node.name}</span>
      {!isDirectory && !node.eligible ? (
        <span className="source-tree-row__note">Excluded</span>
      ) : null}
    </div>
  );
}
