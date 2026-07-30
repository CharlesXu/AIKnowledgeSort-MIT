import { useMemo, useState, type ReactNode } from "react";
import ReactMarkdown, {
  type Components,
} from "react-markdown";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";
import { calloutKind, prepareLocalMarkdown } from "./localMarkdown";
import { MermaidBlock } from "./MermaidBlock";
import { useI18n } from "../../i18n/I18nContext";

interface MarkdownPreviewProps {
  readonly source: string;
}

type LinkKind = "blocked" | "local" | "web" | "wiki";

function textContent(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") {
    return String(node);
  }
  if (Array.isArray(node)) {
    return node.map(textContent).join("");
  }
  if (node && typeof node === "object" && "props" in node) {
    const props = node.props as { readonly children?: ReactNode };
    return textContent(props.children);
  }
  return "";
}

function classifyLink(href: string): LinkKind {
  if (href.startsWith("aiks-wiki:")) return "wiki";
  if (href.startsWith("#")) return "local";
  if (/^https?:\/\//i.test(href)) return "web";
  return "blocked";
}

function localUrlTransform(url: string): string {
  const kind = classifyLink(url);
  return kind === "blocked"
    ? `aiks-blocked:${encodeURIComponent(url)}`
    : url;
}

function languageLabel(className?: string): string {
  const language = /language-([\w-]+)/.exec(className ?? "")?.[1];
  if (!language) return "Code";
  const labels: Readonly<Record<string, string>> = {
    html: "HTML",
    js: "JavaScript",
    javascript: "JavaScript",
    md: "Markdown",
    markdown: "Markdown",
    ts: "TypeScript",
    typescript: "TypeScript",
  };
  return labels[language] ?? language;
}

export function MarkdownPreview({ source }: MarkdownPreviewProps) {
  const { t } = useI18n();
  const prepared = useMemo(() => prepareLocalMarkdown(source), [source]);
  const [handoffMessage, setHandoffMessage] = useState("");

  const components = useMemo<Components>(
    () => ({
      a({ children, href = "" }) {
        const kind = classifyLink(href);
        const effectiveKind = kind === "blocked" && href.startsWith("aiks-blocked:")
          ? "blocked"
          : kind;
        return (
          <button
            className="markdown-link"
            data-link-kind={effectiveKind}
            onClick={() =>
              setHandoffMessage(
                t("markdown.linkDisabled", { kind: effectiveKind }),
              )
            }
            type="button"
          >
            {children}
          </button>
        );
      },
      blockquote({ children }) {
        const kind = calloutKind(textContent(children));
        return <blockquote data-callout={kind ?? undefined}>{children}</blockquote>;
      },
      code({ children, className }) {
        const isBlock = className?.startsWith("language-");
        if (!isBlock) {
          return <code className={className}>{children}</code>;
        }
        if (className === "language-mermaid") {
          return (
            <MermaidBlock source={String(children).replace(/\n$/, "")} />
          );
        }
        return (
          <section className="code-preview">
            <header>
              <span>{languageLabel(className)}</span>
              <small>{t("markdown.codeBlock")}</small>
            </header>
            <pre>
              <code className={className}>{children}</code>
            </pre>
          </section>
        );
      },
      input({ checked, node: _node, ...props }) {
        return (
          <input
            {...props}
            aria-label={checked ? t("markdown.taskDone") : t("markdown.taskPending")}
            checked={checked}
            disabled
            readOnly
          />
        );
      },
      pre({ children }) {
        return <>{children}</>;
      },
    }),
    [t],
  );

  return (
    <article
      aria-label={t("markdown.preview")}
      className="document-preview"
      role="region"
    >
      {prepared.frontmatter.length > 0 ? (
        <section aria-label={t("markdown.metadata")} className="markdown-frontmatter">
          <strong>{t("markdown.frontmatter")}</strong>
          <ul>
            {prepared.frontmatter.map((line, index) => (
              <li key={`${index}-${line}`}>
                <code>{line}</code>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      <ReactMarkdown
        components={components}
        remarkPlugins={[remarkGfm, remarkFrontmatter]}
        skipHtml
        urlTransform={localUrlTransform}
      >
        {prepared.body}
      </ReactMarkdown>
      {handoffMessage ? (
        <p
          aria-label={t("markdown.linkStatus")}
          className="markdown-link-status"
          role="status"
        >
          {handoffMessage}
        </p>
      ) : null}
    </article>
  );
}
