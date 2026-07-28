# Ninebot Classification Draft Profile Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the manifest-only Ninebot shell with a complete, auditable `0.3.0-draft` configuration profile containing the authorized four-level taxonomy and its archive/knowledge conflict policy, while keeping the discussion draft inactive and preventing dictionary terms from becoming unsafe deterministic rules.

**Architecture:** Rust remains the trusted profile authority. Schema version 2 adds bounded taxonomy nodes and declarative governance policy while preserving schema version 1 candidate compatibility. A generated JSON resource contains the normalized 14/94/179/179 tree; the existing executable rule list remains separate and empty until formal semantic rules are reviewed. Import Review displays taxonomy coverage and policy without offering automatic activation of the bundled draft.

**Tech Stack:** Rust 2024, serde/serde_json, SHA-256 identity, cap-std Vault capabilities, Tauri 2, React 19, TypeScript 7, Vitest, Playwright, dependency-free Node.js source compiler.

---

## Scope and source boundary

The authorized inputs are the owner-supplied Ninebot discussion HTML, the
`nimble-kb-org-0.9.0-rc.3` package, its usage manual, and the cross-domain
conflict analysis supplied on 2026-07-29. The normalized taxonomy is derived
from `references/classification_tree.md`; the other documents govern status,
provenance, conflict handling, and terminology.

This plan does not make the draft effective company policy and does not
implement an unreviewed keyword classifier. Dictionary terms narrow model
candidates only; formal placement still requires semantic evidence and
desktop/Agent review. The manual's `SN-02 IPMS 管理营销闭环` wording is retained as
an alias, while the canonical tree and HTML wording remains
`SN-02 IPMS 集成营销服`.

### Task 1: Record exact clean-room source provenance

**Files:**
- Modify: `docs/AUTHORIZATIONS.md`
- Modify: `docs/REQUIREMENT_SOURCES.csv`
- Create: `docs/classification/ninebot-draft-sources.json`
- Test: repository digest and leakage checks

- [x] **Step 1: Write the source manifest**

Record the four supplied artifacts with their exact SHA-256 digests, role,
owner authorization, and draft/non-effective status. Record the ZIP-contained
classification tree, rules, dictionary, and electronic-archive draft as
derived source members without copying unrelated Skill content.

- [x] **Step 2: Extend authorization provenance**

Add `AUTH-2026-07-29-NINEBOT-DRAFT`, limited to owned classification and
knowledge-organization material. Explicitly exclude embedded third-party
content and state that authorization to reuse under MIT is not corporate
approval of the draft.

- [x] **Step 3: Verify source identity and record integrity**

Run:

```bash
shasum -a 256 <four supplied artifact paths>
jq empty docs/classification/ninebot-draft-sources.json
git diff --check
```

Expected: all four hashes match the manifest, JSON parses, and the diff has no
whitespace errors.

- [x] **Step 4: Commit**

```bash
git add docs/AUTHORIZATIONS.md docs/REQUIREMENT_SOURCES.csv docs/classification
git commit -m "docs: record Ninebot draft classification sources"
```

### Task 2: Add a bounded schema-v2 taxonomy and governance policy

**Files:**
- Modify: `src-tauri/src/profiles/schema.rs`
- Test: inline unit tests in `src-tauri/src/profiles/schema.rs`

- [x] **Step 1: Write failing schema-v2 tests**

Add tests proving that:

1. a schema-v2 candidate accepts a valid parent-linked taxonomy and policy;
2. schema-v1 candidates still parse unchanged;
3. duplicate category IDs, missing parents, depth/path mismatches, unsafe path
   segments, more than four levels, invalid aliases, and missing schema-v2
   policy are rejected;
4. unknown or executable-shaped fields remain rejected;
5. draft status is still rejected at candidate-import ingress.

- [x] **Step 2: Run and verify RED**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::schema --lib
```

Expected: new tests fail because taxonomy and policy fields are absent.

- [x] **Step 3: Implement minimal versioned types**

Add bounded `ClassificationCategory` and `ProfileGovernance` data. Each
category carries an opaque category ID, label, depth, optional parent ID,
canonical path, and optional aliases. Governance declares:

- maximum classification depth;
- one unique primary archive category;
- semantic evidence requirement;
- metadata-only dimensions;
- insufficient-evidence and conflicting-evidence review dispositions;
- archive-first processing;
- cross-domain knowledge links;
- independent-node triggers;
- link-only generated indexes.

Keep every field declarative and `deny_unknown_fields`. Accept schema versions
1 and 2; require empty taxonomy/policy for v1 and complete taxonomy/policy for
v2. Preserve the 1 MiB document limit.

- [x] **Step 4: Verify**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::schema --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
```

Expected: schema tests pass and Clippy is clean.

- [x] **Step 5: Commit**

```bash
git add src-tauri/src/profiles/schema.rs
git commit -m "feat: add governed taxonomy profile schema"
```

### Task 3: Compile and validate the complete Ninebot draft profile

**Files:**
- Create: `scripts/compile-ninebot-classification.mjs`
- Create: `src-tauri/resources/profiles/ninebot-electronic-archive-0.3.0-draft.json`
- Create: `src-tauri/src/profiles/ninebot.rs`
- Modify: `src-tauri/src/profiles/mod.rs`
- Test: inline unit tests in `src-tauri/src/profiles/ninebot.rs`

- [ ] **Step 1: Write failing bundled-profile tests**

Assert that the bundled resource:

- parses as schema version 2 and status `draft`;
- contains exactly 14 L1, 94 L2, 179 L3, and 179 L4 nodes;
- contains 466 unique category IDs and a maximum depth of four;
- uses canonical `SN-02 IPMS 集成营销服` and retains
  `SN-02 IPMS 管理营销闭环` as an alias;
- declares the approved conflict and knowledge policy;
- contains zero executable rules and therefore cannot classify or commit.

- [ ] **Step 2: Run and verify RED**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::ninebot --lib
```

Expected: compilation fails because the bundled module/resource is absent.

- [ ] **Step 3: Implement the deterministic compiler**

Parse only the authorized Markdown tree headings and indentation. Reject
unexpected syntax, wrong level counts, missing parents, duplicate IDs,
non-four-level output, or an unexpected canonical SN-02 label. Emit stable,
pretty JSON with source digests and governance values fixed by the reviewed
materials. Do not emit keyword rules from the dictionary.

- [ ] **Step 4: Generate and verify the resource**

```bash
node scripts/compile-ninebot-classification.mjs \
  /private/tmp/nimble-kb-org-rc3.uOFf9A/nimble-kb-org/references/classification_tree.md \
  src-tauri/resources/profiles/ninebot-electronic-archive-0.3.0-draft.json
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::ninebot --lib
```

Expected: deterministic resource generation and all bundled-profile tests pass.

- [ ] **Step 5: Commit**

```bash
git add scripts src-tauri/resources src-tauri/src/profiles
git commit -m "feat: bundle complete Ninebot draft taxonomy"
```

### Task 4: Install and summarize the complete inactive draft

**Files:**
- Modify: `src-tauri/src/profiles/store.rs`
- Test: inline unit tests in `src-tauri/src/profiles/store.rs`

- [ ] **Step 1: Write failing store tests**

Update the existing installed-draft test to require version `0.3.0-draft`,
schema version 2, 466 categories, the four level counts, zero executable rules,
and no active profile. Add a migration assertion that an existing immutable
`0.1.0-draft` installation remains readable while the new bundled version is
added without overwriting prior bytes.

- [ ] **Step 2: Run and verify RED**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::store --lib
```

Expected: installed draft still reports `0.1.0-draft` and lacks taxonomy
summary fields.

- [ ] **Step 3: Install the bundled bytes**

Replace the hand-built zero-rule shell with the validated bundled resource.
Extend `ProfileSummary` with total category count, per-level counts, semantic
evidence requirement, unique-primary-archive policy, and cross-domain knowledge
link policy. Keep activation empty because status is `draft`.

- [ ] **Step 4: Verify migration and authority invariants**

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test profiles::store --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
```

Expected: store and complete Rust suites pass without replacing immutable
records or activating the draft.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/profiles/store.rs
git commit -m "feat: install complete inactive Ninebot draft"
```

### Task 5: Show taxonomy coverage and governance in Import Review

**Files:**
- Modify: `src/features/profiles/types.ts`
- Modify: `src/features/profiles/ProfileReview.tsx`
- Modify: `src/features/profiles/ProfileReview.test.tsx`
- Modify: `src/styles.css`
- Modify: `e2e/source-workbench.spec.ts`

- [ ] **Step 1: Write failing component and E2E assertions**

Require Import Review to display:

- `0.3.0-draft`;
- `466 categories · 14 / 94 / 179 / 179`;
- `0 executable rules — semantic review required`;
- `One primary archive category`;
- `Cross-domain knowledge links`;
- `Discussion draft — not approved or active`.

Keep the candidate digest acknowledgement and approval flow unchanged.

- [ ] **Step 2: Run and verify RED**

```bash
NODE_OPTIONS=--no-experimental-webstorage npm test -- --run src/features/profiles/ProfileReview.test.tsx
```

Expected: assertions fail because the UI still shows only zero-rule shell
metadata.

- [ ] **Step 3: Extend typed rendering**

Add exact readonly summary fields and compact policy rows. Update the browser
fallback to the same honest bundled metadata; it may display the shipped
manifest but must still reject persistence, import, and decisions outside the
desktop runtime.

- [ ] **Step 4: Verify frontend**

```bash
NODE_OPTIONS=--no-experimental-webstorage npm test -- --run
npm run build
npm run e2e
```

Expected: all unit, build, and critical browser flows pass.

- [ ] **Step 5: Commit**

```bash
git add src e2e
git commit -m "feat: show Ninebot taxonomy governance"
```

### Task 6: Document limitations and run release gates

**Files:**
- Modify: `README.md`
- Modify: `docs/IMPLEMENTATION_SPEC.md`
- Modify: `docs/FUNCTIONAL_CONTRACTS.md`
- Modify: this plan's checkboxes

- [ ] **Step 1: Document the delivered boundary**

State that the complete draft taxonomy and conflict policy are bundled, but
semantic model classification and formal-company activation remain pending.
Document that generated indexes are link-only and knowledge nodes may be
cross-domain after the archive receives one primary category.

- [ ] **Step 2: Run formatting, tests, security, and clean-room gates**

```bash
NODE_OPTIONS=--no-experimental-webstorage npm test -- --run
npm run build
npm audit --audit-level=high
npm run e2e
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo fmt --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
git diff --check
rg -n "(sk-[A-Za-z0-9]|Bearer [A-Za-z0-9._-]{20,}|api[_-]?key\\s*[:=])" \
  --glob '!package-lock.json' .
```

Expected: all tests and builds pass, audit reports no high vulnerabilities,
Clippy is warning-free, and the secret scan finds no credential material.

- [ ] **Step 3: Review the complete diff**

Confirm every changed line maps to source provenance, schema-v2 taxonomy,
bundled profile validation, inactive installation, UI summary, tests, or
boundary documentation. Confirm no source file operation or deletion behavior
changed.

- [ ] **Step 4: Commit**

```bash
git add README.md docs
git commit -m "docs: define Ninebot draft profile boundary"
```

- [ ] **Step 5: Fast-forward merge after gates**

Under the owner's standing authorization, verify `main` is an ancestor, then
fast-forward `main` to `codex/ninebot-classification-draft`. Stop instead of
merging if the branch diverged or any gate failed. Do not push or open a PR.
