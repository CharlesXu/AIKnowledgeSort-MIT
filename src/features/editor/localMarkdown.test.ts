import { describe, expect, test } from "vitest";
import { calloutKind, prepareLocalMarkdown } from "./localMarkdown";

describe("prepareLocalMarkdown", () => {
  test("extracts only leading frontmatter", () => {
    expect(
      prepareLocalMarkdown("---\ntitle: Demo\ntags: [local]\n---\nBody"),
    ).toEqual({
      frontmatter: ["title: Demo", "tags: [local]"],
      body: "Body",
    });

    expect(prepareLocalMarkdown("Body\n---\ntitle: Not metadata\n---")).toEqual({
      frontmatter: [],
      body: "Body\n---\ntitle: Not metadata\n---",
    });
  });

  test("converts wiki links and block references outside fences", () => {
    const prepared = prepareLocalMarkdown(
      "[[MCU reset|Reset note]] and [[Reliability]]\n\nEvidence paragraph ^evidence-1",
    );

    expect(prepared.body).toContain("[Reset note](aiks-wiki:MCU%20reset)");
    expect(prepared.body).toContain(
      "[Reliability](aiks-wiki:Reliability)",
    );
    expect(prepared.body).toContain("Evidence paragraph `^evidence-1`");
  });

  test("keeps fenced source byte-for-byte unchanged", () => {
    const source = [
      "[[Outside]]",
      "",
      "```markdown",
      "[[Inside|Example]]",
      "Block ^inside",
      "```",
      "",
      "After ^outside",
    ].join("\n");
    const prepared = prepareLocalMarkdown(source);

    expect(prepared.body).toContain("[Outside](aiks-wiki:Outside)");
    expect(prepared.body).toContain(
      "```markdown\n[[Inside|Example]]\nBlock ^inside\n```",
    );
    expect(prepared.body).toContain("After `^outside`");
  });

  test("does not mutate the source or returned metadata", () => {
    const source = "---\ntitle: Demo\n---\n[[Note]]";
    const prepared = prepareLocalMarkdown(source);

    expect(source).toBe("---\ntitle: Demo\n---\n[[Note]]");
    expect(Object.isFrozen(prepared.frontmatter)).toBe(true);
  });
});

describe("calloutKind", () => {
  test.each([
    ["> [!NOTE]\n> Details", "note"],
    ["> [!TIP] Optional title\n> Details", "tip"],
    ["> [!WARNING]\n> Verify the source", "warning"],
    ["> [!DANGER]\n> Stop", "danger"],
    ["> Ordinary quote", null],
  ] as const)("maps %s to %s", (source, expected) => {
    expect(calloutKind(source)).toBe(expected);
  });
});
