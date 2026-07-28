import { useEffect, useMemo, useState } from "react";
import { MarkdownPreview } from "../editor/MarkdownPreview";
import type { KnowledgeClient, KnowledgeDocument, KnowledgeTarget } from "../knowledge/types";

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

interface DocumentPaneProps {
  readonly client: KnowledgeClient;
  readonly targets: readonly KnowledgeTarget[];
}

function displayName(target: KnowledgeTarget | undefined): string {
  return target?.destinationPath.split("/").at(-1) ?? "Knowledge workspace";
}

function heading(target: KnowledgeTarget | undefined): string {
  return displayName(target).replace(/\.[^.]+$/, "");
}

export function DocumentPane({ client, targets }: DocumentPaneProps) {
  const [mode, setMode] = useState<DocumentMode>("source");
  const [draft, setDraft] = useState(initialDraft);
  const [selectedOperationId, setSelectedOperationId] = useState("");
  const [document, setDocument] = useState<KnowledgeDocument | null>(null);
  const [dirty, setDirty] = useState(false);
  const [pending, setPending] = useState<"open" | "save" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const selectedTarget = useMemo(
    () => targets.find((target) => target.operationId === selectedOperationId),
    [selectedOperationId, targets],
  );

  useEffect(() => {
    if (targets.length > 0 && !targets.some(
      (target) => target.operationId === selectedOperationId
    )) {
      setSelectedOperationId(targets[0].operationId);
    }
  }, [selectedOperationId, targets]);

  function selectTarget(operationId: string): void {
    setSelectedOperationId(operationId);
    setDocument(null);
    setDraft(initialDraft);
    setDirty(false);
    setError(null);
  }

  async function openDocument(): Promise<void> {
    if (selectedTarget === undefined) {
      return;
    }
    setPending("open");
    setError(null);
    try {
      const opened = await client.openDocument({
        authorityId: selectedTarget.authorityId,
        operationId: selectedTarget.operationId,
      });
      setDocument(opened);
      setDraft(opened.markdown);
      setDirty(false);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setPending(null);
    }
  }

  async function saveDocument(): Promise<void> {
    if (document === null) {
      return;
    }
    setPending("save");
    setError(null);
    try {
      const saved = await client.saveDocument({
        authorityId: document.authorityId,
        operationId: document.operationId,
        expectedRevision: document.revision,
        markdown: draft,
      });
      setDocument(saved);
      setDraft(saved.markdown);
      setDirty(false);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setPending(null);
    }
  }

  return (
    <section aria-label="Document workspace" className="document-pane">
      <header className="document-toolbar">
        <div className="document-toolbar__path">
          {document?.markdownPath ?? "Workspace / Drafts / Knowledge workspace.md"}
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
          <h1>{heading(selectedTarget)}</h1>
          <p>
            {document === null
              ? "Local draft · not saved"
              : document.revision === 0
                ? "New Vault note · not saved"
                : `Saved revision ${document.revision}${dirty ? " · unsaved edits" : ""}`}
          </p>
        </div>
        <div className="document-heading__actions">
          {targets.length > 0 ? (
            <select
              aria-label="Eligible archived original"
              onChange={(event) => selectTarget(event.target.value)}
              value={selectedOperationId}
            >
              {targets.map((target) => (
                <option key={target.operationId} value={target.operationId}>
                  {displayName(target)}
                </option>
              ))}
            </select>
          ) : null}
          {selectedTarget !== undefined && document === null ? (
            <button
              disabled={pending !== null}
              onClick={() => void openDocument()}
              type="button"
            >
              {pending === "open" ? "Opening…" : "Create knowledge note"}
            </button>
          ) : null}
          {document !== null ? (
            <button
              disabled={!dirty || pending !== null}
              onClick={() => void saveDocument()}
              type="button"
            >
              {pending === "save" ? "Saving…" : "Save Vault revision"}
            </button>
          ) : null}
          <span className="document-heading__formats">
            Markdown · Mermaid · Code
          </span>
        </div>
      </div>
      {error !== null ? <p className="document-error" role="alert">{error}</p> : null}
      <div
        className={`document-pane__body document-pane__body--${mode}`}
        data-document-mode={mode}
      >
        {mode !== "reading" ? (
          <textarea
            aria-label="Markdown, Mermaid, and code editor"
            className="document-editor"
            onChange={(event) => {
              setDraft(event.target.value);
              if (document !== null) {
                setDirty(event.target.value !== document.markdown);
              }
            }}
            spellCheck={false}
            value={draft}
          />
        ) : null}
        {mode !== "source" ? <MarkdownPreview source={draft} /> : null}
      </div>
    </section>
  );
}
