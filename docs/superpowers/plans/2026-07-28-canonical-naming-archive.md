# Canonical Naming and Archive Binding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add evidence-grounded, deterministic canonical-name proposals and bind an exact approved proposal into the existing source-preserving archive transaction.

**Architecture:** Models, Agents, and users may supply cited naming facts, but only the Rust trusted core normalizes those facts, detects missing or conflicting evidence, resolves namespace collisions, and creates a proposal. An in-memory, bounded, expiring naming registry binds proposals to reviewed discovery items; archive planning consumes that binding rather than accepting frontend-supplied filenames. The committed archive registration records both the original and canonical names plus the exact naming policy and evidence, while the source file remains untouched.

**Tech Stack:** Rust 2021, serde, SHA-256 identity, `unicode-normalization`, cap-std, Tauri 2, React 19, TypeScript 7, Vitest, Playwright.

---

## Scope boundary

This slice implements local evidence entry, deterministic proposals, review routing, collision handling, and canonical archive filenames. It does not call a model, change the source filename, delete a source, import profiles from URLs, or expose MCP. Later desktop-model and Agent integrations must populate the same cited-fact contract and cannot bypass the naming registry or archive confirmation.

## Locked file map

- `src-tauri/src/naming/schema.rs`: strict cited facts, policy, proposal, and validation types.
- `src-tauri/src/naming/normalize.rs`: Unicode normalization, extension preservation, reserved-name checks, and deterministic collision resolution.
- `src-tauri/src/naming/registry.rs`: bounded expiring proposal batches bound to reviewed items.
- `src-tauri/src/naming/mod.rs`: DTO exports and Tauri command.
- `src-tauri/src/archive/plan.rs`: consumes an exact naming batch and embeds naming evidence in plan items.
- `src-tauri/src/archive/transaction.rs`: persists naming audit fields in operation and registration records.
- `src-tauri/src/archive/mod.rs`: archive command wiring only.
- `src-tauri/src/lib.rs`: managed registry and command registration only.
- `src/features/naming/types.ts`: frontend naming contracts.
- `src/features/naming/namingClient.ts`: honest Tauri/browser client boundary.
- `src/features/workbench/ArchivePreviewPane.tsx`: compact per-file evidence entry and canonical-name review.
- `src/features/archive/types.ts`: archive request and plan naming bindings.
- `src/styles.css`: compact naming rows that do not displace the Markdown workspace.
- `e2e/source-workbench.spec.ts`: browser non-mutation and runtime-error acceptance.

### Task 1: Strict naming facts and policy contract

**Files:**
- Create: `src-tauri/src/naming/schema.rs`
- Create: `src-tauri/src/naming/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: inline tests in `src-tauri/src/naming/schema.rs`

- [ ] **Step 1: Write failing schema tests**

Define a valid request fixture:

```rust
NamingRequest {
    item_id: "item-1".into(),
    original_name: "000123.pdf".into(),
    identity: identity(),
    facts: vec![
        fact(NamingFactKind::Project, "Atlas", "page:1"),
        fact(NamingFactKind::Model, "X100", "page:1"),
        fact(NamingFactKind::Version, "V2.1", "page:2"),
        fact(NamingFactKind::Subject, "Reset reliability", "page:1"),
    ],
    occupied_names: vec![],
}
```

Assert strict deserialization and validation reject unknown fields, empty or oversized values, control characters, invalid SHA-256, duplicate exact facts, more than 64 facts, more than 10,000 occupied names, path-like original names, and executable-shaped fields such as `command` or `template`.

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test naming::schema --lib
```

Expected: compilation fails because `naming` does not exist.

- [ ] **Step 3: Implement strict DTOs**

Create:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NamingFactKind {
    Project,
    Model,
    Regulation,
    Version,
    Subject,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamingFact {
    pub kind: NamingFactKind,
    pub value: String,
    pub evidence_location: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamingRequest {
    pub item_id: String,
    pub original_name: String,
    pub identity: ContentIdentity,
    pub facts: Vec<NamingFact>,
    pub occupied_names: Vec<String>,
}
```

Add `NamingPolicy { policy_id, version, required_facts, separator }` and a code-owned `canonical-v1` policy requiring `subject`. Bounds are constants and every ingress struct uses `deny_unknown_fields`. Reuse literal `ContentIdentity::validate`.

- [ ] **Step 4: Verify**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test naming::schema --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
```

Expected: schema tests pass and Clippy has no warnings.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/naming src-tauri/src/lib.rs
git commit -m "feat: define canonical naming evidence contract"
```

### Task 2: Deterministic Unicode-safe proposal engine

**Files:**
- Create: `src-tauri/src/naming/normalize.rs`
- Modify: `src-tauri/src/naming/mod.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `THIRD_PARTY_NOTICES.md`
- Test: inline tests in `src-tauri/src/naming/normalize.rs`

- [ ] **Step 1: Write failing proposal tests**

Cover:

1. `000123.pdf` plus cited Atlas/X100/V2.1/reset-reliability facts produces `Atlas-X100-V2.1-Reset-reliability.pdf`;
2. the exact original extension, including case, is preserved;
3. two distinct normalized values for the same fact kind produce `namingReview` with `conflictingEvidence` and no canonical name;
4. missing subject produces `namingReview` with `missingEvidence`;
5. Unicode composed and decomposed inputs produce the same NFC name;
6. unsafe characters collapse to one separator without dropping meaningful tokens;
7. Windows-reserved names and empty normalized stems route to review;
8. a case-insensitive occupied-name collision appends `--<first eight SHA-256 hex>` before the extension;
9. if the digest-suffixed name is also occupied, the result is `namingReview`, not an invented counter.

- [ ] **Step 2: Run and verify RED**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test naming::normalize --lib
```

Expected: compilation fails because `propose_name` does not exist.

- [ ] **Step 3: Add Unicode normalization dependency**

Add:

```toml
unicode-normalization = "0.1"
```

Record `unicode-normalization` and its SPDX license in `THIRD_PARTY_NOTICES.md`.

- [ ] **Step 4: Implement the pure engine**

Expose:

```rust
pub fn propose_name(
    policy: &NamingPolicy,
    request: &NamingRequest,
) -> Result<NamingProposal, String>;
```

`NamingProposal` records:

```rust
pub struct NamingProposal {
    pub proposal_id: String,
    pub item_id: String,
    pub original_name: String,
    pub canonical_name: Option<String>,
    pub identity: ContentIdentity,
    pub policy_id: String,
    pub policy_version: String,
    pub applied_rule: String,
    pub status: NamingStatus,
    pub review_reason: Option<NamingReviewReason>,
    pub facts: Vec<NamingFact>,
}
```

Normalize each fact to NFC, trim, replace runs of whitespace and unsafe filename punctuation with one policy separator, and preserve Unicode letters and numbers. Sort facts in the fixed order project, model, regulation, version, subject; do not reorder within a value. Detect conflicts before joining. Compare occupied names using Unicode lowercase. The collision suffix is derived only from the already-recorded content digest.

- [ ] **Step 5: Verify**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test naming::normalize --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
```

Expected: proposal vectors and the full Rust suite pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/naming THIRD_PARTY_NOTICES.md
git commit -m "feat: propose deterministic canonical filenames"
```

### Task 3: Exact reviewed-item naming batches

**Files:**
- Create: `src-tauri/src/naming/registry.rs`
- Modify: `src-tauri/src/naming/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: inline tests in `src-tauri/src/naming/registry.rs`

- [ ] **Step 1: Write failing registry tests**

Assert the registry:

- accepts only item IDs resolved from one live `ReviewedSourceRegistry` proposal;
- replaces request names and identities with the trusted reviewed-source values;
- returns both proposed and review outcomes for UI inspection;
- binds the batch ID to exact item IDs, identities, policy version, and proposals;
- expires after five minutes, is single-use when consumed by archive planning, and rejects unknown, replayed, or mismatched selections;
- caps live batches at 32 and items per batch at 1,000.

- [ ] **Step 2: Run and verify RED**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test naming::registry --lib
```

Expected: compilation fails because `NamingBatchRegistry` is absent.

- [ ] **Step 3: Implement the bounded registry**

Define:

```rust
pub struct NamingBatch {
    pub batch_id: String,
    pub discovery_proposal_id: String,
    pub policy_id: String,
    pub policy_version: String,
    pub expires_at_unix_ms: u64,
    pub proposals: Vec<NamingProposal>,
}
```

Use `Arc<Mutex<HashMap<String, StoredBatch>>>`, `Instant` expiry, generated opaque UUID IDs, exact set comparison, and remove-on-consume semantics matching `ArchivePlanRegistry`.

- [ ] **Step 4: Add the Tauri command**

Expose:

```rust
#[tauri::command]
pub fn create_naming_batch(
    request: CreateNamingBatchRequest,
    reviewed_sources: tauri::State<'_, ReviewedSourceRegistry>,
    batches: tauri::State<'_, NamingBatchRegistry>,
) -> Result<NamingBatch, String>;
```

The request contains `proposalId` and per-item cited facts only. Original name, path, byte size, and identity always come from the trusted reviewed-source registry.

- [ ] **Step 5: Verify and commit**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test naming --lib
git add src-tauri/src/naming src-tauri/src/lib.rs
git commit -m "feat: bind canonical names to reviewed sources"
```

### Task 4: Bind canonical names into verified archive commit

**Files:**
- Modify: `src-tauri/src/archive/mod.rs`
- Modify: `src-tauri/src/archive/plan.rs`
- Modify: `src-tauri/src/archive/transaction.rs`
- Modify: `src-tauri/src/naming/registry.rs`
- Test: inline archive plan and transaction tests

- [ ] **Step 1: Write failing archive-binding tests**

Assert:

1. archive planning requires a live naming batch for every selected item;
2. any `namingReview` item blocks plan creation without mutation;
3. item, identity, original name, or discovery-proposal mismatch rejects the batch;
4. the destination is `Originals/<digest>/<canonical-name>`;
5. a committed operation and registration record contain original name/path, canonical name/path, policy ID/version, applied rule, facts/evidence locations, confirmation binding, identity, and committed outcome;
6. a failed copy or registration leaves no canonical destination and no committed naming audit;
7. recovery verifies naming fields together with the destination identity.

- [ ] **Step 2: Run and verify RED**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test archive --lib
```

Expected: new assertions fail because archive plans still use original names.

- [ ] **Step 3: Extend archive request and plan item**

Change:

```rust
pub struct CreateArchivePlanRequest {
    proposal_id: String,
    item_ids: Vec<String>,
    naming_batch_id: String,
}

pub struct ArchivePlanItem {
    // existing fields
    pub original_name: String,
    pub canonical_name: String,
    pub naming: NamingDecisionEvidence,
}
```

The archive command consumes the naming batch and passes the bound proposals to `ArchivePlanRegistry::create_at`. It never accepts a canonical name from the frontend.

- [ ] **Step 4: Persist the audit binding**

Add the same immutable `NamingDecisionEvidence` to `OperationRecord` and `OriginalRegistration`. Derive original format from `original_name`, not an untrusted path. Include the serialized naming evidence in the confirmation-binding hash together with plan ID, item ID, source/destination, and identity.

- [ ] **Step 5: Verify and commit**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test archive --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
git add src-tauri/src/archive src-tauri/src/naming
git commit -m "feat: commit canonical names with archive audit"
```

### Task 5: Typed client and compact archive naming review

**Files:**
- Create: `src/features/naming/types.ts`
- Create: `src/features/naming/namingClient.ts`
- Create: `src/features/naming/namingClient.test.ts`
- Modify: `src/features/archive/types.ts`
- Modify: `src/features/archive/archiveClient.ts`
- Modify: `src/features/archive/archiveClient.test.ts`
- Modify: `src/features/workbench/ArchivePreviewPane.tsx`
- Modify: `src/features/workbench/ArchivePreviewPane.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/app/AppShell.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write failing typed-client tests**

Assert:

```ts
["create_naming_batch", {
  request: {
    proposalId: "proposal-1",
    items: [{
      itemId: "item-1",
      facts: [{
        kind: "subject",
        value: "Reset reliability",
        evidenceLocation: "page:1",
      }],
    }],
  },
}]
```

The browser naming client must reject with `Desktop runtime is required for naming operations.` and never return a fake proposal.

- [ ] **Step 2: Implement typed clients**

Mirror Rust DTOs exactly. Extend `CreateArchivePlanRequest` with `namingBatchId`. Keep both browser clients mutation-free.

- [ ] **Step 3: Write failing UI tests**

With mocked clients, assert:

- selecting an item reveals compact Project, Model, Regulation, Version, Subject, and Evidence location inputs;
- Subject and evidence location are required before requesting a batch;
- a review outcome shows its reason and disables archive-plan creation;
- a valid proposal shows `original → canonical`, policy/version, and SHA-256;
- archive-plan creation sends only the returned `namingBatchId`, proposal ID, and selected item IDs;
- the existing exact-plan confirmation remains required before commit.

- [ ] **Step 4: Implement compact UI**

Keep evidence rows inside `ArchivePreviewPane`; do not create a full-workspace wizard. Label manual entries as `Local evidence` so later model/Agent suggestions can populate distinguishable sources. Render strings as text only.

- [ ] **Step 5: Verify and commit**

```bash
npm test -- --run src/features/naming src/features/workbench/ArchivePreviewPane.test.tsx
npm test -- --run
npm run build
git add src
git commit -m "feat: review canonical names before archive"
```

### Task 6: Browser acceptance, documentation, and milestone audit

**Files:**
- Modify: `e2e/source-workbench.spec.ts`
- Modify: `README.md`

- [ ] **Step 1: Add browser non-mutation E2E**

Select one source, fill the required local evidence, and request naming in the browser fixture. Assert the visible desktop-runtime error, unchanged original name, no canonical archive plan, and unchanged `0 changes` status.

- [ ] **Step 2: Document the shipped boundary**

State that deterministic local canonical-name proposals and archive audit binding are implemented. Explicitly state that model-generated evidence, Agent adjudication, source renaming, cleanup, MCP, and URL profile import remain unimplemented.

- [ ] **Step 3: Run the full verification loop**

```bash
npm test -- --run
npm run build
npm audit --audit-level=high
npm run e2e
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo fmt --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

Expected: all tests pass, audit reports no high vulnerabilities, and no whitespace errors exist.

- [ ] **Step 4: Run clean-room leakage checks**

```bash
rg -n '/Users/charles|KLOrgFromFile|ai-file-sorter/.+src|github\.com/hyperfield/ai-file-sorter/.+blob' \
  src src-tauri README.md THIRD_PARTY_NOTICES.md
```

Expected: no private path or contaminated-source pointer.

- [ ] **Step 5: Commit**

```bash
git add e2e/source-workbench.spec.ts README.md
git commit -m "docs: record canonical naming milestone"
```

## Self-review

- **Spec coverage:** Task 1 covers strict inputs; Task 2 covers NAME-001, NAME-002, NAME-003, and missing/conflicting evidence under NAME-004; Tasks 3–4 bind proposals and committed audit evidence under NAME-004, FILE-001, and SAFE-004; Task 5 exposes the review and confirmation path; Task 6 verifies non-mutation.
- **Explicit gaps:** This plan does not generate evidence with a model, rename a source, clean duplicates, expose MCP, or import a profile URL. Those remain separate vertical slices.
- **Type consistency:** `itemId`, `proposalId`, `batchId`, `namingBatchId`, `canonicalName`, `policyId`, `policyVersion`, `appliedRule`, and `evidenceLocation` remain fixed across Rust, TypeScript, UI, and archive records.
- **Safety boundary:** The frontend never supplies a canonical filename to archive planning. Only a single-use registry batch created from trusted reviewed-source identity may advance.
- **Placeholder scan:** No task contains TBD, TODO, “similar to,” or an undefined implementation placeholder.
