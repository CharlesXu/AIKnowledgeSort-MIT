# Archive-Gated Authoritative Markdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user create and save authoritative Vault Markdown only from a currently readable, SHA-256-valid registered original produced by a committed archive operation.

**Architecture:** Add a Rust `knowledge` boundary that resolves an opaque archive operation ID against the authoritative Vault, independently re-verifies the registered original, and stores immutable Markdown revisions plus immutable metadata. The React workbench receives committed archive items, explicitly opens a knowledge note, and saves through a typed client; browser adapters reject every persistence call.

**Tech Stack:** Tauri 2 commands, Rust `cap-std`/Serde/SHA-256, React 19/TypeScript, Vitest/Testing Library, Playwright.

---

## File map

- Create `src-tauri/src/knowledge/mod.rs`: Tauri commands and request validation.
- Create `src-tauri/src/knowledge/store.rs`: archive-gated document lookup, append-only revision writes, and original re-verification.
- Modify `src-tauri/src/archive/transaction.rs`: expose a bounded verified-registration projection without exposing source paths to the frontend.
- Modify `src-tauri/src/vault/mod.rs`: create trusted knowledge directories.
- Modify `src-tauri/src/lib.rs`: register the two knowledge commands.
- Create `src/features/knowledge/types.ts`: typed request/result contract.
- Create `src/features/knowledge/knowledgeClient.ts`: Tauri and honest browser adapters.
- Create `src/features/knowledge/knowledgeClient.test.ts`: command-boundary tests.
- Modify `src/features/workbench/ArchivePreviewPane.tsx`: emit successful committed items to the shell.
- Modify `src/features/workbench/ArchivePreviewPane.test.tsx`: verify only committed items become knowledge-eligible.
- Modify `src/features/workbench/DocumentPane.tsx`: explicit note creation/opening, dirty state, optimistic revision save, and errors.
- Create `src/features/workbench/DocumentPane.test.tsx`: archive gating, persistence, conflict, and failure tests.
- Modify `src/app/AppShell.tsx`, `src/App.tsx`, and `src/App.test.tsx`: connect archive results to the knowledge client without changing pane ownership.
- Modify `e2e/source-workbench.spec.ts`: prove browser mode never simulates a saved Vault note.
- Modify `README.md` and this plan: record exact delivered scope and remaining graph work.

### Task 1: Verify a committed registered original at the trusted boundary

- [x] **Step 1: Write failing Rust tests**

Add tests that create a committed archive fixture and assert `verified_registered_original(vault, operation_id)` returns only authority ID, operation ID, Vault-relative original path, canonical display name, format, byte size, and exact `ContentIdentity`. Add negative vectors for an unknown operation ID, a symlinked/replaced original, changed bytes, invalid opaque IDs, and a registration whose operation is not committed.

- [x] **Step 2: Run the focused test and confirm RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test archive::transaction::tests::verified_registered_original --lib`

Expected: FAIL because `verified_registered_original` and its projection do not exist.

- [x] **Step 3: Add the minimal projection and verifier**

Expose a crate-private immutable projection:

```rust
pub(crate) struct VerifiedRegisteredOriginal {
    pub authority_id: String,
    pub operation_id: String,
    pub relative_path: String,
    pub canonical_name: String,
    pub original_format: String,
    pub byte_size: u64,
    pub identity: ContentIdentity,
}
```

Validate the operation ID as a bounded lowercase ASCII opaque ID, read `.aiks/registrations/{operation_id}.json` without following links, find the latest operation journal entry, require `Committed`, compare the registration with the journal-derived registration, and independently hash the Vault-relative original before returning the projection.

- [x] **Step 4: Run the focused and archive suites**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test archive::transaction --lib`

Expected: all archive transaction tests PASS and the source fixture remains byte-identical.

- [x] **Step 5: Commit**

```bash
git add src-tauri/src/archive/transaction.rs
git commit -m "feat: verify registered originals for knowledge"
```

### Task 2: Store authoritative Markdown as immutable Vault revisions

- [x] **Step 1: Write failing store tests**

Cover these observable cases: unregistered/failed archive IDs create no knowledge files; first save with `expectedRevision: 0` creates revision 1; the next save with `expectedRevision: 1` creates revision 2; a stale expected revision is rejected without a third revision; a failed Markdown write or metadata publication leaves the archive original readable and unchanged; reopening returns the highest committed revision and its provenance identity.

- [x] **Step 2: Run the focused test and confirm RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test knowledge::store --lib`

Expected: FAIL because the knowledge store does not exist.

- [x] **Step 3: Implement bounded append-only revisions**

Use these trusted paths:

```text
Knowledge/<operation-id>/00000001.md
.aiks/knowledge/<operation-id>/00000001.json
```

Each metadata record contains schema version, document ID, revision, operation ID, authority ID, Markdown relative path, saved timestamp, literal `SHA-256` Markdown identity, and the verified archived-original identity. Limit Markdown to 1 MiB, revision scans to 10,000 records, and all IDs/path components to validated values. Write Markdown atomically with `write_new_bytes`, then publish metadata with `write_new_json`; metadata is the commit marker, so an orphan Markdown file is never authoritative.

- [x] **Step 4: Add two Tauri commands**

```rust
open_knowledge_document(request: { authority_id, operation_id })
save_knowledge_document(request: { authority_id, operation_id, expected_revision, markdown })
```

Both commands lease the exact current Vault authority. `open` re-verifies the original before returning an existing revision or a deterministic starter document; `save` re-verifies immediately before writing and rejects stale revisions.

- [x] **Step 5: Run Rust verification**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib`

Expected: all Rust tests PASS, including archive-preservation assertions.

- [x] **Step 6: Commit**

```bash
git add src-tauri/src/knowledge src-tauri/src/vault/mod.rs src-tauri/src/lib.rs
git commit -m "feat: persist archive-gated markdown revisions"
```

### Task 3: Add a typed and honest frontend knowledge boundary

- [x] **Step 1: Write failing client tests**

Assert the Tauri adapter invokes only `open_knowledge_document` and `save_knowledge_document` with `{ request }`, and the browser adapter rejects both with `Desktop runtime is required for knowledge operations.`

- [x] **Step 2: Run the focused test and confirm RED**

Run: `npm test -- --run src/features/knowledge/knowledgeClient.test.ts`

Expected: FAIL because the client module does not exist.

- [x] **Step 3: Implement immutable TypeScript contracts and adapters**

Define `KnowledgeDocument` with `documentId`, `authorityId`, `operationId`, `revision`, `markdownPath`, `markdown`, `savedAtUnixMs`, `markdownIdentity`, and `originalIdentity`. Define `openDocument` and `saveDocument` methods; do not add browser fixtures or localStorage fallbacks.

- [x] **Step 4: Run the focused test**

Run: `npm test -- --run src/features/knowledge/knowledgeClient.test.ts`

Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add src/features/knowledge
git commit -m "feat: add trusted knowledge client boundary"
```

### Task 4: Connect confirmed archive results to the central editor

- [x] **Step 1: Write failing component tests**

Verify that failed archive items never appear as knowledge-eligible; a committed item produces an explicit `Create knowledge note` action; opening loads the returned starter or saved revision; edits survive source/live/reading mode changes; save sends the exact authority ID, operation ID, expected revision, and Markdown; success updates revision and clean state; a stale revision or runtime error stays visible and preserves the editor text.

- [x] **Step 2: Run focused tests and confirm RED**

Run: `npm test -- --run src/features/workbench/ArchivePreviewPane.test.tsx src/features/workbench/DocumentPane.test.tsx`

Expected: FAIL because the archive callback and knowledge-aware editor props do not exist.

- [x] **Step 3: Wire the minimum state flow**

`ArchivePreviewPane` calls `onCommittedItems(items, vault)` only after `confirmPlan` resolves, filtering exact `status === "committed"` items. `AppShell` owns the currently selected eligible item and passes it with `KnowledgeClient` to `DocumentPane`. The document pane never accepts a raw path and never enables save until a native open succeeds.

- [x] **Step 4: Preserve the workbench layout**

Keep Archive Preview as the compact top strip, the Markdown/Mermaid/code surface as the central workspace, and the right pane unchanged. Show provenance and revision in the existing document heading rather than adding a new full-workspace stage.

- [x] **Step 5: Run component and application tests**

Run: `npm test -- --run`

Expected: all frontend tests PASS.

- [x] **Step 6: Commit**

```bash
git add src/App.tsx src/App.test.tsx src/app/AppShell.tsx src/features/workbench
git commit -m "feat: open archived originals in knowledge editor"
```

### Task 5: Prove browser honesty and update traceability

- [x] **Step 1: Add a failing browser E2E assertion**

In the browser fixture, assert the document header remains `Local draft · not saved`, no `Saved revision` claim appears, and no knowledge action can bypass the desktop archive prerequisite.

- [x] **Step 2: Run E2E and confirm the new assertion**

Run: `npm run e2e -- --project=chromium`

Expected: all Chromium flows PASS without filesystem mutation claims.

- [x] **Step 3: Update delivery documentation**

Mark only archive-gated authoritative Markdown and provenance as implemented. Keep model-generated knowledge, graph relation review/timeline, physical cleanup, dual-model comparison, and MCP explicitly unimplemented.

- [x] **Step 4: Run the complete release-quality gate**

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

Expected: every command exits 0; the worktree is clean after the final documentation commit.

- [x] **Step 5: Commit**

```bash
git add e2e/source-workbench.spec.ts README.md docs/superpowers/plans/2026-07-28-archive-gated-authoritative-markdown.md
git commit -m "docs: record archive-gated knowledge milestone"
```

## Self-review

- Spec coverage: this plan covers ARCH-002, ARCH-003, FILE-003, KNOW-001, KNOW-002, and SAFE-006 for authoritative Markdown persistence. KNOW-005 graph relations are intentionally a separate next slice.
- Safety boundary: frontend inputs contain opaque authority/operation IDs and Markdown only; Rust resolves and re-verifies trusted paths.
- Failure boundary: no derived-work failure mutates or unregisters an archive original.
- Scope boundary: no model execution, relationship inference, MCP transport, cleanup, or physical rename is added here.
