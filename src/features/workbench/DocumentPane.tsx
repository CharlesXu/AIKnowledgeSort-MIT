import { useState, type ReactNode } from "react";

const initialDraft = `# Knowledge workspace

Use this document surface for research notes, classification decisions, and generated knowledge.

## Processing path

\`\`\`mermaid
flowchart LR
  Source --> Review
  Review --> Archive
  Archive --> Knowledge
\`\`\`

## Review state

\`\`\`ts
type ReviewState = "draft" | "approved";
const currentState: ReviewState = "draft";
\`\`\`

> Source files remain unchanged until an approved archive operation is verified.`;

const languageNames: Readonly<Record<string, string>> = {
  js: "JavaScript",
  javascript: "JavaScript",
  mermaid: "Mermaid",
  ts: "TypeScript",
  typescript: "TypeScript",
};

function renderDocument(source: string): readonly ReactNode[] {
  const lines = source.split("\n");
  const blocks: ReactNode[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index] ?? "";
    if (line.startsWith("```")) {
      const language = line.slice(3).trim().toLocaleLowerCase() || "text";
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index]?.startsWith("```")) {
        code.push(lines[index] ?? "");
        index += 1;
      }
      blocks.push(
        <section className={`code-preview code-preview--${language}`} key={`code-${index}`}>
          <header>
            <span>{languageNames[language] ?? language}</span>
            <small>{language === "mermaid" ? "Diagram source preview" : "Code block"}</small>
          </header>
          <pre>
            <code>{code.join("\n")}</code>
          </pre>
        </section>,
      );
    } else if (line.startsWith("# ")) {
      blocks.push(<h1 key={`line-${index}`}>{line.slice(2)}</h1>);
    } else if (line.startsWith("## ")) {
      blocks.push(<h2 key={`line-${index}`}>{line.slice(3)}</h2>);
    } else if (line.startsWith("> ")) {
      blocks.push(<blockquote key={`line-${index}`}>{line.slice(2)}</blockquote>);
    } else if (line.startsWith("- ")) {
      blocks.push(<p key={`line-${index}`}>• {line.slice(2)}</p>);
    } else if (line.length > 0) {
      blocks.push(<p key={`line-${index}`}>{line}</p>);
    }
    index += 1;
  }

  return blocks;
}

export function DocumentPane() {
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [draft, setDraft] = useState(initialDraft);

  return (
    <section aria-label="Document workspace" className="document-pane">
      <header className="document-toolbar">
        <div className="document-toolbar__path">
          Workspace / Drafts / Knowledge workspace.md
        </div>
        <div aria-label="Document mode" className="document-toolbar__tabs" role="tablist">
          <button
            aria-selected={mode === "edit"}
            onClick={() => setMode("edit")}
            role="tab"
            type="button"
          >
            Edit
          </button>
          <button
            aria-selected={mode === "preview"}
            onClick={() => setMode("preview")}
            role="tab"
            type="button"
          >
            Preview
          </button>
        </div>
      </header>
      <div className="document-heading">
        <div>
          <h1>Knowledge workspace</h1>
          <p>Local draft · not saved</p>
        </div>
        <span className="document-heading__formats">Markdown · Mermaid · Code</span>
      </div>
      <div className="document-pane__body">
        {mode === "edit" ? (
          <textarea
            aria-label="Markdown, Mermaid, and code editor"
            className="document-editor"
            onChange={(event) => setDraft(event.target.value)}
            spellCheck={false}
            value={draft}
          />
        ) : (
          <article
            aria-label="Document preview"
            className="document-preview"
            role="region"
          >
            {renderDocument(draft)}
          </article>
        )}
      </div>
    </section>
  );
}
