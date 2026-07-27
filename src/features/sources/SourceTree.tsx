import { useMemo, useState } from "react";
import {
  deriveSelectionState,
  resolveEligibleSelection,
  toggleSelection,
} from "./selection";
import { SourceTreeRow } from "./SourceTreeRow";
import type { SourceNode } from "./types";

interface SourceTreeProps {
  readonly tree: SourceNode;
  readonly initialSelectionIds?: readonly string[];
}

function collectDirectoryIds(node: SourceNode, ids: Set<string>): void {
  if (node.kind === "directory") {
    ids.add(node.id);
  }
  for (const child of node.children) {
    collectDirectoryIds(child, ids);
  }
}

function initiallyExpanded(tree: SourceNode): ReadonlySet<string> {
  const ids = new Set<string>();
  collectDirectoryIds(tree, ids);
  return ids;
}

export function SourceTree({
  tree,
  initialSelectionIds = [],
}: SourceTreeProps) {
  const [explicitIds, setExplicitIds] = useState<readonly string[]>(() => [
    ...initialSelectionIds,
  ]);
  const [expandedIds, setExpandedIds] = useState<ReadonlySet<string>>(() =>
    initiallyExpanded(tree),
  );
  const resolved = useMemo(
    () => resolveEligibleSelection(tree, explicitIds),
    [explicitIds, tree],
  );
  const selectedCount = resolved.files.length;

  function toggleExpanded(id: string): void {
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function toggleNode(id: string, checked: boolean): void {
    setExplicitIds((current) => toggleSelection(tree, current, id, checked));
  }

  function renderNode(node: SourceNode, depth: number): React.ReactNode {
    const expanded = expandedIds.has(node.id);
    return (
      <li key={node.id} role="none">
        <SourceTreeRow
          depth={depth}
          expanded={expanded}
          node={node}
          onToggleExpanded={toggleExpanded}
          onToggleSelection={toggleNode}
          selectionState={deriveSelectionState(tree, explicitIds, node.id)}
        />
        {node.kind === "directory" && expanded && node.children.length > 0 ? (
          <ul role="group">
            {node.children.map((child) => renderNode(child, depth + 1))}
          </ul>
        ) : null}
      </li>
    );
  }

  return (
    <div className="source-tree">
      <ul aria-label="Local source folders" role="tree">
        {renderNode(tree, 0)}
      </ul>
      <p aria-live="polite" className="source-tree__summary">
        {selectedCount} unique eligible {selectedCount === 1 ? "file" : "files"}{" "}
        selected
      </p>
    </div>
  );
}
