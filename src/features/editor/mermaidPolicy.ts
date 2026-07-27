import DOMPurify from "dompurify";

export const MAX_MERMAID_SOURCE_LENGTH = 50_000;

export type MermaidValidation =
  | { readonly ok: true }
  | { readonly ok: false; readonly message: string };

const configurationDirective =
  /%%\{\s*(?:init|initialize|config)\s*:/i;
const clickDirective = /^\s*click(?:\s|$)/im;

export function validateMermaidSource(source: string): MermaidValidation {
  if (source.trim().length === 0) {
    return { ok: false, message: "The Mermaid diagram is empty." };
  }
  if (source.length > MAX_MERMAID_SOURCE_LENGTH) {
    return {
      ok: false,
      message: `The Mermaid diagram is too large (maximum ${MAX_MERMAID_SOURCE_LENGTH} characters).`,
    };
  }
  if (configurationDirective.test(source)) {
    return {
      ok: false,
      message: "Mermaid configuration directives are disabled in local preview.",
    };
  }
  if (clickDirective.test(source)) {
    return {
      ok: false,
      message: "Mermaid click directives are disabled in local preview.",
    };
  }
  return { ok: true };
}

export function sanitizeMermaidSvg(svg: string): string {
  return String(
    DOMPurify.sanitize(svg, {
      USE_PROFILES: { svg: true, svgFilters: true },
      FORBID_TAGS: ["script", "style", "foreignObject", "image", "a"],
      FORBID_ATTR: ["href", "xlink:href", "style"],
      SANITIZE_NAMED_PROPS: true,
    }),
  );
}
