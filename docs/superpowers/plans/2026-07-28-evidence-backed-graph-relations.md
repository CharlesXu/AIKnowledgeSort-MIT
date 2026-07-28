# Evidence-Backed Graph Relations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add locally reviewable graph relations whose claims are bound to exact committed Markdown lines and archived-original provenance, with a compact creation timeline in the right pane.

**Architecture:** A Rust `graph` store creates immutable relation versions inside the authoritative Vault. Requests identify a committed knowledge revision and line ranges; Rust reopens that revision, extracts the evidence text itself, and refuses missing, stale, or unverifiable evidence. React exposes a typed honest boundary, review controls, a bounded node-link view, and a 34 px timeline driven only by persisted relation events.

**Tech Stack:** Tauri 2, Rust `cap-std`/Serde/SHA-256/UUID, React 19/TypeScript, Vitest/Testing Library, Playwright.

---

## File map

- Modify `src-tauri/src/knowledge/store.rs`: expose exact committed-revision lookup for evidence verification.
- Create `src-tauri/src/graph/mod.rs`: strict command requests and per-document/relation write serialization.
- Create `src-tauri/src/graph/store.rs`: immutable relation versions, exact-line evidence extraction, review transitions, and bounded inspection.
- Modify `src-tauri/src/vault/mod.rs`: initialize `.aiks/graph` and `.aiks/graph/relations`.
- Modify `src-tauri/src/lib.rs`: manage graph write state and register three graph commands.
- Create `src/features/graph/types.ts`: graph relation, evidence, event, request, and client types.
- Create `src/features/graph/graphClient.ts` and `graphClient.test.ts`: Tauri adapter and honest browser rejection.
- Create `src/features/workbench/KnowledgeGraphPane.tsx` and `.test.tsx`: relationship editor/review, node-link projection, evidence inspector, and 34 px timeline.
- Modify `src/features/workbench/DocumentPane.tsx`: emit the currently opened/saved authoritative document.
- Modify `src/features/workbench/ContextPane.tsx`, `src/app/AppShell.tsx`, `src/App.tsx`, and tests: route only authoritative documents into the real graph pane.
- Modify `src/styles.css`, `e2e/source-workbench.spec.ts`, and `README.md`: compact layout, browser honesty, and exact delivery claims.

### Task 1: Reopen one exact authoritative Markdown revision

- [ ] **Step 1: Write the failing Rust test**

Add a store test that saves revisions 1 and 2, then asserts `open_committed_revision(vault, operation_id, 1)` returns revision 1 unchanged while revision 0, revision 3, a changed archived original, and tampered Markdown fail.

- [ ] **Step 2: Run RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test knowledge::store::tests::opens_one_exact_committed_revision --lib`

Expected: FAIL because `open_committed_revision` does not exist.

- [ ] **Step 3: Implement exact revision lookup**

Scan the bounded metadata namespace, require the requested nonzero revision, re-run `verified_registered_original`, validate metadata authority/operation/path/identities, no-follow read the UTF-8 Markdown, and independently recompute its SHA-256.

- [ ] **Step 4: Run GREEN and commit**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test knowledge::store --lib`

```bash
git add src-tauri/src/knowledge/store.rs
git commit -m "feat: reopen exact knowledge revisions"
```

### Task 2: Persist evidence-backed relation versions

- [ ] **Step 1: Write failing graph store tests**

Test a proposal with source node, relation type, target node, and line range `2..=3`. Assert Rust records the exact extracted text, knowledge revision/identity, original identity, operation ID, stable relation ID, version 1, `review` status, actor, and timestamp. Reject zero ranges, reversed/out-of-bounds ranges, empty extracted text, stale knowledge revision, unregistered originals, control characters, oversized fields, and unknown IDs without graph mutation.

- [ ] **Step 2: Run RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test graph::store --lib`

Expected: FAIL because the graph module does not exist.

- [ ] **Step 3: Implement strict types and storage**

Use:

```text
.aiks/graph/relations/<relation-id>/00000001.json
```

Each `GraphRelation` exposes `relationId`, `version`, `authorityId`, `operationId`, `knowledgeRevision`, `sourceNode`, `relationType`, `targetNode`, `status`, `evidence[]`, `actor`, `reason`, and `recordedAtUnixMs`. Each evidence record includes exact start/end lines, Rust-extracted text, Markdown SHA-256, and archived-original SHA-256. Limit nodes/type/reason to 160/80/512 characters, evidence to 16 ranges, relations to 10,000, and versions to 100.

- [ ] **Step 4: Implement non-replayable review transitions**

`accept` and `reject` append a terminal version only when `expectedVersion` matches the latest `review` version. `revise` requires replacement nodes/type/ranges, re-extracts evidence from the same exact knowledge revision, and appends another `review` version under the same relation ID. Terminal relations reject further decisions. The original archive and Markdown remain untouched.

- [ ] **Step 5: Implement inspection and timeline events**

Inspection returns the latest version of every relation for one operation plus an ordered event per persisted version. Events contain relation ID, version, status, source/target, and recorded time; sorting is `(recordedAtUnixMs, relationId, version)`.

- [ ] **Step 6: Run GREEN and commit**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test graph --lib`

```bash
git add src-tauri/src/graph src-tauri/src/vault/mod.rs src-tauri/src/lib.rs
git commit -m "feat: persist evidence-backed graph relations"
```

### Task 3: Add three typed graph commands

- [ ] **Step 1: Write failing frontend client tests**

Assert `inspect_knowledge_graph`, `propose_graph_relation`, and `decide_graph_relation` receive only `{ request }`. Assert browser adapters reject all three with `Desktop runtime is required for graph operations.`

- [ ] **Step 2: Add command requests and write serialization**

Commands lease the exact authority and expose:

```text
inspect(authorityId, operationId)
propose(authorityId, operationId, knowledgeRevision, sourceNode, relationType, targetNode, evidenceRanges)
decide(authorityId, relationId, expectedVersion, decision, reason, optional revision fields)
```

Serialize proposal writes per Vault document and decisions per relation. The frontend never supplies evidence text, paths, identities, actor, timestamps, relation IDs, or status.

- [ ] **Step 3: Run client and Rust tests, then commit**

Run: `npm test -- --run src/features/graph/graphClient.test.ts`

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib`

```bash
git add src/features/graph src-tauri/src/graph/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add trusted graph command boundary"
```

### Task 4: Replace the placeholder with a real evidence graph

- [ ] **Step 1: Write failing component tests**

With no authoritative document, keep `Proposal topology` and its disabled timeline. With a saved document, load the graph, propose using line ranges, show source/type/target nodes and evidence text, accept/reject with exact version and reason, revise back into review, preserve visible errors, and never mutate editor text.

- [ ] **Step 2: Lift authoritative document state**

Add `onDocumentChange(document | null)` to `DocumentPane`; emit only after a successful native open/save and clear on target change. `AppShell` passes that document and `GraphClient` to `ContextPane`.

- [ ] **Step 3: Implement the compact graph pane**

Render a bounded two-dimensional node-link projection from current persisted relations, an accessible relation list, evidence inspector, and review buttons. The proposal form accepts nodes/type and start/end line numbers; it never asks for evidence text. Keep profile review as a separate tab.

- [ ] **Step 4: Implement timeline playback**

Render exactly one 34 px `.knowledge-timeline` with play/pause, range, and 1x/2x selector. Timeline position filters the projection by persisted event time; relation labels remain visible, and the status text gives the active event/time. Empty graph disables playback.

- [ ] **Step 5: Run GREEN and commit**

Run: `npm test -- --run`

```bash
git add src/App.tsx src/app/AppShell.tsx src/features/workbench src/styles.css
git commit -m "feat: review graph relations in the workbench"
```

### Task 5: Verify honesty, dimensions, and delivery scope

- [ ] **Step 1: Extend browser E2E**

Assert the browser fixture still shows proposal topology, has no `Add relation` control, no accepted relation, and a disabled timeline. Its local draft remains unsaved.

- [ ] **Step 2: Document the milestone**

README shall claim manual evidence-backed relation proposal/review and persisted timeline only. Model inference, GraphRAG indexing, 3D graph, dual-model comparison, MCP, and cleanup remain explicitly unimplemented.

- [ ] **Step 3: Run the complete gate**

```bash
npm test -- --run
npm run build
npm audit --audit-level=high
npm run e2e -- --project=chromium
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo fmt --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

- [ ] **Step 4: Commit**

```bash
git add README.md e2e/source-workbench.spec.ts docs/superpowers/plans/2026-07-28-evidence-backed-graph-relations.md
git commit -m "docs: record evidence graph milestone"
```

## Self-review

- Spec coverage: implements KNOW-005 and UI-004 evidence inspection, review decisions, persisted graph browsing, and a 34 px time projection. Existing KNOW-001/002 provenance is reused and revalidated.
- Safety: no request accepts evidence text, filesystem paths, identities, actor, timestamps, status, or relation IDs for creation. Rust derives those from the authoritative Vault.
- Scope: model inference, GraphRAG retrieval, 3D rendering, MCP, and filesystem mutation are excluded and remain visibly unclaimed.
- Type consistency: `knowledgeRevision`, `evidenceRanges`, `expectedVersion`, and `recordedAtUnixMs` retain identical names through Rust camelCase serialization and TypeScript clients.
