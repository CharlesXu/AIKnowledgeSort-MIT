import { useMemo, useState } from "react";
import {
  deriveSelectionState,
  resolveEligibleSelection,
  toggleSelection,
} from "./selection";
import { Icon } from "../../ui/Icon";
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

function filterSourceTree(node: SourceNode, query: string): SourceNode | null {
  if (node.name.toLocaleLowerCase().includes(query)) {
    return node;
  }

  const matchingChildren = node.children
    .map((child) => filterSourceTree(child, query))
    .filter((child): child is SourceNode => child !== null);

  return matchingChildren.length > 0
    ? { ...node, children: matchingChildren }
    : null;
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
  const [filter, setFilter] = useState("");
  const resolved = useMemo(
    () => resolveEligibleSelection(tree, explicitIds),
    [explicitIds, tree],
  );
  const normalizedFilter = filter.trim().toLocaleLowerCase();
  const filteredTree = useMemo(
    () =>
      normalizedFilter.length > 0
        ? filterSourceTree(tree, normalizedFilter)
        : tree,
    [normalizedFilter, tree],
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
    const expanded =
      normalizedFilter.length > 0 || expandedIds.has(node.id);
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
      <div className="source-tree__filter">
        <Icon name="search" size={14} />
        <input
          aria-label="Search sources"
          onChange={(event) => setFilter(event.target.value)}
          placeholder="Filter files and folders"
          type="search"
          value={filter}
        />
      </div>
      <ul aria-label="Local source folders" role="tree">
        {filteredTree ? renderNode(filteredTree, 0) : null}
      </ul>
      {filteredTree === null ? (
        <p
          aria-label="Source filter status"
          className="source-tree__empty"
          role="status"
        >
          No sources match “{filter.trim()}”
        </p>
      ) : null}
      <p aria-live="polite" className="source-tree__summary">
        {selectedCount} unique eligible {selectedCount === 1 ? "file" : "files"}{" "}
        selected
      </p>
    </div>
  );
}
