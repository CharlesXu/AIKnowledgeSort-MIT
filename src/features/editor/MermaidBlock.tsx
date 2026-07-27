import { useEffect, useId, useMemo, useState } from "react";
import {
  sanitizeMermaidSvg,
  validateMermaidSource,
} from "./mermaidPolicy";

interface MermaidBlockProps {
  readonly source: string;
}

type RenderState =
  | { readonly status: "idle" | "loading" }
  | { readonly status: "ready"; readonly svg: string }
  | { readonly status: "error"; readonly message: string };

let mermaidInitialized = false;

function rendererId(reactId: string): string {
  const safeId = reactId.replace(/[^a-zA-Z0-9_-]/g, "");
  return `aiks-mermaid-${safeId || "diagram"}`;
}

export function MermaidBlock({ source }: MermaidBlockProps) {
  const validation = useMemo(() => validateMermaidSource(source), [source]);
  const id = rendererId(useId());
  const [state, setState] = useState<RenderState>({ status: "idle" });

  useEffect(() => {
    if (!validation.ok) {
      setState({ status: "idle" });
      return;
    }

    let active = true;
    setState({ status: "loading" });

    void import("mermaid")
      .then(async ({ default: mermaid }) => {
        if (!mermaidInitialized) {
          mermaid.initialize({
            startOnLoad: false,
            securityLevel: "strict",
            suppressErrorRendering: true,
            htmlLabels: false,
            flowchart: { htmlLabels: false },
          });
          mermaidInitialized = true;
        }
        await mermaid.parse(source, { suppressErrors: false });
        const { svg } = await mermaid.render(id, source);
        if (active) {
          setState({ status: "ready", svg: sanitizeMermaidSvg(svg) });
        }
      })
      .catch(() => {
        if (active) {
          setState({
            status: "error",
            message:
              "Check the diagram syntax and correct the highlighted Mermaid source.",
          });
        }
      });

    return () => {
      active = false;
    };
  }, [id, source, validation]);

  const diagnostic = validation.ok
    ? state.status === "error"
      ? state.message
      : null
    : validation.message;

  return (
    <section className="mermaid-block">
      <header>
        <span>Mermaid</span>
        <small>
          {state.status === "loading"
            ? "Rendering locally…"
            : diagnostic
              ? "Source preserved"
              : "Local diagram"}
        </small>
      </header>
      {state.status === "ready" ? (
        <div
          aria-label="Rendered Mermaid diagram"
          className="mermaid-block__diagram"
          dangerouslySetInnerHTML={{ __html: state.svg }}
          role="img"
        />
      ) : null}
      {diagnostic ? (
        <p
          aria-label="Mermaid diagnostic"
          className="mermaid-block__diagnostic"
          role="alert"
        >
          {diagnostic}
        </p>
      ) : null}
      <details className="mermaid-block__source" open={diagnostic !== null}>
        <summary>Mermaid source</summary>
        <pre>
          <code>{source}</code>
        </pre>
      </details>
    </section>
  );
}
