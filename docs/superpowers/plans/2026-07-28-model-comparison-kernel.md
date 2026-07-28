# Configurable Model Comparison Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add persistent local/OpenAI-compatible model configurations and an evidence-bound two-model comparison whose Agent-side adjudication is visible, accountable, and incapable of directly mutating files.

**Architecture:** Rust owns model configuration validation, exact Vault evidence extraction, the identical semantic request envelope, independent provider execution, Agent-side adjudication validation, and immutable comparison records. Provider secrets are never accepted by the UI or written to configuration; authenticated remote configurations derive a dedicated `AIKS_MODEL_API_KEY_<CONFIG_ID>` environment-variable reference and resolve its value only inside the Rust request boundary. React adds an Obsidian-style Settings surface and a separate Agent Review tab in the right context pane; browser adapters fail visibly instead of simulating configuration, calls, or decisions.

**Tech Stack:** Tauri 2, Rust/Serde/SHA-256/UUID, `reqwest` blocking client with rustls, React 19/TypeScript, Vitest/Testing Library, Playwright.

---

## Scope and invariants

This slice implements AGENT-001, AGENT-002, and the no-mutation portions of AGENT-003/AGENT-004 for evidence-backed graph-relation suggestions. It does not apply model output to the graph, classify or rename a file, expose MCP, persist API-key values, add an Agent-only cleanup tool, or authorize any filesystem-changing operation. A later graph-import action must call the existing trusted graph proposal boundary and create `review` relations; a later MCP plan must bind Agent identity and grants before exposing comparison or adjudication tools.

The two proposal calls receive byte-identical `ComparisonEnvelope` JSON. Provider config IDs, endpoints, model names, credentials, and side labels remain transport metadata outside that envelope. Only after both independent proposal calls finish may the Agent-side provider receive an adjudication request containing the immutable envelope and both recorded proposals. Missing, invalid, inconsistent, oversized, failed, or timed-out output produces `review` or `failed`; it never calls archive, naming, graph, knowledge-save, cleanup, or other mutation commands.

## File map

- Modify `src-tauri/Cargo.toml`: add bounded OpenAI-compatible HTTP client dependencies.
- Create `src-tauri/src/model_runtime/config.rs`: strict config schema, app-config persistence, endpoint policy, derived credential reference, and tests.
- Create `src-tauri/src/model_runtime/protocol.rs`: identical evidence envelope plus strict proposal/adjudication schemas and validation.
- Create `src-tauri/src/model_runtime/openai_compatible.rs`: redirect-free, proxy-free, timed, response-capped chat-completions transport.
- Create `src-tauri/src/model_runtime/store.rs`: authoritative evidence extraction and immutable Vault comparison records.
- Create `src-tauri/src/model_runtime/mod.rs`: Tauri commands, write serialization, independent proposal execution, and Agent adjudication orchestration.
- Modify `src-tauri/src/vault/mod.rs` and `src-tauri/src/lib.rs`: initialize comparison directories, manage runtime authority, and register commands.
- Create `src/features/models/types.ts`, `modelRuntimeClient.ts`, and `modelRuntimeClient.test.ts`: typed native boundary and honest browser adapter.
- Create `src/features/models/ModelSettingsDialog.tsx` and `.test.tsx`: model configuration CRUD without secret-value inputs.
- Create `src/features/models/AgentReviewPane.tsx` and `.test.tsx`: comparison controls and read-only proposal/adjudication inspection.
- Modify `src/features/sources/ToolRail.tsx`, `src/app/AppShell.tsx`, `src/App.tsx`, `src/features/workbench/ContextPane.tsx`, their tests, and `src/styles.css`: open Settings and add the separate Agent Review tab.
- Modify `e2e/source-workbench.spec.ts`, `README.md`, and this plan: browser honesty and exact delivery claims.

### Task 1: Persist strict model configurations without secrets

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/model_runtime/config.rs`
- Create: `src-tauri/src/model_runtime/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] **Step 1: Write failing configuration tests**

Define tests around a generated temporary app-config directory. A valid local record is:

```rust
ModelConfigInput {
    config_id: "local-ollama".to_owned(),
    label: "Local Ollama".to_owned(),
    location: ModelLocation::Local,
    endpoint_url: "http://127.0.0.1:11434/v1/chat/completions".to_owned(),
    model: "qwen3:8b".to_owned(),
    timeout_ms: 30_000,
    authenticated: false,
}
```

Assert upsert/read/remove round-trips, writes schema version 1 atomically, returns no secret value, and derives no credential reference for unauthenticated local endpoints. A valid authenticated remote record derives `AIKS_MODEL_API_KEY_REMOTE_REASONER` from `remote-reasoner`. Reject unknown JSON fields, duplicate IDs, invalid/control-character text, more than 32 configs, embedded URL credentials, query/fragment values, non-loopback local hosts, non-HTTP local schemes, non-HTTPS remote endpoints, localhost remote endpoints, timeouts outside 1–120 seconds, symlinked config files, and replacement conflicts without changing the prior config file.

- [x] **Step 2: Run RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test model_runtime::config --lib`

Expected: FAIL because `model_runtime` does not exist.

- [x] **Step 3: Add the strict schema and app-config store**

Add the URL parser used by the configuration trust boundary:

```toml
url = "2"
```

Implement these public serialized types:

```rust
pub enum ModelLocation { Local, Remote }

pub struct ModelConfigInput {
    pub config_id: String,
    pub label: String,
    pub location: ModelLocation,
    pub endpoint_url: String,
    pub model: String,
    pub timeout_ms: u64,
    pub authenticated: bool,
}

pub struct ModelConfigSummary {
    pub config_id: String,
    pub label: String,
    pub location: ModelLocation,
    pub endpoint_url: String,
    pub model: String,
    pub timeout_ms: u64,
    pub authenticated: bool,
    pub credential_environment: Option<String>,
}

pub struct ModelRuntimeState {
    pub configs: Vec<ModelConfigSummary>,
}
```

Use `serde(rename_all = "camelCase", deny_unknown_fields)`. Resolve `app.path().app_config_dir()` in Tauri commands and store `model-runtime-v1.json` beneath that directory. Use a same-directory unique temporary file, `sync_all`, atomic rename, and directory synchronization; reject links and non-regular records. Restrict config IDs to lowercase ASCII alphanumeric plus `-`, labels/models to 256 visible characters, endpoint URLs to 2 KiB, and config count to 32. Local URLs must be literal loopback HTTP; remote URLs must be HTTPS and must not use loopback or private literal IPs. Disallow usernames, passwords, query strings, and fragments. Derive the credential environment name from the validated ID; never accept it from the frontend.

- [x] **Step 4: Add inspect/upsert/remove commands**

Expose:

```text
inspect_model_runtime()
upsert_model_config(request: ModelConfigInput)
remove_model_config(request: { configId })
```

Manage one `ModelRuntimeAuthority` mutex in `lib.rs`. Browser code is not part of this task.

- [x] **Step 5: Run GREEN and commit**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test model_runtime::config --lib`

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings`

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/model_runtime src-tauri/src/lib.rs
git commit -m "feat: persist secret-free model configurations"
```

### Task 2: Build one exact identical-evidence comparison envelope

**Files:**
- Create: `src-tauri/src/model_runtime/protocol.rs`
- Create: `src-tauri/src/model_runtime/store.rs`
- Modify: `src-tauri/src/model_runtime/mod.rs`
- Modify: `src-tauri/src/vault/mod.rs`

- [x] **Step 1: Write failing envelope and record tests**

Create a verified archive fixture and two committed Markdown revisions. Request revision 1 with lines 2–3 and assert Rust reopens revision 1, independently re-verifies the archived original, extracts exact text, and constructs:

```rust
ComparisonEnvelope {
    schema_version: 1,
    task: ComparisonTask::KnowledgeRelations,
    original_identity,
    markdown_identity,
    knowledge_revision: 1,
    rule_snapshot: RuleSnapshot {
        policy_id: "knowledge-relations-v1".to_owned(),
        version: "1.0.0".to_owned(),
        identity: ContentIdentity { algorithm: "SHA-256", digest },
        json: KNOWLEDGE_RELATION_RULE_JSON.to_owned(),
    },
    evidence: vec![EvidenceExcerpt {
        evidence_id: "line-2-3".to_owned(),
        start_line: 2,
        end_line: 3,
        text: "...".to_owned(),
    }],
}
```

Assert deterministic JSON bytes and SHA-256 identity across two calls. Reject revision 0, missing/tampered revisions, changed originals, zero/reversed/out-of-range/duplicate ranges, empty evidence, more than 16 ranges, and an envelope above 128 KiB without creating `.aiks/comparisons` records.

- [x] **Step 2: Run RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test model_runtime::store --lib`

Expected: FAIL because the envelope/store functions do not exist.

- [x] **Step 3: Implement strict protocol types**

Add `ComparisonEnvelope`, `RuleSnapshot`, `EvidenceExcerpt`, `RelationSuggestion`, `ModelProposal`, `AgentAdjudication`, `ComparisonStatus`, `ProviderOutcome`, and `ComparisonRecord`, all with camelCase serialization. `RelationSuggestion` contains only bounded source/type/target strings and 1–16 evidence IDs. `ModelProposal` contains a bounded summary and 1–64 relations. `AgentAdjudication` contains `accept | revise | reject | review`, a non-empty reason, 1–16 evidence IDs, an optional selected side, and revised relations only for `revise`. Every evidence ID must exist in the envelope.

- [x] **Step 4: Implement immutable comparison storage**

Initialize `.aiks/comparisons` in every Vault. Store one immutable record at:

```text
.aiks/comparisons/<comparison-id>/00000001.json
```

Records include the envelope and its SHA-256, the distinct desktop/Agent config IDs, both provider outcomes, optional adjudication, status, actor `desktop-orchestrator`, and timestamps. Use a simple UUID relation-style ID, reject links/non-regular entries, and cap inspection to 10,000 comparisons. No method in this module calls archive, naming, graph, knowledge save, cleanup, or deletion.

- [x] **Step 5: Run GREEN and commit**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test model_runtime::store --lib`

```bash
git add src-tauri/src/model_runtime src-tauri/src/vault/mod.rs
git commit -m "feat: bind model comparisons to exact evidence"
```

### Task 3: Execute bounded OpenAI-compatible proposal and Agent calls

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/model_runtime/openai_compatible.rs`
- Modify: `src-tauri/src/model_runtime/mod.rs`

- [x] **Step 1: Write failing transport and orchestration tests**

Define an internal `ModelTransport` trait that receives a `ModelConfigSummary`, a request kind, and serialized body. A capture transport must prove the desktop and Agent proposal calls receive identical envelope bytes and begin independently before either result is consumed. Test distinct configs are required. Return two distinguishable strict proposals, then assert the Agent config alone receives the adjudication request containing the exact envelope plus both proposals.

Add failure cases for one timeout, non-2xx response, response over 256 KiB, malformed JSON, Markdown-fenced JSON, unknown fields, missing choices/content, invalid evidence IDs, missing adjudication reason/evidence, and materially conflicting proposals followed by an Agent `review`. Every failure must persist a visible `review` or `failed` record and leave fixture source/archive/Markdown/graph bytes unchanged.

- [x] **Step 2: Run RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test model_runtime::openai_compatible model_runtime::tests --lib`

Expected: FAIL because transport and orchestration are absent.

- [x] **Step 3: Add the bounded HTTP transport**

Add:

```toml
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
```

Build `reqwest::blocking::Client` with total/connect/read timeouts from the validated config, `redirect(reqwest::redirect::Policy::none())`, `no_proxy()`, no cookies, and a fixed `AIKnowledgeSort/0.1` user agent. POST JSON to the exact validated endpoint. For authenticated remote configs, read only the derived environment variable and add `bearer_auth`; never include the value in errors, records, logs, or returned state. Require a success status, reject declared bodies above 256 KiB, then read through `Read::take(256 * 1024 + 1)` and reject overflow before parsing.

The proposal request contains one fixed system instruction and one user message containing the exact envelope JSON. The adjudication request uses a different fixed system instruction and one user message containing the exact envelope plus both already-recorded proposals. Set temperature 0 and request JSON output, but still strictly validate returned content rather than trusting provider mode flags.

- [x] **Step 4: Add the comparison command**

Expose:

```text
run_model_comparison({
  authorityId,
  operationId,
  knowledgeRevision,
  evidenceRanges,
  desktopConfigId,
  agentConfigId
})
```

The frontend cannot supply evidence text, identities, rule JSON, provider outcomes, Agent decision, actor, timestamps, status, or comparison ID. The command leases the exact Vault, loads both stored configs, builds one envelope, runs the two proposal calls on separate blocking workers, validates them independently, invokes Agent adjudication only when both proposals are valid, writes the immutable record, and returns it.

- [x] **Step 5: Run GREEN and commit**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test model_runtime --lib`

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo fmt --all -- --check`

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings`

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/model_runtime src-tauri/src/lib.rs
git commit -m "feat: compare independent model proposals"
```

### Task 4: Add typed frontend boundaries and Settings

**Files:**
- Create: `src/features/models/types.ts`
- Create: `src/features/models/modelRuntimeClient.ts`
- Create: `src/features/models/modelRuntimeClient.test.ts`
- Create: `src/features/models/ModelSettingsDialog.tsx`
- Create: `src/features/models/ModelSettingsDialog.test.tsx`
- Modify: `src/features/sources/ToolRail.tsx`
- Modify: `src/app/AppShell.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

- [x] **Step 1: Write failing client tests**

Assert Tauri adapters invoke only `inspect_model_runtime`, `upsert_model_config`, `remove_model_config`, and `run_model_comparison`, each with a single `{ request }` object except no-argument inspect. Assert the browser adapter rejects every method with `Desktop runtime is required for model runtime operations.` and never returns fake configs or comparisons.

- [x] **Step 2: Write failing Settings tests**

Open Settings from the lower-left tool rail. Assert the dialog lists configs, adds/edits/removes one exact config, distinguishes Local and Remote, shows the derived credential environment reference for authenticated remote configs, contains no password/API-key textbox, preserves entered values on native failure, closes with Escape, and restores focus to the Settings button.

- [x] **Step 3: Implement the typed client and Settings dialog**

Mirror Rust camelCase types exactly. Keep one `ModelRuntimeClient` injected from `App`. `AppShell` owns `settingsOpen`; `ToolRail` receives `onOpenSettings` and enables only the existing lower-left Settings button. Render an accessible modal dialog over the workbench with dense Obsidian-style rows. Submit only config input fields, never a credential value. Display: `Set the credential in AIKS_MODEL_API_KEY_<CONFIG_ID>` without reading it.

- [x] **Step 4: Run GREEN and commit**

Run: `npm test -- --run src/features/models/modelRuntimeClient.test.ts src/features/models/ModelSettingsDialog.test.tsx src/App.test.tsx`

Run: `npm run build`

```bash
git add src/App.tsx src/app/AppShell.tsx src/features/models src/features/sources/ToolRail.tsx src/styles.css
git commit -m "feat: configure model runtimes in settings"
```

### Task 5: Add the right-pane Agent Review workflow

**Files:**
- Create: `src/features/models/AgentReviewPane.tsx`
- Create: `src/features/models/AgentReviewPane.test.tsx`
- Modify: `src/features/workbench/ContextPane.tsx`
- Modify: `src/features/workbench/ContextPane.test.tsx`
- Modify: `src/app/AppShell.tsx`
- Modify: `src/styles.css`

- [x] **Step 1: Write failing Agent Review tests**

With no authoritative saved document, assert Agent Review explains that a saved Vault revision is required and exposes no run action. With a saved document and two configs, select distinct desktop/Agent configs plus line ranges and assert the exact request fields. Render both provider outcomes with side/config/model labels, the shared envelope SHA-256, Agent decision, reason, evidence excerpts, failures, and `review` state. Assert no buttons named apply, move, rename, delete, cleanup, accept filesystem operation, or write graph exist.

- [x] **Step 2: Implement the separate Agent Review tab**

Add `Agent Review` beside `Knowledge Graph` and `Import Review`. Load model state when the tab opens. The form contains desktop config, Agent config, start/end line, and `Run comparison`. It does not accept evidence text. Results are read-only cards; the Agent adjudication card is visually distinct and states `Semantic advice · no operation authorized`. Preserve the current result and form if refresh or execution fails.

- [x] **Step 3: Run GREEN and commit**

Run: `npm test -- --run src/features/models/AgentReviewPane.test.tsx src/features/workbench/ContextPane.test.tsx src/App.test.tsx`

```bash
git add src/app/AppShell.tsx src/features/models src/features/workbench/ContextPane.tsx src/features/workbench/ContextPane.test.tsx src/styles.css
git commit -m "feat: review agent model comparisons"
```

### Task 6: Verify browser honesty and delivery claims

**Files:**
- Modify: `e2e/source-workbench.spec.ts`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-07-28-model-comparison-kernel.md`

- [x] **Step 1: Extend browser E2E**

Open Settings and assert the desktop-runtime error is visible, no config is persisted, and no API-key input exists. Open Agent Review and assert the unsaved browser draft cannot run comparison, no proposal/adjudication is fabricated, and no mutation action appears.

- [x] **Step 2: Document only delivered scope**

README shall claim secret-free local/OpenAI-compatible config persistence, exact identical evidence envelopes, independent two-provider proposals, Agent-side adjudication, immutable visible outcomes, timeouts/failures with zero mutation, Settings, and Agent Review. It shall explicitly keep secure keychain entry, model discovery, provider-specific APIs, applying graph suggestions, automatic classification/naming, cleanup, MCP stdio/HTTP/grants, GraphRAG, and 3D graph unimplemented.

- [x] **Step 3: Run the complete gate**

```bash
npm test -- --run
npm run build
npm audit --audit-level=high
npm run e2e -- --project=chromium
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo fmt --all -- --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

- [x] **Step 4: Commit and merge after the gate**

```bash
git add README.md e2e/source-workbench.spec.ts docs/superpowers/plans/2026-07-28-model-comparison-kernel.md
git commit -m "docs: record model comparison milestone"
git -C /Users/charles/Documents/AIKnowledgeSort-MIT merge --ff-only codex/model-comparison-kernel
```

## Self-review

- **Spec coverage:** Task 2 proves identical identity/rules/evidence; Task 3 proves independent proposals, Agent-only adjudication, required reason/evidence, strict failure/review states, and zero mutation; Tasks 4–5 make configuration and outcomes visible. MCP identity/grants/transports and applying suggestions are intentionally separate plans.
- **Safety:** Secrets never cross the frontend boundary or enter persisted JSON. Model calls cannot invoke mutation modules. The final record is semantic advice; critical operations still require existing deterministic validation and user confirmation.
- **Type consistency:** `configId`, `authorityId`, `operationId`, `knowledgeRevision`, `evidenceRanges`, `desktopConfigId`, `agentConfigId`, `envelopeIdentity`, and `recordedAtUnixMs` use identical camelCase names through Rust serialization and TypeScript.
- **Operational honesty:** Browser clients reject. A missing environment credential, timeout, malformed response, or disagreement is visible and non-mutating. Settings never claims secure keychain storage.
