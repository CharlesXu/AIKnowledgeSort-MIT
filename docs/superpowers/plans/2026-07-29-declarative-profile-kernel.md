# Declarative Profile Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a persistent, non-executable classification-profile authority that can inspect the bundled Ninebot draft, import a local candidate for review, record an explicit immutable decision, and produce exact-version classification or dedicated-review proposals without changing source files.

**Architecture:** Rust remains the trusted authority. Strict `serde` data types with `deny_unknown_fields` define the only accepted JSON profile representation; immutable Vault records hold imported bytes, candidates, decisions, and installed versions. A pure proposal engine evaluates literal evidence terms and returns either one exact destination or `classificationReview`; the React Import Review tab calls only typed Tauri commands and never simulates activation in browser mode.

**Tech Stack:** Rust 2024, serde/serde_json, SHA-256 identity, cap-std Vault capabilities, Tauri 2 commands/dialog, React 19, TypeScript 7, Vitest, Playwright.

---

## Scope boundary

This plan implements local-file profile import and review only. It deliberately does not implement URL import, model-generated candidate authoring, canonical naming, archive-path replacement, or full Ninebot taxonomy material. Those are separate security and domain-data plans. The bundled Ninebot entry is a manifest-only draft shell because the clean implementation handoff contains the draft-status requirement but not the owned taxonomy rows; it must never classify or become approved without later reviewed content.

## Locked file map

- `src-tauri/src/profiles/schema.rs`: strict declarative profile representation and validation.
- `src-tauri/src/profiles/proposal.rs`: deterministic literal-evidence matching and conflict review.
- `src-tauri/src/profiles/store.rs`: Vault-backed immutable sources, candidates, decisions, installations, and active-profile state.
- `src-tauri/src/profiles/mod.rs`: registry, Tauri commands, DTO exports, and bundled Ninebot draft shell.
- `src-tauri/src/vault/mod.rs`: profile directories and a lease for profile operations.
- `src-tauri/src/lib.rs`: state and command registration only.
- `src/features/profiles/types.ts`: frontend profile contracts.
- `src/features/profiles/profileClient.ts`: Tauri and honest browser adapters.
- `src/features/profiles/ProfileReview.tsx`: Import Review profile workflow.
- `src/features/workbench/ContextPane.tsx`: mounts Profile Review in the existing Import Review tab.
- `src/styles.css`: compact Obsidian-inspired review styling.
- `e2e/source-workbench.spec.ts`: browser non-mutation acceptance.

### Task 1: Strict declarative profile schema

**Files:**
- Create: `src-tauri/src/profiles/schema.rs`
- Create: `src-tauri/src/profiles/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: inline unit tests in `src-tauri/src/profiles/schema.rs`

- [ ] **Step 1: Write failing schema tests**

Add tests that deserialize this valid representation:

```rust
{
  "schemaVersion": 1,
  "profileId": "fixture-profile",
  "version": "1.0.0",
  "title": "Fixture profile",
  "status": "candidate",
  "provenance": {
    "sourceTitle": "Owned fixture",
    "ownership": "owned",
    "evidence": ["authorization:test"]
  },
  "rules": [{
    "ruleId": "fixture.report",
    "destination": ["01-Research", "Reports"],
    "allOf": [{"kind": "documentText", "term": "quarterly report"}]
  }]
}
```

Assert that empty identifiers, absolute/parent destination segments, duplicate rule IDs, empty evidence, an imported `approved` status, unknown fields such as `command`, and payloads larger than 1 MiB are rejected.

- [ ] **Step 2: Run the schema test and verify RED**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::schema --lib
```

Expected: compilation fails because the `profiles` module does not exist.

- [ ] **Step 3: Implement the strict types and validator**

Define:

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclarativeProfile {
    pub schema_version: u32,
    pub profile_id: String,
    pub version: String,
    pub title: String,
    pub status: ProfileStatus,
    pub provenance: ProfileProvenance,
    pub rules: Vec<ClassificationRule>,
}
```

Use enums limited to `draft`, `candidate`, `approved`, and `rejected`; evidence kinds limited to `documentText`, `ocrText`, `transcript`, and `reliableCompanion`. Validate opaque IDs, bounded strings and collection sizes, literal non-empty terms, relative destination segments, unique rule IDs, and candidate-ingress status. Parse from a byte slice only after enforcing the 1 MiB bound. Do not add expression, regex, script, template evaluation, or generic instruction fields.

- [ ] **Step 4: Run schema tests and Clippy**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::schema --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all schema tests pass and Clippy emits no warning.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/profiles src-tauri/src/lib.rs
git commit -m "feat: define declarative profile schema"
```

### Task 2: Exact-version classification and dedicated review

**Files:**
- Create: `src-tauri/src/profiles/proposal.rs`
- Modify: `src-tauri/src/profiles/mod.rs`
- Test: inline unit tests in `src-tauri/src/profiles/proposal.rs`

- [ ] **Step 1: Write failing proposal tests**

Cover:

```rust
let evidence = EvidencePacket {
    source_identity: identity(),
    references: vec![EvidenceReference {
        kind: EvidenceKind::DocumentText,
        location: "page:3".into(),
        text: "Quarterly report for Project Atlas".into(),
    }],
};
```

Assert:

1. one matching rule returns `proposed`, exact `profileId`, exact `profileVersion`, rule ID, evidence location, destination, and `committable == false` for a draft;
2. two different matching destinations return `classificationReview`, both rule IDs, no destination, and no catch-all path;
3. no match returns `classificationReview` with reason `missingEvidence`;
4. approved status alone makes a single structurally valid match committable.

- [ ] **Step 2: Run and verify RED**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::proposal --lib
```

Expected: compilation fails because proposal types are absent.

- [ ] **Step 3: Implement literal matching**

Define:

```rust
pub struct ClassificationProposal {
    pub proposal_id: String,
    pub source_identity: ContentIdentity,
    pub profile_id: String,
    pub profile_version: String,
    pub status: ProposalStatus,
    pub rule_ids: Vec<String>,
    pub evidence: Vec<EvidenceCitation>,
    pub destination: Option<Vec<String>>,
    pub review_reason: Option<ReviewReason>,
    pub committable: bool,
}
```

Normalize evidence and terms with lowercase Unicode text and require every `allOf` term to occur in a reference of the declared kind. Group matches by destination. Exactly one destination produces `proposed`; zero or more than one produces `classificationReview`. Never produce a filesystem path for review outcomes.

- [ ] **Step 4: Verify**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::proposal --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
```

Expected: proposal tests and the complete Rust suite pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/profiles
git commit -m "feat: add exact profile classification proposals"
```

### Task 3: Vault-backed candidate and decision authority

**Files:**
- Create: `src-tauri/src/profiles/store.rs`
- Modify: `src-tauri/src/profiles/mod.rs`
- Modify: `src-tauri/src/vault/mod.rs`
- Test: inline unit tests in `src-tauri/src/profiles/store.rs`

- [ ] **Step 1: Write failing persistence tests**

Create a temporary Vault and assert:

1. the Ninebot shell is installed as `draft`, has zero rules, and cannot be activated;
2. importing valid bytes stores one source keyed by SHA-256 and one unapproved candidate with a reviewable added/removed/changed rule-ID diff;
3. the persisted local locator is a SHA-256 locator digest plus basename, never the absolute source path;
4. repeated identical import is idempotent;
5. approve and reject decisions require the exact candidate digest, persist actor/time/decision/reviewed digest, and cannot be replayed;
6. approval installs an immutable `approved` version and appends an activation record;
7. malformed or executable-shaped input creates no source, candidate, decision, installation, or activation record.

- [ ] **Step 2: Run and verify RED**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::store --lib
```

Expected: compilation fails because `ProfileStore` is absent.

- [ ] **Step 3: Extend Vault product directories**

Add:

```text
.aiks/profiles
.aiks/profiles/sources
.aiks/profiles/candidates
.aiks/profiles/decisions
.aiks/profiles/installed
.aiks/profiles/activations
```

Reject symlinks and non-directories exactly as existing Vault product paths do. Reuse capability-relative record helpers; do not use ambient writes after authorization.

- [ ] **Step 4: Implement immutable records**

Persist:

```rust
pub struct ProfileCandidateRecord {
    pub candidate_id: String,
    pub imported_at_unix_ms: u64,
    pub source_kind: ProfileSourceKind,
    pub source_basename: String,
    pub locator_identity: ContentIdentity,
    pub source_identity: ContentIdentity,
    pub profile_id: String,
    pub profile_version: String,
    pub status: CandidateStatus,
    pub base: Option<ProfileVersionRef>,
    pub diff: ProfileDiff,
    pub approval: Option<ProfileDecisionSummary>,
}
```

Source bytes use `.aiks/profiles/sources/<digest>.json`; candidate, decision, installed, and activation records use create-new filenames. Maintain one in-process registry lock around import and decision publication. On approval, rewrite the semantic profile status to `approved`, validate again, write the installed version, then append activation. A failed write leaves no active change; recovery treats installation without activation as inactive.

- [ ] **Step 5: Verify**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::store --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all persistence, Rust, and lint checks pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/profiles src-tauri/src/vault
git commit -m "feat: persist profile candidates and decisions"
```

### Task 4: Native commands and typed client boundary

**Files:**
- Modify: `src-tauri/src/profiles/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/features/profiles/types.ts`
- Create: `src/features/profiles/profileClient.ts`
- Create: `src/features/profiles/profileClient.test.ts`

- [ ] **Step 1: Write failing client tests**

Assert the Tauri client calls:

```ts
["inspect_profile_state"]
["import_local_profile_candidate"]
["decide_profile_candidate", {
  request: {
    candidateId: "candidate-1",
    reviewedDigest: "a".repeat(64),
    decision: "approve",
  },
}]
```

Assert every browser adapter method rejects with `Desktop runtime is required for profile operations.` and never returns fake state.

- [ ] **Step 2: Run and verify RED**

```bash
npm test -- --run src/features/profiles/profileClient.test.ts
```

Expected: import failure because the client does not exist.

- [ ] **Step 3: Add native commands**

Expose:

```rust
inspect_profile_state() -> ProfileStateSummary
import_local_profile_candidate(app, vaults, profiles) -> Option<ProfileCandidateRecord>
decide_profile_candidate(request, vaults, profiles) -> ProfileStateSummary
```

The native import dialog selects one file. Open it no-follow, require a regular file of at most 1 MiB, read bounded bytes, compute SHA-256, parse/validate before any Vault write, and pass only capability-validated data to the store. The decision command accepts only candidate ID, literal reviewed digest, and `approve` or `reject`; actor remains trusted-core `desktop-user`.

- [ ] **Step 4: Implement typed clients**

Mirror the serialized Rust DTOs in TypeScript. The Tauri adapter invokes exactly the three commands. The browser adapter rejects all operations and has no in-memory mutation implementation.

- [ ] **Step 5: Verify**

```bash
npm test -- --run src/features/profiles/profileClient.test.ts
npm run build
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
```

Expected: client tests, TypeScript build, and Rust tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/profiles src-tauri/src/lib.rs src/features/profiles
git commit -m "feat: expose native profile review commands"
```

### Task 5: Compact Import Review UI

**Files:**
- Create: `src/features/profiles/ProfileReview.tsx`
- Create: `src/features/profiles/ProfileReview.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/app/AppShell.tsx`
- Modify: `src/features/workbench/ContextPane.tsx`
- Modify: `src/styles.css`
- Modify: `e2e/source-workbench.spec.ts`

- [ ] **Step 1: Write failing UI tests**

With a mocked client, assert the Import Review tab:

1. shows `Ninebot electronic archive`, `Draft`, version, provenance, and `0 rules — classification disabled`;
2. imports a local candidate and shows source `SHA-256`, added/removed/changed rule IDs, and `Unapproved`;
3. requires a checkbox stating the reviewed digest before enabling Approve or Reject;
4. sends only candidate ID, reviewed digest, and decision;
5. shows `Approved and active` only after the client returns that state;
6. shows desktop-runtime errors in browser mode and never claims activation.

- [ ] **Step 2: Run and verify RED**

```bash
npm test -- --run src/features/profiles/ProfileReview.test.tsx
```

Expected: component import failure.

- [ ] **Step 3: Implement the review panel**

Keep the existing right pane and tabs. Replace the current static Import Review content with `ProfileReview`. Use compact sections for installed profiles, candidate provenance, rule diff, and decision confirmation. Do not render imported JSON as HTML. Do not add a full-workspace wizard.

- [ ] **Step 4: Add browser E2E**

Click Import Review and Import local profile in the browser fixture. Assert the visible desktop-runtime error, draft Ninebot label, no `Approved and active`, and unchanged `0 changes` status.

- [ ] **Step 5: Verify frontend and E2E**

```bash
npm test -- --run
npm run build
npm run e2e
```

Expected: all frontend tests, production build, and Chromium flows pass.

- [ ] **Step 6: Commit**

```bash
git add src e2e
git commit -m "feat: add profile import review panel"
```

### Task 6: Milestone verification and clean-room review

**Files:**
- Modify: `README.md`
- Modify: `THIRD_PARTY_NOTICES.md` only if a new production dependency was added.

- [ ] **Step 1: Document the shipped boundary**

Add a README section stating that local declarative profile import, immutable approval records, exact-version proposals, and the draft Ninebot shell are implemented. State explicitly that full Ninebot taxonomy, URL import, model-generated profile conversion, canonical naming, and archive-path application are not yet claimed.

- [ ] **Step 2: Run the complete verification loop**

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

Expected: all checks pass, audit reports zero high vulnerabilities, and Git diff has no whitespace errors.

- [ ] **Step 3: Run clean-room leakage checks**

```bash
rg -n '/Users/charles|KLOrgFromFile|ai-file-sorter/.+src|github\\.com/hyperfield/ai-file-sorter/.+blob' \
  src src-tauri README.md THIRD_PARTY_NOTICES.md
```

Expected: no private absolute path or contaminated-source pointer. Product-facing historical names may appear only in existing approved provenance text.

- [ ] **Step 4: Commit**

```bash
git add README.md THIRD_PARTY_NOTICES.md
git commit -m "docs: record profile kernel milestone"
```

## Self-review

- **Spec coverage:** RULE-001, RULE-002 provenance fields, local-file portion of RULE-003, RULE-004, RULE-005, SAFE-004 persistent decisions, and SAFE-007 non-mutation are mapped to Tasks 1–5.
- **Explicit gaps:** URL candidate import and its SSRF/redirect/credential bounds require a separate security plan. Canonical naming requires a separate deterministic naming-and-archive plan. Full Ninebot taxonomy requires a clean authorized data handoff and is not invented here.
- **Type consistency:** `profileId`, `profileVersion`, `candidateId`, `reviewedDigest`, `sourceIdentity`, and `classificationReview` are fixed across Rust DTOs, TypeScript, UI tests, and commands.
- **Placeholder scan:** No task uses TBD, TODO, generic “handle errors,” or undefined follow-up code.
