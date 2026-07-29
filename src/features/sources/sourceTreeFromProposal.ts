import type { DiscoveredItem, DiscoveryProposal } from "../drop/types";
import type { SourceNode } from "./types";

function splitPath(path: string): readonly string[] {
  return path.replaceAll("\\", "/").split("/").filter(Boolean);
}

function commonDirectoryPrefix(
  items: readonly DiscoveredItem[],
): readonly string[] {
  if (items.length === 0) {
    return [];
  }

  const directories = items.map((item) => splitPath(item.path).slice(0, -1));
  const first = directories[0];
  let length = first.length;

  for (const directory of directories.slice(1)) {
    length = Math.min(length, directory.length);
    for (let index = 0; index < length; index += 1) {
      if (first[index] !== directory[index]) {
        length = index;
        break;
      }
    }
  }

  return first.slice(0, length);
}

function insertItem(
  children: readonly SourceNode[],
  directories: readonly string[],
  item: DiscoveredItem,
  proposalId: string,
  directoryPath: readonly string[] = [],
): readonly SourceNode[] {
  if (directories.length === 0) {
    return [
      ...children,
      {
        id: item.itemId,
        name: item.name,
        kind: "file",
        eligible: true,
        children: [],
      },
    ];
  }

  const [name, ...remaining] = directories;
  const nextPath = [...directoryPath, name];
  const id = `directory:${proposalId}:${nextPath.join("/")}`;
  const existingIndex = children.findIndex((child) => child.id === id);
  const existing =
    existingIndex === -1
      ? {
          id,
          name,
          kind: "directory" as const,
          eligible: false,
          children: [],
        }
      : children[existingIndex];
  const updated = {
    ...existing,
    children: insertItem(
      existing.children,
      remaining,
      item,
      proposalId,
      nextPath,
    ),
  };

  return existingIndex === -1
    ? [...children, updated]
    : children.map((child, index) => (index === existingIndex ? updated : child));
}

export function sourceTreeFromProposal(
  proposal: DiscoveryProposal,
): SourceNode {
  const commonPrefix = commonDirectoryPrefix(proposal.items);
  const rootName =
    commonPrefix.at(-1) ??
    (proposal.items.length === 0 ? "No sources" : "Reviewed sources");
  const children = proposal.items.reduce<readonly SourceNode[]>(
    (current, item) => {
      const directories = splitPath(item.path).slice(0, -1);
      return insertItem(
        current,
        directories.slice(commonPrefix.length),
        item,
        proposal.proposalId,
      );
    },
    [],
  );

  return {
    id: `proposal:${proposal.proposalId}`,
    name: rootName,
    kind: "directory",
    eligible: false,
    children,
  };
}
