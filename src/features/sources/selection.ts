import type {
  ResolvedSelection,
  SelectionState,
  SourceNode,
} from "./types";

// Exclusions retain a selected ancestor's intent so a single-child directory
// can remain indeterminate when that child is unchecked.
const EXCLUDED_PREFIX = "!";

function isEligibleFile(node: SourceNode): boolean {
  return node.kind === "file" && node.eligible;
}

function collectNodes(
  node: SourceNode,
  id: string,
  matches: SourceNode[],
): void {
  if (node.id === id) {
    matches.push(node);
  }

  for (const child of node.children) {
    collectNodes(child, id, matches);
  }
}

function findNodes(tree: SourceNode, id: string): readonly SourceNode[] {
  const matches: SourceNode[] = [];
  collectNodes(tree, id, matches);
  return matches;
}

function collectSubtreeIds(node: SourceNode, ids: Set<string>): void {
  ids.add(node.id);
  for (const child of node.children) {
    collectSubtreeIds(child, ids);
  }
}

function selectedFileIds(
  tree: SourceNode,
  explicitIds: readonly string[],
): ReadonlySet<string> {
  return new Set(
    resolveEligibleSelection(tree, explicitIds).files.map((file) => file.id),
  );
}

function collectEligibleIds(node: SourceNode, ids: Set<string>): void {
  if (isEligibleFile(node)) {
    ids.add(node.id);
  }

  for (const child of node.children) {
    collectEligibleIds(child, ids);
  }
}

export function toggleSelection(
  tree: SourceNode,
  explicitIds: readonly string[],
  id: string,
  checked: boolean,
): readonly string[] {
  const targets = findNodes(tree, id);
  // Unknown ids are deliberately ignored so stale UI events are safe.
  if (targets.length === 0) {
    return [...explicitIds];
  }

  const subtreeIds = new Set<string>();
  for (const target of targets) {
    collectSubtreeIds(target, subtreeIds);
  }

  const retained = explicitIds.filter((explicitId) => {
    const referencedId = explicitId.startsWith(EXCLUDED_PREFIX)
      ? explicitId.slice(EXCLUDED_PREFIX.length)
      : explicitId;
    return !subtreeIds.has(referencedId);
  });

  return [...retained, checked ? id : `${EXCLUDED_PREFIX}${id}`];
}

interface TargetContext {
  readonly node: SourceNode;
  readonly selectionIntent: boolean;
}

function findTargetContexts(
  node: SourceNode,
  id: string,
  selectedIds: ReadonlySet<string>,
  excludedIds: ReadonlySet<string>,
  inheritedSelected: boolean,
  inheritedExcluded: boolean,
  contexts: TargetContext[],
): void {
  const explicitlySelected = selectedIds.has(node.id);
  const selected = inheritedSelected || explicitlySelected;
  const excluded = excludedIds.has(node.id)
    ? true
    : explicitlySelected
      ? false
      : inheritedExcluded;

  if (node.id === id) {
    contexts.push({ node, selectionIntent: selected && !excluded });
  }

  for (const child of node.children) {
    findTargetContexts(
      child,
      id,
      selectedIds,
      excludedIds,
      selected,
      excluded,
      contexts,
    );
  }
}

export function deriveSelectionState(
  tree: SourceNode,
  explicitIds: readonly string[],
  id: string,
): SelectionState {
  const selectedIds = new Set(
    explicitIds.filter((item) => !item.startsWith(EXCLUDED_PREFIX)),
  );
  const excludedIds = new Set(
    explicitIds
      .filter((item) => item.startsWith(EXCLUDED_PREFIX))
      .map((item) => item.slice(EXCLUDED_PREFIX.length)),
  );
  const contexts: TargetContext[] = [];

  findTargetContexts(
    tree,
    id,
    selectedIds,
    excludedIds,
    false,
    false,
    contexts,
  );

  if (contexts.length === 0) {
    return "unchecked";
  }

  const eligibleIds = new Set<string>();
  for (const context of contexts) {
    collectEligibleIds(context.node, eligibleIds);
  }

  if (eligibleIds.size === 0) {
    return "unchecked";
  }

  const selected = selectedFileIds(tree, explicitIds);
  const selectedCount = [...eligibleIds].filter((eligibleId) =>
    selected.has(eligibleId),
  ).length;

  if (selectedCount === eligibleIds.size) {
    return "checked";
  }

  if (
    selectedCount > 0 ||
    contexts.some((context) => context.selectionIntent)
  ) {
    return "indeterminate";
  }

  return "unchecked";
}

export function resolveEligibleSelection(
  tree: SourceNode,
  explicitIds: readonly string[],
): ResolvedSelection {
  const selectedIds = new Set(
    explicitIds.filter((item) => !item.startsWith(EXCLUDED_PREFIX)),
  );
  const excludedIds = new Set(
    explicitIds
      .filter((item) => item.startsWith(EXCLUDED_PREFIX))
      .map((item) => item.slice(EXCLUDED_PREFIX.length)),
  );
  const seen = new Set<string>();
  const files: SourceNode[] = [];

  function visit(
    node: SourceNode,
    inheritedSelected: boolean,
    inheritedExcluded: boolean,
  ): void {
    const explicitlySelected = selectedIds.has(node.id);
    const selected = inheritedSelected || explicitlySelected;
    const excluded = excludedIds.has(node.id)
      ? true
      : explicitlySelected
        ? false
        : inheritedExcluded;

    if (
      selected &&
      !excluded &&
      isEligibleFile(node) &&
      !seen.has(node.id)
    ) {
      seen.add(node.id);
      files.push(node);
    }

    for (const child of node.children) {
      visit(child, selected, excluded);
    }
  }

  visit(tree, false, false);

  return {
    explicitIds: [...explicitIds],
    files,
  };
}
