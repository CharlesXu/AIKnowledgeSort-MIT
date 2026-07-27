export interface PreparedMarkdown {
  readonly body: string;
  readonly frontmatter: readonly string[];
}

type CalloutKind = "note" | "tip" | "warning" | "danger";

const calloutPattern = /^>\s*\[!(NOTE|TIP|WARNING|DANGER)\](?:\s|$)/i;
const fencePattern = /^\s*(```+|~~~+)/;
const wikiLinkPattern = /\[\[([^\]|]+?)(?:\|([^\]]+?))?\]\]/g;
const blockReferencePattern = /(^|\s)(\^[A-Za-z0-9][\w-]*)\s*$/;

function extractFrontmatter(source: string): {
  readonly body: string;
  readonly frontmatter: readonly string[];
} {
  const lines = source.split("\n");
  if (lines[0]?.trim() !== "---") {
    return { body: source, frontmatter: Object.freeze([]) };
  }

  const closingIndex = lines.findIndex(
    (line, index) => index > 0 && line.trim() === "---",
  );
  if (closingIndex < 0) {
    return { body: source, frontmatter: Object.freeze([]) };
  }

  return {
    body: lines.slice(closingIndex + 1).join("\n"),
    frontmatter: Object.freeze(lines.slice(1, closingIndex)),
  };
}

function transformLine(line: string): string {
  const linked = line.replace(
    wikiLinkPattern,
    (_match, rawTarget: string, rawLabel: string | undefined) => {
      const target = rawTarget.trim();
      const label = rawLabel?.trim() || target;
      return `[${label}](aiks-wiki:${encodeURIComponent(target)})`;
    },
  );

  return linked.replace(
    blockReferencePattern,
    (_match, spacing: string, reference: string) =>
      `${spacing}\`${reference}\``,
  );
}

export function prepareLocalMarkdown(source: string): PreparedMarkdown {
  const extracted = extractFrontmatter(source);
  const transformed: string[] = [];
  let activeFence: string | null = null;

  for (const line of extracted.body.split("\n")) {
    const fence = fencePattern.exec(line)?.[1] ?? null;
    if (fence !== null) {
      if (activeFence === null) {
        activeFence = fence[0] ?? "`";
      } else if (fence[0] === activeFence) {
        activeFence = null;
      }
      transformed.push(line);
      continue;
    }
    transformed.push(activeFence === null ? transformLine(line) : line);
  }

  return Object.freeze({
    body: transformed.join("\n"),
    frontmatter: extracted.frontmatter,
  });
}

export function calloutKind(source: string): CalloutKind | null {
  const match = calloutPattern.exec(source);
  return match?.[1]?.toLocaleLowerCase() as CalloutKind | undefined ?? null;
}
