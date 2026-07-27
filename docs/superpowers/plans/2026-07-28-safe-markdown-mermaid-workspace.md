# Safe Markdown and Mermaid Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the central document shell into a safe local authoring surface with source, live-preview, and reading modes; supported Markdown extensions; locally rendered Mermaid diagrams; and actionable diagnostics that never destroy source.

**Architecture:** `DocumentPane` owns one authoritative in-memory draft and mode selection. Focused renderer components transform that draft into React elements without enabling raw HTML; Mermaid is loaded lazily, configured for strict local rendering, checked for forbidden directives, and sanitized before its SVG enters the DOM. This slice does not save to the Vault, open links, call privileged Tauri commands, or mutate source files.

**Tech Stack:** React 19, TypeScript 7, `react-markdown`, `remark-gfm`, `remark-frontmatter`, Mermaid 11, DOMPurify, Vitest, Testing Library, Playwright.

---

### Task 1: Extended Markdown model and local syntax

**Files:**
- Create: `src/features/editor/localMarkdown.ts`
- Create: `src/features/editor/localMarkdown.test.ts`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `THIRD_PARTY_NOTICES.md`

- [ ] **Step 1: Add failing tests for documented local representations**

Create table-driven tests that require:

```ts
expect(prepareLocalMarkdown("---\ntitle: Demo\n---\nBody").frontmatter)
  .toEqual(["title: Demo"]);
expect(prepareLocalMarkdown("[[MCU reset|Reset note]]").body)
  .toContain("[Reset note](aiks-wiki:MCU%20reset)");
expect(prepareLocalMarkdown("Evidence paragraph ^evidence-1").body)
  .toContain("`^evidence-1`");
expect(calloutKind("> [!WARNING]\n> Verify the source")).toBe("warning");
```

The transformation must skip fenced code blocks so example syntax remains unchanged.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
npx vitest run src/features/editor/localMarkdown.test.ts
```

Expected: FAIL because `prepareLocalMarkdown` and `calloutKind` do not exist.

- [ ] **Step 3: Install the renderer dependencies**

Run:

```bash
npm install react-markdown remark-gfm remark-frontmatter mermaid dompurify
```

Record all five direct dependencies and their license identifiers in `THIRD_PARTY_NOTICES.md`. Do not add `rehype-raw`, remote themes, syntax-highlighter bundles, or CDN assets.

- [ ] **Step 4: Implement immutable local-syntax preparation**

Export:

```ts
interface PreparedMarkdown {
  readonly body: string;
  readonly frontmatter: readonly string[];
}

export function prepareLocalMarkdown(source: string): PreparedMarkdown;
export function calloutKind(source: string): "note" | "tip" | "warning" | "danger" | null;
```

Rules:

- extract only a leading `---` frontmatter block;
- convert `[[target|label]]` and `[[target]]` outside fences to `aiks-wiki:` links;
- represent trailing block IDs as inline code without changing their identifier;
- leave fenced Markdown, Mermaid, and code bytes untouched;
- return new strings and arrays without mutating inputs.

- [ ] **Step 5: Verify and commit**

Run:

```bash
npx vitest run src/features/editor/localMarkdown.test.ts
npm test -- --run
npm run build
```

Expected: all tests and build pass.

Commit:

```bash
git add package.json package-lock.json THIRD_PARTY_NOTICES.md src/features/editor
git commit -m "feat: add extended markdown preparation"
```

### Task 2: Safe React Markdown preview

**Files:**
- Create: `src/features/editor/MarkdownPreview.tsx`
- Create: `src/features/editor/MarkdownPreview.test.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write failing rendering and link-safety tests**

Render one fixture containing frontmatter, a GFM table, task list, footnote, callout, wiki link, block reference, raw `<script>`, `javascript:` URL, `file:` URL, and a regular HTTPS link. Assert:

```ts
expect(screen.getByRole("table")).toBeVisible();
expect(screen.getByRole("checkbox", { name: /done/i })).toBeDisabled();
expect(screen.getByText("title: Demo")).toBeVisible();
expect(screen.getByText("Reset note")).toHaveAttribute("data-link-kind", "wiki");
expect(document.querySelector("script")).toBeNull();
expect(document.querySelector('a[href^="javascript:"]')).toBeNull();
expect(document.querySelector('a[href^="file:"]')).toBeNull();
expect(screen.getByText(/link opening is disabled/i)).toBeVisible();
```

Also spy on `window.open`, `fetch`, and the injected privileged-bridge callback; clicking any rendered link must call none of them.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
npx vitest run src/features/editor/MarkdownPreview.test.tsx
```

Expected: FAIL because `MarkdownPreview` does not exist.

- [ ] **Step 3: Implement the safe preview**

Use:

```tsx
<ReactMarkdown
  remarkPlugins={[remarkGfm, remarkFrontmatter]}
  skipHtml
  components={safeComponents}
>
  {prepared.body}
</ReactMarkdown>
```

Required component policy:

- links render as inert buttons with `data-link-kind="wiki" | "web" | "blocked"`;
- clicking a link only updates a visible local handoff message;
- code fences remain text unless their language is `mermaid`;
- task inputs remain disabled;
- frontmatter renders as a compact metadata card;
- blockquotes expose the detected callout kind;
- no `dangerouslySetInnerHTML` is used for Markdown;
- no renderer code calls Tauri, `fetch`, `window.open`, or creates external resource elements.

- [ ] **Step 4: Add restrained Obsidian-like preview styles**

Style tables, tasks, footnotes, callouts, metadata, inline code, code fences, and inert links within `.document-preview`. Keep the existing density, typography, and violet accent; do not add branded assets or a second editor layout.

- [ ] **Step 5: Verify and commit**

Run:

```bash
npx vitest run src/features/editor/MarkdownPreview.test.tsx
npm test -- --run
npm run build
```

Expected: all tests and build pass.

Commit:

```bash
git add src/features/editor/MarkdownPreview.tsx src/features/editor/MarkdownPreview.test.tsx src/styles.css
git commit -m "feat: render extended markdown safely"
```

### Task 3: Strict local Mermaid rendering and diagnostics

**Files:**
- Create: `src/features/editor/mermaidPolicy.ts`
- Create: `src/features/editor/mermaidPolicy.test.ts`
- Create: `src/features/editor/MermaidBlock.tsx`
- Create: `src/features/editor/MermaidBlock.test.tsx`
- Modify: `src/features/editor/MarkdownPreview.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write failing policy and component tests**

Require deterministic rejection of Mermaid init/config directives and click directives:

```ts
expect(validateMermaidSource("%%{init: {'theme':'dark'}}%%\ngraph TD\nA-->B"))
  .toEqual({ ok: false, message: expect.stringMatching(/directive/i) });
expect(validateMermaidSource("graph TD\nclick A href \"https://example.com\""))
  .toEqual({ ok: false, message: expect.stringMatching(/click/i) });
```

Mock the lazy Mermaid module so valid source returns SVG containing safe nodes plus `script`, `foreignObject`, `image`, `a`, `href`, `style`, and event attributes. Assert only the safe SVG structure reaches the DOM. Mock a parse rejection and assert the source remains visible beside an actionable diagnostic.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
npx vitest run src/features/editor/mermaidPolicy.test.ts src/features/editor/MermaidBlock.test.tsx
```

Expected: FAIL because the policy and component do not exist.

- [ ] **Step 3: Implement strict initialization and source validation**

Initialize Mermaid once with:

```ts
mermaid.initialize({
  startOnLoad: false,
  securityLevel: "strict",
  suppressErrorRendering: true,
  htmlLabels: false,
  flowchart: { htmlLabels: false },
});
```

Before parsing or rendering, reject:

- `%%{init:...}%%`, `%%{initialize:...}%%`, and `%%{config:...}%%`;
- any `click` directive;
- input over the bounded editor limit.

Render with a unique deterministic DOM ID, catch every parse/render error, and never replace or edit the source draft.

- [ ] **Step 4: Sanitize generated SVG**

Use DOMPurify with the SVG profile and explicit blocklists:

```ts
DOMPurify.sanitize(svg, {
  USE_PROFILES: { svg: true, svgFilters: true },
  FORBID_TAGS: ["script", "style", "foreignObject", "image", "a"],
  FORBID_ATTR: ["href", "xlink:href", "style"],
  SANITIZE_NAMED_PROPS: true,
});
```

Only this already-sanitized SVG may use `dangerouslySetInnerHTML`. Do not invoke Mermaid `bindFunctions`. Show the original source in a collapsible text block for both success and failure.

- [ ] **Step 5: Integrate Mermaid fences and verify**

The custom Markdown `code` component shall route only `language-mermaid` blocks to `MermaidBlock`; all other languages remain escaped code.

Run:

```bash
npx vitest run src/features/editor
npm test -- --run
npm run build
npm audit --audit-level=high
```

Expected: all tests, build, and audit pass.

Commit:

```bash
git add src/features/editor src/styles.css
git commit -m "feat: render mermaid with strict sanitization"
```

### Task 4: Source, live-preview, and reading modes

**Files:**
- Modify: `src/features/workbench/DocumentPane.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/styles.css`
- Modify: `e2e/source-workbench.spec.ts`

- [ ] **Step 1: Replace the two-mode contract with failing three-mode tests**

Require three tabs named `Source`, `Live preview`, and `Reading`. Edit the authoritative textarea, switch through all modes, and return to Source; assert the edited bytes remain exact. In live preview, assert the textarea and rendered preview are simultaneously visible. In reading mode, assert only the rendered preview is visible.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
npx vitest run src/App.test.tsx
```

Expected: FAIL because the current component has only Edit and Preview.

- [ ] **Step 3: Refactor `DocumentPane` to one authoritative draft**

Keep one `useState<string>` draft and a mode union:

```ts
type DocumentMode = "source" | "live" | "reading";
```

Mode layout:

- `source`: full-width textarea;
- `live`: resizable-looking 50/50 editor and preview split, with no fake persistence control;
- `reading`: full-width `MarkdownPreview`;
- all modes keep the existing path header and “Local draft · not saved” truth label.

Remove the hand-written line parser and route every preview through `MarkdownPreview`.

- [ ] **Step 4: Extend E2E acceptance**

At desktop width, edit Markdown and Mermaid, enter live preview, verify the diagram or diagnostic and edited heading, then enter reading mode and return to source without source loss. At 700px, stack live editor above preview and keep all three mode tabs reachable.

- [ ] **Step 5: Full verification and rendered inspection**

Run:

```bash
npm test -- --run
npm run build
npm run e2e
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
PATH=/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm run tauri build -- --debug --no-bundle
```

Start Vite on port `1421`, leaving KL-Man on `1420` untouched. Inspect source, live, reading, invalid Mermaid, and unsafe-content fixtures at 2048px, 1280px, and 700px; capture temporary screenshots outside the repository.

- [ ] **Step 6: Commit**

```bash
git add src/features/workbench/DocumentPane.tsx src/App.test.tsx src/styles.css e2e/source-workbench.spec.ts
git commit -m "feat: add mixed document workspace modes"
```

## Phase 2 completion gate

- [ ] `KNOW-002`, `KNOW-003`, and `KNOW-004` acceptance vectors are covered by deterministic tests.
- [ ] Raw HTML never executes; rendered links never open or fetch; Mermaid SVG is directive-checked and sanitized.
- [ ] Invalid Markdown or Mermaid preserves the authoritative source and leaves unaffected content usable.
- [ ] Source, live-preview, and reading modes share one exact draft and remain usable at all required widths.
- [ ] No file-writing, Vault registration, graph mutation, model, Agent, MCP, or cleanup capability is introduced in this slice.
- [ ] All frontend, Rust, E2E, audit, Clippy, and Tauri debug checks pass.
