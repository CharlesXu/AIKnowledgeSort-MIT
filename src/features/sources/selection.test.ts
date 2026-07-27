import { describe, expect, test } from "vitest";
import {
  deriveSelectionState,
  resolveEligibleSelection,
  toggleSelection,
} from "./selection";
import type { SourceNode } from "./types";

const one: SourceNode = {
  id: "root/a/one.txt",
  name: "one.txt",
  kind: "file",
  eligible: true,
  children: [],
};

const two: SourceNode = {
  id: "root/a/sub/two.txt",
  name: "two.txt",
  kind: "file",
  eligible: true,
  children: [],
};

const ineligible: SourceNode = {
  id: "root/a/ignored.bin",
  name: "ignored.bin",
  kind: "file",
  eligible: false,
  children: [],
};

const tree: SourceNode = {
  id: "root",
  name: "root",
  kind: "directory",
  eligible: false,
  children: [
    {
      id: "root/a",
      name: "a",
      kind: "directory",
      eligible: false,
      children: [
        one,
        {
          id: "root/a/sub",
          name: "sub",
          kind: "directory",
          eligible: false,
          children: [two],
        },
        ineligible,
      ],
    },
  ],
};

describe("source selection", () => {
  test("selecting a directory resolves its unique eligible leaves", () => {
    const explicitIds = toggleSelection(tree, [], "root/a", true);

    expect(resolveEligibleSelection(tree, explicitIds).files).toEqual([one, two]);
    expect(deriveSelectionState(tree, explicitIds, "root/a")).toBe("checked");
  });

  test("deselecting a selected leaf makes its ancestors indeterminate", () => {
    const selected = toggleSelection(tree, [], "root/a", true);
    const withoutTwo = toggleSelection(
      tree,
      selected,
      "root/a/sub/two.txt",
      false,
    );

    expect(resolveEligibleSelection(tree, withoutTwo).files).toEqual([one]);
    expect(deriveSelectionState(tree, withoutTwo, "root/a/sub")).toBe(
      "indeterminate",
    );
    expect(deriveSelectionState(tree, withoutTwo, "root/a")).toBe(
      "indeterminate",
    );
  });

  test("reselecting a leaf makes its ancestors checked again", () => {
    const selected = toggleSelection(tree, [], "root/a", true);
    const withoutTwo = toggleSelection(
      tree,
      selected,
      "root/a/sub/two.txt",
      false,
    );
    const restored = toggleSelection(
      tree,
      withoutTwo,
      "root/a/sub/two.txt",
      true,
    );

    expect(deriveSelectionState(tree, restored, "root/a/sub")).toBe("checked");
    expect(deriveSelectionState(tree, restored, "root/a")).toBe("checked");
  });

  test("mixed explicit selections remain unchanged while resolving deduplicated files", () => {
    const explicitIds = ["root/a", "root/a/one.txt"];

    const resolved = resolveEligibleSelection(tree, explicitIds);

    expect(resolved.explicitIds).toEqual(explicitIds);
    expect(resolved.files).toEqual([one, two]);
    expect(explicitIds).toEqual(["root/a", "root/a/one.txt"]);
  });

  test("ineligible leaves are excluded", () => {
    const resolved = resolveEligibleSelection(tree, [
      "root/a",
      "root/a/ignored.bin",
    ]);

    expect(resolved.files).toEqual([one, two]);
  });

  test("duplicate ids and repeated tree references resolve by stable identity once", () => {
    const aliasedTree: SourceNode = {
      ...tree,
      children: [...tree.children, one],
    };

    const resolved = resolveEligibleSelection(aliasedTree, [
      "root/a/one.txt",
      "root/a/one.txt",
      "root",
    ]);

    expect(resolved.files.map((file) => file.id)).toEqual([
      "root/a/one.txt",
      "root/a/sub/two.txt",
    ]);
  });

  test("unknown ids are safe no-ops", () => {
    const explicitIds = ["root/a/one.txt"];

    expect(toggleSelection(tree, explicitIds, "missing", true)).toEqual(
      explicitIds,
    );
    expect(deriveSelectionState(tree, explicitIds, "missing")).toBe(
      "unchecked",
    );
    expect(
      resolveEligibleSelection(tree, [...explicitIds, "missing"]).files,
    ).toEqual([one]);
  });

  test("operations do not mutate the tree or explicit ids", () => {
    const explicitIds = ["root/a"];
    const treeSnapshot = structuredClone(tree);

    toggleSelection(tree, explicitIds, "root/a/sub/two.txt", false);
    deriveSelectionState(tree, explicitIds, "root/a");
    resolveEligibleSelection(tree, explicitIds);

    expect(tree).toEqual(treeSnapshot);
    expect(explicitIds).toEqual(["root/a"]);
  });
});
