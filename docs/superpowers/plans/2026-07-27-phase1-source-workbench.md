# Phase 1 Source Workbench Implementation Plan

> **For the implementing Agent:** Use the installed `executing-plans` skill to implement this plan task by task. Keep each red/green/refactor cycle and commit independently reviewable.

**Goal:** Deliver a production-quality, runnable Tauri 2 + React/TypeScript + Rust desktop skeleton whose first vertical slice provides an original, Obsidian-inspired narrow toolbar and source tree, correct mixed file/directory tri-state selection, and a reviewable, deduplicated, non-mutating OS Drop discovery proposal.

**Architecture:** React owns presentation and pure selection state. A small `DiscoveryPort` separates UI orchestration from runtime integration. The production Tauri adapter forwards only OS-provided local paths to a Rust command; Rust owns grants, resolved-boundary checks, read-only traversal, symlink detection, deduplication, and proposal construction. A browser-only adapter exposes deterministic named scenarios for component and Playwright tests, but has no real filesystem API and is excluded from production runtime selection.

**Tech Stack:** Tauri 2, Rust stable, React, TypeScript, Vite, Vitest, React Testing Library, Playwright, ESLint, Prettier, Cargo test/clippy/fmt.

**Phase 1 requirement claims:** `ARCH-001` target-stack skeleton; `UI-005` interaction inspiration without copied assets; `UI-006` source-tree selection; `UI-007` mixed local Drop; `SAFE-009` scoped, visible, non-mutating discovery.

**Explicitly not claimed in Phase 1:** full `UI-002` pane resize/collapse/split persistence, graph playback, archive mutation, SHA-256 registration, classification, canonical naming, Markdown, Mermaid, graph, model adjudication, cleanup, recovery, audit persistence, or MCP. The workbench shell must reserve ordinary component slots for later stages, but must not implement or simulate those capabilities.

---

## Success criteria

1. `npm run tauri dev` launches a Tauri 2 app backed by Rust with a React/TypeScript UI.
2. A narrow original toolbar sits beside a source tree. No third-party branding, icons, screenshots, or proprietary visual assets ship.
3. Every displayed file and directory has a keyboard-accessible checkbox.
4. Selecting a directory selects every eligible discovered descendant. Deselecting a child makes every affected displayed ancestor indeterminate; reselecting it restores selected state.
5. Explicit mixed file/directory selections are retained for display while resolution produces one deduplicated set of eligible files.
6. A Tauri OS Drop of overlapping local files/directories produces one proposal with visible `included`, `excluded`, `unreadable`, `symlink`, and `out-of-scope` counts and review details.
7. Plain text and URL drag data never enter the local-path proposal flow.
8. Discovery detects links without following them, resolves containment in Rust instead of using string-prefix checks, and never exposes a filesystem mutation command.
9. Rust integration tests compare a before/after filesystem manifest to demonstrate that success, exclusion, unreadability, links, and scope denial do not alter paths or bytes.
10. Browser UI and E2E tests simulate drops through a deterministic local adapter that cannot read or mutate the real filesystem.
11. Unit, component, Rust integration, browser E2E, type, lint, formatting, and current-platform Tauri build checks pass. CI declares build/test jobs for Windows, macOS, and Linux.

## Key decisions and tradeoffs

- **Rust owns discovery.** This makes path authorization and link handling testable at the trusted boundary. It costs a little more IPC modeling than frontend traversal, but prevents renderer code from becoming the authority for filesystem scope.
- **Deduplicate by resolved path identity, not file content.** Phase 1 is discovery, not content registration. The same local file reached through overlapping roots appears once; content-level SHA-256 identity remains deferred to the archive phase.
- **Keep explicit selection separate from resolved selection.** `explicitSelectionIds` preserves what the user checked, while a pure resolver derives unique eligible leaf IDs. This is slightly more state than storing selected leaves only, but it satisfies the observable mixed-selection contract without ambiguity.
- **Use disjoint proposal outcomes.** Every discovered entry has exactly one outcome: `included`, `excluded`, `unreadable`, `symlink`, or `out_of_scope`. Summary counts are derived from outcomes; duplicate traversal encounters are removed before counting and reported separately as `duplicateEncounterCount`.
- **Use dependency-injected read-only filesystem access in Rust.** The production implementation delegates to `std::fs`; tests can deterministically inject metadata/read failures on every platform. This is justified by the cross-platform unreadable test contract and does not create a general repository abstraction.
- **Use named browser scenarios, not arbitrary browser paths.** Playwright can exercise the UI without Tauri or local filesystem grants. The tradeoff is that OS path integration remains a Rust/Tauri integration test and a desktop smoke check rather than a browser test.
- **Defer full pane behavior.** Phase 1 supplies semantic navigation/content/details regions only. Resizing, splitting, collapsing, and persistence need a separate acceptance plan for `UI-002`.

## Component and boundary map

```text
App
└── SourceWorkbench
    ├── LeftRail
    ├── SourceNavigator
    │   ├── ScopeToolbar
    │   └── SourceTree
    │       └── SourceTreeNode (recursive presentation only)
    ├── DiscoveryProposalPanel
    │   ├── ProposalSummary
    │   └── ProposalOutcomeList
    └── ItemDetailsPanel

SourceWorkbenchController
├── pure selection model
└── DiscoveryPort
    ├── TauriDiscoveryAdapter -> invoke("discover_drop") -> Rust trusted core
    └── BrowserDiscoveryAdapter -> named in-memory scenarios only

Rust discover_drop
├── ActiveGrantStore
├── ScopeResolver
├── ReadOnlyFilesystem
└── DiscoveryService
```

The future Markdown, graph, archive, and MCP work must connect through new ports. They must not be added to `DiscoveryPort`, because discovery is read-only and should remain incapable of mutation.

## Core interfaces

Create these TypeScript contracts in `src/features/source-workbench/model.ts`:

```ts
export type NodeKind = "file" | "directory";
export type CheckState = "unchecked" | "checked" | "indeterminate";

export interface SourceNode {
  readonly id: string;
  readonly name: string;
  readonly kind: NodeKind;
  readonly eligible: boolean;
  readonly children: readonly SourceNode[];
}

export interface SelectionState {
  readonly explicitSelectionIds: ReadonlySet<string>;
}

export type DiscoveryOutcome =
  | "included"
  | "excluded"
  | "unreadable"
  | "symlink"
  | "out_of_scope";

export interface DiscoveryItem {
  readonly id: string;
  readonly displayPath: string;
  readonly kind: NodeKind | "other";
  readonly outcome: DiscoveryOutcome;
  readonly reasonCode?: string;
}

export interface DiscoveryCounts {
  readonly included: number;
  readonly excluded: number;
  readonly unreadable: number;
  readonly symlink: number;
  readonly outOfScope: number;
}

export interface DiscoveryProposal {
  readonly id: string;
  readonly grantId: string;
  readonly createdAt: string;
  readonly roots: readonly string[];
  readonly items: readonly DiscoveryItem[];
  readonly counts: DiscoveryCounts;
  readonly duplicateEncounterCount: number;
  readonly mutationCapability: "none";
}

export interface LocalPathDrop {
  readonly kind: "local-paths";
  readonly paths: readonly string[];
}

export interface DiscoveryPort {
  discoverDrop(drop: LocalPathDrop): Promise<DiscoveryProposal>;
}
```

The production adapter must not accept `text/plain`, `text/uri-list`, URLs, or arbitrary DOM `DataTransfer` strings. It receives only the local path array from the Tauri webview drag/drop event.

Mirror the serialized request/result in `src-tauri/src/discovery/types.rs`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverDropRequest {
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiscoveryOutcome {
    Included,
    Excluded,
    Unreadable,
    Symlink,
    OutOfScope,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProposal {
    pub id: String,
    pub grant_id: String,
    pub created_at: String,
    pub roots: Vec<String>,
    pub items: Vec<DiscoveryItem>,
    pub counts: DiscoveryCounts,
    pub duplicate_encounter_count: usize,
    pub mutation_capability: MutationCapability,
}
```

`ActiveGrantStore` remains Rust-owned. Renderer requests carry no `allowedRoots` field and therefore cannot expand their own authority.

---

### Task 1: Bootstrap the target stack and verification scripts

**Files:**

- Create: `package.json`
- Create: `package-lock.json`
- Create: `index.html`
- Create: `tsconfig.json`
- Create: `tsconfig.app.json`
- Create: `tsconfig.node.json`
- Create: `vite.config.ts`
- Create: `vitest.config.ts`
- Create: `playwright.config.ts`
- Create: `eslint.config.js`
- Create: `.prettierrc.json`
- Create: `.prettierignore`
- Create: `.gitignore`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/vite-env.d.ts`
- Create: `src/test/setup.ts`
- Create: `src/App.test.tsx`
- Create: `src/styles/tokens.css`
- Create: `src/styles/global.css`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `assets/app-icon.svg`
- Generate: `src-tauri/icons/32x32.png`
- Generate: `src-tauri/icons/128x128.png`
- Generate: `src-tauri/icons/128x128@2x.png`
- Generate: `src-tauri/icons/icon.icns`
- Generate: `src-tauri/icons/icon.ico`

**Step 1: Write the failing stack smoke test**

Create `src/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { App } from "./App";

it("renders the source workbench landmark", () => {
  render(<App />);
  expect(
    screen.getByRole("main", { name: "Source workbench" }),
  ).toBeInTheDocument();
});
```

**Step 2: Run the test and verify RED**

Run:

```bash
npm test -- --run src/App.test.tsx
```

Expected: FAIL because the app and test configuration do not exist yet.

**Step 3: Create the minimal Vite/React/Tauri skeleton**

- Pin supported dependency versions in `package-lock.json` and `Cargo.lock`.
- Use Tauri 2 APIs only.
- Export `App`; render semantic `main`, navigation, content, and details landmarks.
- Keep all colors, spacing, focus rings, and typography in original CSS tokens.
- Use CSS-drawn geometric marks or text labels in `LeftRail`; do not import an icon pack.
- Set a restrictive Tauri capability with only core window/event functionality and the explicit discovery command added later. Do not enable shell, filesystem write, process, opener, or broad path capabilities.
- Create an original geometric `assets/app-icon.svg` and generate required platform icons with the Tauri icon command. Record it as project-created material; do not use Tauri/sample/vendor artwork.

**Step 4: Add repeatable scripts**

`package.json` must expose:

```json
{
  "scripts": {
    "dev": "vite",
    "dev:browser": "vite --mode browser",
    "build": "tsc -b && vite build",
    "typecheck": "tsc -b --pretty false",
    "lint": "eslint . --max-warnings 0",
    "format:check": "prettier --check .",
    "test": "vitest",
    "test:coverage": "vitest run --coverage",
    "test:e2e": "playwright test",
    "tauri": "tauri"
  }
}
```

Configure coverage thresholds at 80% for lines, functions, branches, and statements. Exclude only generated files and the Tauri entrypoint, not business logic.

**Step 5: Verify GREEN and target metadata**

Run:

```bash
npm ci
npm test -- --run src/App.test.tsx
npm run typecheck
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps --format-version 1
```

Expected: tests/build pass; metadata identifies Tauri 2 and Rust, and `package-lock.json` identifies React and TypeScript.

**Step 6: Commit**

```bash
git add package.json package-lock.json index.html tsconfig.json tsconfig.app.json tsconfig.node.json vite.config.ts vitest.config.ts playwright.config.ts eslint.config.js .prettierrc.json .prettierignore .gitignore src src-tauri assets
git commit -m "feat: bootstrap Tauri source workbench"
```

---

### Task 2: Implement the tri-state model and accessible source tree

**Files:**

- Create: `src/features/source-workbench/model.ts`
- Create: `src/features/source-workbench/selection.ts`
- Create: `src/features/source-workbench/fixtures.ts`
- Create: `src/features/source-workbench/selection.test.ts`

**Step 1: Write failing tests from `UI-006-source-tree-mixed-selection`**

Cover all of these cases:

```ts
it("selects all eligible descendants of a directory");
it("makes every displayed ancestor indeterminate after a child is deselected");
it("returns ancestors to checked when the child is reselected");
it("keeps mixed explicit selections while resolving unique eligible files");
it("does not select ineligible descendants");
it("returns unchecked for a directory with no eligible descendants");
it("updates duplicate displayed views from canonical node ids");
```

Use the generated hierarchy:

```text
root/a
├── one.txt
└── sub
    └── two.txt
```

Also display the same two canonical file IDs beneath a second view. Assert that explicitly selecting `root/a` and `root/a/one.txt` retains both IDs in `explicitSelectionIds`, while `resolveEligibleFileIds` returns exactly two IDs.

**Step 2: Run and verify RED**

```bash
npm test -- --run src/features/source-workbench/selection.test.ts
```

Expected: FAIL because the selection functions are absent.

**Step 3: Implement the minimum pure functions**

Implement and export:

```ts
indexSourceTree(nodes): SourceTreeIndex
toggleExplicitSelection(state, nodeId, nextChecked): SelectionState
getCheckState(index, state, nodeId): CheckState
resolveEligibleFileIds(index, state): ReadonlySet<string>
```

Rules:

- Never mutate input arrays, nodes, maps, or sets; return new values.
- Directory state derives from eligible descendant files.
- A directory with some but not all eligible descendants selected is `indeterminate`.
- A child toggle recomputes all rendered ancestors through the index; do not store stale ancestor state.
- Canonical IDs, not labels or display paths, synchronize duplicate views.
- Directory IDs remain in explicit state when directly checked; resolved output contains eligible files only and deduplicates by canonical file ID.

**Step 4: Verify GREEN and coverage**

```bash
npm test -- --run src/features/source-workbench/selection.test.ts
npm run test:coverage
```

Expected: every vector passes and selection branch coverage is at least 90%.

**Step 5: Commit**

```bash
git add src/features/source-workbench
git commit -m "feat: add tri-state source selection model"
```

---

#### Task 2B: Build the accessible toolbar and source tree

**Files:**

- Create: `src/features/source-workbench/SourceWorkbench.tsx`
- Create: `src/features/source-workbench/LeftRail.tsx`
- Create: `src/features/source-workbench/SourceNavigator.tsx`
- Create: `src/features/source-workbench/SourceTree.tsx`
- Create: `src/features/source-workbench/SourceTreeNode.tsx`
- Create: `src/features/source-workbench/SourceWorkbench.test.tsx`
- Create: `src/features/source-workbench/source-workbench.css`
- Modify: `src/App.tsx`
- Modify: `src/styles/tokens.css`
- Modify: `src/styles/global.css`

**Step 1: Write failing component tests**

Test observable behavior, not component internals:

```tsx
it("renders an original narrow toolbar beside the source tree");
it("gives every displayed file and directory an accessible checkbox");
it("propagates directory selection to eligible descendants");
it("sets directory and every ancestor checkbox to indeterminate");
it("restores selected ancestors after the child is reselected");
it("shows explicit mixed selections and two unique resolved files");
it("supports keyboard expansion and checkbox activation");
```

Use `HTMLInputElement.indeterminate` assertions and accessible roles/names. Avoid snapshots as the primary behavior assertion.

**Step 2: Run and verify RED**

```bash
npm test -- --run src/features/source-workbench/SourceWorkbench.test.tsx
```

Expected: FAIL because the components are absent.

**Step 3: Implement the component shell**

- `LeftRail` is 44–52 logical pixels wide and uses original CSS shapes/text only.
- `SourceNavigator` places its tree directly beside the rail.
- `SourceTreeNode` is presentation-only: it receives check state and callbacks rather than owning selection rules.
- Use native checkbox semantics; set `.indeterminate` through a focused ref effect.
- Use `tree`, `treeitem`, `group`, and descriptive labels correctly.
- Maintain visible focus at 200% zoom and a minimum 24 px row hit area.
- Render center `DiscoveryProposalPanel` and right `ItemDetailsPanel` slots as Phase 1 regions, not fake Markdown or graph features.
- Add a visible “Discovery only — no files will be changed” status.

**Step 4: Verify behavior and accessibility**

```bash
npm test -- --run src/features/source-workbench/SourceWorkbench.test.tsx
npm run lint
npm run typecheck
```

Expected: component tests pass with no lint/type errors.

**Step 5: Commit**

```bash
git add src/App.tsx src/styles src/features/source-workbench
git commit -m "feat: render accessible source tree workbench"
```

---

### Task 3: Implement the Rust read-only discovery core

**Files:**

- Create: `src-tauri/src/discovery/mod.rs`
- Create: `src-tauri/src/discovery/types.rs`
- Create: `src-tauri/src/discovery/grant.rs`
- Create: `src-tauri/src/discovery/scope.rs`
- Create: `src-tauri/src/discovery/filesystem.rs`
- Create: `src-tauri/src/discovery/service.rs`
- Create: `src-tauri/tests/discovery_contract.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

**Step 1: Write failing Rust unit and contract tests**

Unit tests:

```rust
#[test] fn overlapping_roots_produce_unique_included_files();
#[test] fn resolved_containment_rejects_sibling_prefixes();
#[test] fn symlinks_are_reported_and_never_followed();
#[test] fn injected_metadata_failure_is_unreadable();
#[test] fn unsupported_entry_kind_is_excluded();
#[test] fn expired_or_missing_grant_rejects_discovery();
#[test] fn counts_are_derived_from_disjoint_outcomes();
```

Integration tests matching `UI-007-mixed-file-directory-drop` and `SAFE-009-drop-boundary-review`:

```rust
#[test] fn mixed_drop_reports_all_boundaries_without_mutation();
#[test] fn overlapping_file_and_directory_roots_are_deduplicated();
```

For the no-mutation proof, record a manifest of relative paths, entry kinds, file lengths, and independently calculated test-only SHA-256 values before discovery; compare it byte-for-byte after discovery. Include one in-scope readable file, one deterministic injected unreadable entry, one symlink/junction fixture when supported, and one out-of-scope file.

**Step 2: Run and verify RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml discovery
```

Expected: FAIL because the discovery modules are absent.

**Step 3: Implement the read-only ports and grant store**

Keep the filesystem surface deliberately small:

```rust
pub trait ReadOnlyFilesystem {
    fn symlink_metadata(&self, path: &Path) -> io::Result<MetadataView>;
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;
}
```

`ActiveGrantStore` owns an opaque grant ID, canonical allowed roots, and expiry. Production UI receives only public grant metadata. Do not accept allowed roots from `DiscoverDropRequest`.

Scope rules:

- Inspect `symlink_metadata` before canonicalization or recursion.
- Record a symlink outcome and do not follow it.
- Canonicalize the active grant roots when the native grant is created.
- Canonicalize each non-link candidate, then use platform-aware `Path::starts_with` component containment against a canonical root.
- Reject lexical traversal, sibling-prefix matches, resolution failures, expired grants, and scope drift.
- Revalidate descendants during traversal; never assume a parent authorization automatically authorizes a replaced child.

**Step 4: Implement deterministic discovery**

- Preserve input order for root presentation, then sort directory children by a stable normalized display key.
- Use canonical path keys for deduplication within one proposal.
- Count each canonical entry once even when reached through overlapping roots.
- Include regular eligible files only.
- Treat directories as traversal containers rather than included action items.
- Record unsupported non-file/non-directory entry types as `excluded`.
- Record metadata/read failures as `unreadable`.
- Record detected links as `symlink`.
- Record resolved paths beyond all active roots as `out_of_scope`.
- Return one proposal with `mutationCapability: "none"`.
- Expose no write, rename, archive, trash, delete, shell, or arbitrary-open operation from this module.

**Step 5: Verify GREEN, formatting, and static checks**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: all Rust tests pass; clippy emits no warnings.

**Step 6: Commit**

```bash
git add src-tauri
git commit -m "feat: add scoped read-only discovery core"
```

---

### Task 4: Add native scope grant acquisition and the Tauri Drop adapter

**Files:**

- Create: `src-tauri/src/discovery/commands.rs`
- Create: `src/adapters/discovery/createDiscoveryPort.ts`
- Create: `src/adapters/discovery/tauriDiscoveryAdapter.ts`
- Create: `src/adapters/discovery/tauriDiscoveryAdapter.test.ts`
- Create: `src/features/source-workbench/useDiscoveryDrop.ts`
- Create: `src/features/source-workbench/ScopeToolbar.tsx`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src/features/source-workbench/SourceWorkbench.tsx`

**Step 1: Write failing adapter and command tests**

Frontend tests:

```ts
it("forwards only Tauri local path drop events");
it("ignores hover, cancel, text, and URL-like browser drag data");
it("submits one request for one OS drop batch");
it("shows a visible error and no proposal when discovery is denied");
```

Rust command tests:

```rust
#[test] fn request_without_active_grant_is_denied();
#[test] fn renderer_cannot_supply_or_expand_allowed_roots();
#[test] fn discover_command_returns_the_read_only_proposal_shape();
```

**Step 2: Run and verify RED**

```bash
npm test -- --run src/adapters/discovery/tauriDiscoveryAdapter.test.ts
cargo test --manifest-path src-tauri/Cargo.toml commands
```

Expected: FAIL because adapters and commands are absent.

**Step 3: Implement native grant acquisition**

- `request_discovery_grant` opens the operating system directory picker from the Rust/Tauri side.
- Only paths returned by that native user gesture may enter `ActiveGrantStore`.
- Canonicalize and validate selected roots before replacing the active discovery grant.
- Return opaque grant ID, redacted display roots, and expiry to the renderer.
- Cancellation leaves the prior grant unchanged.
- Do not persist grants in Phase 1; relaunch requires a fresh native grant.

This is a discovery scope grant only. It authorizes metadata enumeration and read-only proposal construction, never mutation.

**Step 4: Implement the Tauri OS Drop adapter**

- Subscribe to Tauri 2 webview drag/drop events.
- On `drop`, pass the OS-provided local path list to `discover_drop`.
- Do not register a generic DOM drop handler as a source of local paths.
- Ignore hover/cancel events after updating transient UI state.
- Ensure unsubscribe runs on component teardown.
- Normalize backend errors into visible, non-sensitive reason codes.
- Do not log absolute paths in production diagnostics.

`createDiscoveryPort.ts` must choose `TauriDiscoveryAdapter` only when running in Tauri and must fail closed for an unknown runtime mode.

**Step 5: Restrict Tauri capabilities**

Allow only:

- the application window/event functionality required to receive native Drop;
- `request_discovery_grant`;
- `discover_drop`.

Explicitly verify the capability file does not grant general filesystem write, shell, process, opener, rename, remove, copy, or trash access.

**Step 6: Verify GREEN**

```bash
npm test -- --run src/adapters/discovery/tauriDiscoveryAdapter.test.ts
cargo test --manifest-path src-tauri/Cargo.toml
npm run typecheck
npm run lint
```

Expected: all tests/checks pass.

**Step 7: Desktop smoke test**

Run:

```bash
npm run tauri dev
```

Manually verify:

1. Choose a discovery scope through the native picker.
2. Drop a directory plus one nested file.
3. One proposal appears and overlapping entries are deduplicated.
4. Required counts and read-only status are visible.
5. Drop a link and an out-of-scope path; both are visibly excluded.
6. Drop text and an HTTPS URL from another app; neither becomes a local item.
7. Compare source filenames and bytes before/after.

Record the smoke result in the eventual implementation PR/test report, not as an unverified automated assertion.

**Step 8: Commit**

```bash
git add src src-tauri
git commit -m "feat: connect native drop discovery"
```

---

### Task 5: Add the browser/dev adapter and proposal UI

**Files:**

- Create: `src/adapters/discovery/browserDiscoveryAdapter.ts`
- Create: `src/adapters/discovery/browserScenarios.ts`
- Create: `src/adapters/discovery/browserDiscoveryAdapter.test.ts`
- Create: `src/features/source-workbench/DiscoveryProposalPanel.tsx`
- Create: `src/features/source-workbench/ProposalSummary.tsx`
- Create: `src/features/source-workbench/ProposalOutcomeList.tsx`
- Create: `src/features/source-workbench/DiscoveryProposalPanel.test.tsx`
- Modify: `src/adapters/discovery/createDiscoveryPort.ts`
- Modify: `src/features/source-workbench/SourceWorkbench.tsx`
- Modify: `src/features/source-workbench/source-workbench.css`
- Modify: `src/vite-env.d.ts`

**Step 1: Write failing browser-adapter tests**

```ts
it("accepts only a known immutable scenario id");
it("returns a fresh proposal copy for each simulation");
it("rejects arbitrary filesystem paths");
it("has no mutation method");
it("provides included excluded unreadable symlink and out-of-scope outcomes");
```

The named `mixed-overlap-boundaries` scenario must represent:

- two unique included files reached through overlapping directory/file roots;
- one excluded unsupported entry;
- one unreadable entry;
- one symlink;
- one out-of-scope entry;
- a non-zero duplicate encounter count.

**Step 2: Write failing proposal-panel tests**

```tsx
it("shows all five required counts even when zero");
it("groups review details by outcome");
it("shows overlap deduplication separately from excluded count");
it("states that discovery cannot mutate files");
it("renders backend denial without stale success data");
```

**Step 3: Run and verify RED**

```bash
npm test -- --run src/adapters/discovery/browserDiscoveryAdapter.test.ts src/features/source-workbench/DiscoveryProposalPanel.test.tsx
```

Expected: FAIL because the adapter and panel are absent.

**Step 4: Implement the browser-only path**

- Enable only under Vite `browser` or `test` mode.
- Accept `simulateScenario("mixed-overlap-boundaries")`, not path arrays.
- Deep-freeze scenario definitions and return cloned immutable results.
- Provide no Node API, Tauri invoke, browser File System Access API, file input, fetch, or network access.
- Expose a development-only toolbar button labeled “Simulate mixed drop” when browser mode is active.
- Remove the simulator through static mode branching in production builds; never render it in Tauri mode.

**Step 5: Implement proposal presentation**

- Render five labeled counts: Included, Excluded, Unreadable, Symlink, Out of scope.
- Render duplicate encounters separately to avoid category ambiguity.
- Show proposal roots redacted to safe display values supplied by the backend.
- List item outcome and reason code without leaking hidden absolute-path details.
- Offer “Dismiss proposal” only. Do not add approve, archive, rename, move, delete, or import execution buttons in Phase 1.

**Step 6: Verify GREEN**

```bash
npm test -- --run src/adapters/discovery/browserDiscoveryAdapter.test.ts src/features/source-workbench/DiscoveryProposalPanel.test.tsx
npm run test:coverage
npm run build
```

Expected: tests pass, thresholds pass, and the production bundle does not contain scenario labels or the simulation button.

**Step 7: Commit**

```bash
git add src
git commit -m "feat: add safe browser discovery simulator"
```

---

### Task 6: Add browser E2E, cross-platform CI, and final verification

**Files:**

- Create: `e2e/source-workbench.spec.ts`
- Create: `.github/workflows/ci.yml`
- Create: `scripts/verify-no-mutation-capabilities.mjs`
- Create: `scripts/verify-asset-provenance.mjs`
- Create: `docs/ASSET_PROVENANCE.md`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `README.md`

**Step 1: Write the failing Playwright flow**

`e2e/source-workbench.spec.ts` must:

1. Launch `npm run dev:browser`.
2. Verify the narrow rail is adjacent to the source tree at desktop width.
3. Select a directory, deselect `two.txt`, and assert both `sub` and `root/a` are indeterminate.
4. Reselect `two.txt` and assert ancestors are checked.
5. Select the directory and nested file explicitly and assert two unique resolved files.
6. Activate “Simulate mixed drop”.
7. Assert all five required counts and duplicate encounter count.
8. Assert the “no files will be changed” status.
9. Assert no approve/move/rename/archive/delete controls exist.
10. Repeat essential navigation at 200% zoom and by keyboard.

**Step 2: Run and verify RED**

```bash
npm run test:e2e
```

Expected: FAIL until browser startup wiring and assertions are complete.

**Step 3: Add security and provenance checks**

`scripts/verify-no-mutation-capabilities.mjs` must fail if Phase 1 capability/config/source registrations expose prohibited write/shell/process commands. Keep the scan narrow to declared Tauri capabilities and command registration; do not claim that text scanning is a general security proof.

`scripts/verify-asset-provenance.mjs` must compare shipped static assets against `docs/ASSET_PROVENANCE.md` and fail on an unrecorded file. The provenance file records:

- `assets/app-icon.svg`: original project-created geometric mark;
- generated Tauri icon outputs: generated from that original mark;
- no third-party branding, screenshots, proprietary assets, or remote fonts.

Add scripts:

```json
{
  "verify:capabilities": "node scripts/verify-no-mutation-capabilities.mjs",
  "verify:assets": "node scripts/verify-asset-provenance.mjs"
}
```

**Step 4: Add CI**

`.github/workflows/ci.yml` must contain:

- one Ubuntu job for `npm ci`, format, lint, typecheck, unit/component coverage, Playwright, capability check, and asset check;
- a `matrix.os: [ubuntu-latest, macos-latest, windows-latest]` Tauri/Rust job for `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, frontend build, and `npm run tauri build`;
- dependency caching keyed by lockfiles;
- no signing, publishing, upload, network model, or remote runtime credentials.

Platform-specific unreadable/link cases may use the injected test filesystem where native permissions differ, while each platform must still run a real temporary-directory no-mutation test.

**Step 5: Update README without overstating completion**

Document:

- prerequisites;
- `npm ci`;
- `npm run dev:browser`;
- `npm run tauri dev`;
- `npm run verify`;
- how to acquire a discovery scope and test a local Drop;
- Phase 1 implemented requirement claims;
- all explicitly deferred features.

Add an aggregate script:

```json
{
  "verify": "npm run format:check && npm run lint && npm run typecheck && npm run test:coverage && npm run test:e2e && npm run verify:capabilities && npm run verify:assets && cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings && cargo test --manifest-path src-tauri/Cargo.toml"
}
```

**Step 6: Verify GREEN locally**

```bash
npm run verify
npm run build
npm run tauri build
git diff --check
git status --short
```

Expected: every check passes; the current-platform desktop bundle is created; only intended project files are changed.

**Step 7: Commit**

```bash
git add e2e .github scripts docs/ASSET_PROVENANCE.md README.md package.json package-lock.json
git commit -m "test: verify Phase 1 source workbench"
```

---

#### Final requirement-vector audit

**Files:**

- Create: `docs/PHASE1_VERIFICATION.md`
- Modify only if a defect is found: files introduced in Tasks 1–7

**Step 1: Build the traceability table**

In `docs/PHASE1_VERIFICATION.md`, map:

| Vector | Automated evidence | Manual evidence | Status rule |
|---|---|---|---|
| `ARCH-001-target-stack-build-metadata` | lockfiles, Cargo metadata, matrix build | current-platform launch | pass only after CI matrix |
| `UI-005-inspiration-without-copied-assets` | asset inventory check | visual review | no unrecorded assets |
| `UI-006-source-tree-mixed-selection` | pure, component, Playwright tests | keyboard smoke | exact two-leaf resolution |
| `UI-007-mixed-file-directory-drop` | Rust contract tests, adapter tests | OS Drop smoke | one proposal, unique paths |
| `SAFE-009-drop-boundary-review` | Rust manifest comparison, UI counts | text/URL Drop smoke | all outcomes visible, no mutation |

Do not mark Windows/macOS/Linux packaging accepted until the CI matrix has actually passed. Do not mark blocked black-box cases as observed.

**Step 2: Run the complete verification loop**

```bash
npm run verify
npm run build
npm run tauri build
git diff --check
```

**Step 3: Review security boundaries**

Manually inspect:

- command registration contains discovery/grant commands only;
- renderer cannot send allowed roots;
- traversal never follows symlinks;
- containment uses resolved path components;
- proposal categories are disjoint;
- no mutation method exists on `DiscoveryPort`;
- browser scenario code is absent from the Tauri production bundle;
- absolute paths are not written to logs;
- no remote fonts, analytics, fetches, or third-party assets exist.

Fix only Phase 1 defects and rerun the smallest failing check followed by the complete loop.

**Step 4: Commit**

```bash
git add docs/PHASE1_VERIFICATION.md
git commit -m "docs: record Phase 1 verification evidence"
```

## Deferred extension points

- **Archive and SHA-256 identity:** add a separate trusted `ArchivePort` and Rust transaction core. Never add mutation methods to `DiscoveryPort`.
- **Classification and canonical naming:** consume immutable discovery proposal items and produce separate review proposals tied to exact profile versions.
- **Markdown/Mermaid:** mount an editor in the center content slot only after archive-gating and renderer isolation are designed and tested.
- **Graph:** mount evidence-backed relation review in the details slot; do not infer graph claims from the Phase 1 tree.
- **MCP:** expose separate authenticated, bounded tools through local transports only after grants, replay defense, resource limits, and trusted-core routing have their own ADR and tests.
- **Full workbench layout:** implement `UI-002` resize/collapse/split persistence and invalid-data fallback in a later slice.

These are architectural seams, not TODO implementations in Phase 1. Do not add placeholder APIs, database tables, fake buttons, or speculative dependencies for them.
