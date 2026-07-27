# Phase 1 Source Workbench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a runnable Tauri 2 desktop skeleton whose first vertical slice implements the narrow tool rail, source tree, mixed file/directory tri-state selection, and a non-mutating file/directory Drop discovery proposal.

**Architecture:** React and TypeScript own presentation and deterministic selection state. Rust is the trusted boundary for scoped filesystem discovery and returns immutable proposal data; it exposes no mutation command in Phase 1. A typed frontend adapter separates the real Tauri command from deterministic browser and test fixtures.

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Vite, Vitest, Testing Library, Playwright.

---

### Task 1: Runnable application and verification baseline

**Files:**
- Create: `package.json`, `package-lock.json`, `index.html`, `tsconfig.json`, `vite.config.ts`
- Create: `src/main.tsx`, `src/App.tsx`, `src/styles.css`, `src/test/setup.ts`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Create: `src-tauri/capabilities/main.json`
- Create: `THIRD_PARTY_NOTICES.md`
- Test: `src/App.test.tsx`, `src-tauri/src/lib.rs`

- [ ] Write a failing React smoke test asserting the workbench landmark and a Rust unit test asserting the library exposes its application entry.
- [ ] Add the minimal React/Vite and Tauri 2 configuration, with one main window and least-privilege capability configuration.
- [ ] Record direct dependency licenses and avoid third-party visual assets.
- [ ] Run `npm test -- --run`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Commit with `chore: scaffold Tauri source workbench`.

### Task 2: Deterministic source-tree selection domain

**Files:**
- Create: `src/features/sources/types.ts`
- Create: `src/features/sources/selection.ts`
- Test: `src/features/sources/selection.test.ts`

- [ ] Define immutable `SourceNode`, `SelectionState`, and `ResolvedSelection` types. Files are eligible leaves; directories contain ordered children.
- [ ] Write failing tests for selecting a directory, deselecting a child, ancestor `indeterminate` propagation, reselecting the child, and explicit mixed file/directory selection deduplicating to unique eligible files.
- [ ] Implement pure functions `toggleSelection(tree, explicitIds, id, checked)`, `deriveSelectionState(tree, explicitIds, id)`, and `resolveEligibleSelection(tree, explicitIds)` without mutating inputs.
- [ ] Run the focused tests and full frontend test suite.
- [ ] Commit with `feat: add tri-state source selection`.

### Task 3: Trusted non-mutating Drop discovery

**Files:**
- Create: `src-tauri/src/discovery.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/features/drop/types.ts`
- Create: `src/features/drop/discoveryClient.ts`
- Test: `src-tauri/src/discovery.rs`, `src/features/drop/discoveryClient.test.ts`

- [ ] Define Rust `DiscoveryProposal`, `DiscoveredItem`, `DiscoveryCounts`, and categorized diagnostics. Inputs are dropped local paths plus an explicit granted root.
- [ ] Write failing Rust tests using generated temporary trees for overlapping roots, one readable file, an unreadable simulation hook, a symlink, and an out-of-scope path. Assert one deduplicated included set and zero mutation by comparing names, bytes, and metadata before/after.
- [ ] Implement resolved containment checks, `symlink_metadata` rejection, deterministic traversal/order, canonical-path deduplication, and visible included/excluded/unreadable/symlink/out-of-scope counts. Do not add move, rename, archive, delete, or write commands.
- [ ] Expose one narrowly named Tauri command, `propose_local_drop`, and register only that command.
- [ ] Add a typed frontend `DiscoveryClient`; the browser/test adapter accepts generated fixtures while the Tauri adapter invokes the Rust command.
- [ ] Run focused Rust/TypeScript tests and full suites.
- [ ] Commit with `feat: add scoped drop discovery proposals`.

### Task 4: Obsidian-inspired source workbench UI

**Files:**
- Create: `src/app/AppShell.tsx`
- Create: `src/features/sources/ToolRail.tsx`
- Create: `src/features/sources/SourceTree.tsx`
- Create: `src/features/sources/SourceTreeRow.tsx`
- Create: `src/features/drop/DropProposalPanel.tsx`
- Create: `src/features/workbench/DocumentPane.tsx`
- Create: `src/features/workbench/ContextPane.tsx`
- Create: `src/ui/Icon.tsx`
- Create: `src/data/demoSources.ts`
- Modify: `src/App.tsx`, `src/styles.css`
- Test: `src/features/sources/SourceTree.test.tsx`, `src/features/drop/DropProposalPanel.test.tsx`, `src/App.test.tsx`

- [ ] Write failing component tests for the narrow tool rail, adjacent source tree, directory/file checkboxes, indeterminate rendering, deduplicated selection summary, and all five proposal counts.
- [ ] Build an original, restrained light UI: 44px tool rail, 286px source panel, resizable three-pane workbench, compact typography, neutral white/gray surfaces, muted violet selection accent, and no copied branding or assets.
- [ ] Ensure keyboard focus, accessible labels, disclosure controls, checkbox semantics, minimum contrast, and responsive collapse below laptop width.
- [ ] Keep Markdown editor, graph, classification, archive, and MCP controls as honest disabled/deferred regions, not fake functionality.
- [ ] Run component tests, typecheck, and production build.
- [ ] Commit with `feat: build source workbench interface`.

### Task 5: Native Drop integration and rendered acceptance

**Files:**
- Create: `src/features/drop/useNativeDrop.ts`
- Modify: `src/app/AppShell.tsx`, `src/features/drop/DropProposalPanel.tsx`
- Create: `e2e/source-workbench.spec.ts`, `playwright.config.ts`
- Modify: `package.json`
- Test: `src/features/drop/useNativeDrop.test.tsx`, `e2e/source-workbench.spec.ts`

- [ ] Write failing tests that simulate mixed local file/directory drops, overlapping roots, cancellation, external text, and URL drag data; assert only local path drops call discovery.
- [ ] Subscribe with Tauri 2 `getCurrentWebview().onDragDropEvent`, clean up the listener on unmount, and keep the browser adapter injectable.
- [ ] Render an explicit hover state, then a review-only proposal. No proposal action may invoke filesystem mutation.
- [ ] Run all frontend and Rust tests, `npm run build`, and `cargo check --manifest-path src-tauri/Cargo.toml`.
- [ ] Start the Vite surface and use the available browser workflow to verify: app load, no overlay, no console errors, directory tri-state interaction, proposal counts, desktop layout, and a narrower viewport.
- [ ] Capture a temporary screenshot outside the repository and inspect it for layout, typography, clipping, focus, and fidelity to the textual clean specification.
- [ ] Commit with `test: verify source workbench flow`.

## Phase 1 completion gate

- [ ] A spec reviewer confirms UI-006, UI-007, and SAFE-009 without scope expansion.
- [ ] A code-quality reviewer finds no Critical or Important issue.
- [ ] A security reviewer confirms the trusted boundary is read-only and scope-resolved.
- [ ] `npm test -- --run`, `npm run build`, `cargo test`, `cargo check`, and the E2E flow pass.
- [ ] The implementation branch contains no upstream source, copied branding, screenshot asset, absolute private path, secrets, or destructive filesystem command.

## Deferred, not stubbed as working

Markdown/Mermaid editing, graph visualization, classification profiles, archival transactions, canonical naming, cleanup, model adjudication, and MCP transports remain later vertical slices. Phase 1 may reserve layout regions and typed extension boundaries but shall not claim these capabilities are implemented.
