import { useState } from "react";
import { MarkdownPreview } from "../editor/MarkdownPreview";

type DocumentMode = "source" | "live" | "reading";

const initialDraft = `---
title: Knowledge workspace
status: draft
---

# Knowledge workspace

Use this document surface for research notes, classification decisions, and generated knowledge.

## Processing path

\`\`\`mermaid
flowchart LR
  Source --> Review
  Review --> Archive
  Archive --> Knowledge
\`\`\`

## Review state

| State | Meaning |
| --- | --- |
| Draft | Local edits only |
| Approved | Eligible for a later archive workflow |

- [x] Preserve the source
- [ ] Confirm the archive

Link evidence with [[Reliability|a local knowledge note]] and stable block references. ^review-state

> [!WARNING]
> Source files remain unchanged until an approved archive operation is verified.

\`\`\`ts
type ReviewState = "draft" | "approved";
const currentState: ReviewState = "draft";
\`\`\``;

const modes: readonly {
  readonly id: DocumentMode;
  readonly label: string;
}[] = [
  { id: "source", label: "Source" },
  { id: "live", label: "Live preview" },
  { id: "reading", label: "Reading" },
];

export function DocumentPane() {
  const [mode, setMode] = useState<DocumentMode>("source");
  const [draft, setDraft] = useState(initialDraft);

  return (
    <section aria-label="Document workspace" className="document-pane">
      <header className="document-toolbar">
        <div className="document-toolbar__path">
          Workspace / Drafts / Knowledge workspace.md
        </div>
        <div
          aria-label="Document mode"
          className="document-toolbar__tabs"
          role="tablist"
        >
          {modes.map((item) => (
            <button
              aria-selected={mode === item.id}
              key={item.id}
              onClick={() => setMode(item.id)}
              role="tab"
              type="button"
            >
              {item.label}
            </button>
          ))}
        </div>
      </header>
      <div className="document-heading">
        <div>
          <h1>Knowledge workspace</h1>
          <p>Local draft · not saved</p>
        </div>
        <span className="document-heading__formats">
          Markdown · Mermaid · Code
        </span>
      </div>
      <div
        className={`document-pane__body document-pane__body--${mode}`}
        data-document-mode={mode}
      >
        {mode !== "reading" ? (
          <textarea
            aria-label="Markdown, Mermaid, and code editor"
            className="document-editor"
            onChange={(event) => setDraft(event.target.value)}
            spellCheck={false}
            value={draft}
          />
        ) : null}
        {mode !== "source" ? <MarkdownPreview source={draft} /> : null}
      </div>
    </section>
  );
}
