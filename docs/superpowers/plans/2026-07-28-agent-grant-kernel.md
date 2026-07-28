# Governed Agent Grant Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the trusted, persistent Agent identity and permission kernel that future local MCP stdio and loopback Streamable HTTP transports must use, including native directory selection, bounded grants, authenticated sessions, request replay defense, resource limits, revocation, immutable audit records, and desktop management UI.

**Architecture:** Rust is the sole authority. The frontend never supplies filesystem paths: a native multiple-directory picker opens no-follow capability roots and returns one short-lived opaque selection ID; grant creation consumes that selection once and records opaque scope IDs plus display-only paths. High-entropy grant/session tokens are returned only at issuance, while only SHA-256 verifiers are persisted or retained; every Agent request passes through one authorization function that re-checks caller, session, grant, scope, tool, expiry, revocation, request ID, and byte/request limits before yielding a capability lease. This slice intentionally does not start an MCP server or dispatch domain tools; the next transport slice must call this kernel and may expose only its fixed safe tool catalog.

**Tech Stack:** Tauri 2, Rust 2021, `cap-std`, `getrandom`, `serde`, `sha2`, React 19, TypeScript 7, Vitest, Testing Library, Playwright.

---

## Scope and non-negotiable boundaries

- Implements the grant/session/replay/resource portions of MCP-002 and MCP-004 and makes MCP-003 mechanically testable through a code-owned tool catalog.
- Does not claim MCP-001 transport completion. No stdio or HTTP listener is started in this slice.
- The safe catalog is exactly `capabilities.read`, `knowledge.read`, `graph.read`, `comparison.run`, `classification.propose`, and `cleanup.suggest`. There is no cleanup execution, archive commit, move, rename, delete, or arbitrary filesystem tool.
- `cleanup.suggest` is semantic advice only. It cannot call a cleanup implementation because none exists in this slice.
- Directory scopes are selected through the native picker, opened as no-follow capabilities, and referenced by opaque `scopeId`. The frontend cannot invent an absolute path.
- Grants remain visible after relaunch but are `inactive` until the user reselects their exact directories; this slice does not silently reopen filesystem authority from persisted path strings. In-memory grants are immediately active.
- Grant bearer tokens and session bearer tokens are shown/returned only once. Only lowercase SHA-256 token digests are stored. Logs and errors never include tokens.
- Browser preview adapters reject all Agent-access mutations and never fabricate grants, tokens, sessions, or persistence.

## File map

- Create `src-tauri/src/agent_access/schema.rs`: strict request/response types, fixed tool catalog, limits, identifier validation, and token helpers.
- Create `src-tauri/src/agent_access/store.rs`: atomic app-config persistence and immutable per-event audit records.
- Create `src-tauri/src/agent_access/authority.rs`: pending native selections, active capability roots, grant/session state, replay and resource enforcement.
- Create `src-tauri/src/agent_access/mod.rs`: Tauri commands and public kernel facade used by the later transport layer.
- Modify `src-tauri/src/lib.rs`: manage one `AgentAccessAuthority` and register commands.
- Modify `src-tauri/Cargo.toml` and lockfile: add direct `getrandom` dependency.
- Create `src/features/agentAccess/types.ts`: exact camelCase frontend contract.
- Create `src/features/agentAccess/agentAccessClient.ts` and tests: honest Tauri/browser adapters.
- Create `src/features/agentAccess/AgentAccessPanel.tsx` and tests: grant creation, one-time token display, active/inactive state, and revocation.
- Create `src/features/settings/SettingsDialog.tsx` and tests: accessible Model runtime / Agent access tabs.
- Refactor `src/features/models/ModelSettingsDialog.tsx` into an embeddable model panel without changing existing model behavior.
- Modify `src/App.tsx`, `src/app/AppShell.tsx`, `src/styles.css`, `src/App.test.tsx`, and `e2e/source-workbench.spec.ts`: wire the client, render settings, and prove browser honesty.
- Modify `README.md`: document delivered grant kernel and retain transports as explicitly unimplemented.

### Task 1: Strict grant schema and fixed safe capability catalog

**Files:**
- Create: `src-tauri/src/agent_access/schema.rs`
- Create: `src-tauri/src/agent_access/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

- [ ] **Step 1: Write failing Rust schema tests**

Add tests covering:

```rust
#[test]
fn catalog_contains_only_bounded_non_destructive_tools() {
    let ids = tool_catalog().iter().map(|tool| tool.tool_id).collect::<Vec<_>>();
    assert_eq!(ids, vec![
        "capabilities.read",
        "knowledge.read",
        "graph.read",
        "comparison.run",
        "classification.propose",
        "cleanup.suggest",
    ]);
    assert!(ids.iter().all(|id| !id.contains("delete") && !id.contains("execute")));
}

#[test]
fn grant_request_rejects_unknown_fields_tools_ids_and_limits() {
    // Parse with deny_unknown_fields; reject path-like/control-character IDs,
    // duplicate or unknown tool IDs, zero/excessive TTL, zero/excessive limits,
    // duplicate scope IDs, and more than documented collection bounds.
}
```

Use these exact bounds: 128-byte IDs, 256-character labels, 16 scopes, 16 tools, TTL 60 seconds through 30 days, 1 through 100,000 requests, 1 KiB through 1 MiB request bodies, and 1 KiB through 4 MiB response budgets. All externally received structs use `#[serde(rename_all = "camelCase", deny_unknown_fields)]`.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml agent_access::schema --lib`

Expected: FAIL because `agent_access` does not exist.

- [ ] **Step 3: Implement the minimal schema and catalog**

Define:

```rust
pub const TOOL_CATALOG_VERSION: &str = "agent-tools-v1";

pub struct AgentToolDescriptor {
    pub tool_id: &'static str,
    pub title: &'static str,
    pub effect: AgentToolEffect, // Read | SemanticAdvice
}

pub struct AgentResourceLimits {
    pub max_requests_per_session: u32,
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
}

pub struct CreateAgentGrantRequest {
    pub selection_id: String,
    pub agent_id: String,
    pub label: String,
    pub tool_ids: Vec<String>,
    pub expires_in_seconds: u64,
    pub limits: AgentResourceLimits,
}
```

Add `getrandom = "0.3"`. Generate 32 random bytes, encode them as lowercase hex, and store/compare only SHA-256 token digests with a constant-time byte comparison. Token helpers must never implement `Debug` or `Serialize` for plaintext token wrappers.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml agent_access::schema --lib`

Expected: all schema tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/agent_access src-tauri/src/lib.rs
git commit -m "feat: define bounded agent grant schema"
```

### Task 2: Native capability selection and persistent grant records

**Files:**
- Create: `src-tauri/src/agent_access/authority.rs`
- Create: `src-tauri/src/agent_access/store.rs`
- Modify: `src-tauri/src/agent_access/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing authority/store tests**

Cover all of the following with temporary directories and an injected clock/config root:

```rust
#[test]
fn consumes_one_native_selection_into_one_active_grant() {
    // Selection owns opened cap_std::fs::Dir values. Grant output contains
    // opaque scope IDs and display paths, plus one plaintext grant token.
    // Persisted JSON contains grantTokenSha256 but never the plaintext token.
}

#[test]
fn rejects_reused_expired_symlinked_and_out_of_bound_selections() {}

#[test]
fn relaunch_inspection_preserves_metadata_but_marks_grants_inactive() {}

#[test]
fn revoke_is_idempotent_and_prevents_new_sessions() {}

#[test]
fn config_and_audit_paths_reject_links_and_non_regular_entries() {}
```

Also assert at most 32 grants, 16 pending selections, a five-minute selection TTL, exact atomic replacement of `agent-access-v1.json`, and immutable audit paths `agent-access-audit/<20-digit-sequence>-<event-id>.json`.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml agent_access::authority agent_access::store --lib`

Expected: FAIL because persistence and authority behavior are missing.

- [ ] **Step 3: Implement selection, grant persistence, and audit**

`AgentAccessAuthority` owns:

```rust
struct State {
    pending_selections: HashMap<String, PendingSelection>,
    active_grants: HashMap<String, ActiveGrant>,
    sessions: HashMap<String, AgentSession>,
}

struct PendingSelection {
    expires_at: SystemTime,
    roots: Vec<SelectedRoot>, // display path + open cap_std::fs::Dir
}

struct ActiveGrant {
    record: AgentGrantRecord,
    roots: HashMap<String, SelectedRoot>,
}
```

Open picker results through the existing no-follow `open_trusted_drop_root`; accept directories only, deduplicate by the resolved display path, and keep the open handles. Assign scope IDs independently from paths. Persist grant metadata and token digest, never capability handles or plaintext tokens. Every state change writes an immutable audit event before exposing success; if persistence or audit fails, leave the previous in-memory state unchanged.

Public summaries expose `active`, `inactive`, `revoked`, or `expired`. Inspection prunes expired pending selections/sessions and derives time status without rewriting history. Revocation records `revokedAtUnixMs`, clears active roots and sessions, and is idempotent for the exact grant.

- [ ] **Step 4: Add Tauri directory-picker and grant commands**

Register only:

```rust
select_agent_grant_directories(app, authority)
inspect_agent_access(app, authority)
create_agent_grant(app, authority, request)
revoke_agent_grant(app, authority, request)
```

The picker command calls `blocking_pick_folders()`. `create_agent_grant` accepts the opaque selection ID but no path field. Resolve `app.path().app_config_dir()` in the command boundary and pass it to the store.

- [ ] **Step 5: Run focused and full Rust tests**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml agent_access --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Expected: all tests pass and existing 89 tests remain green.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/agent_access src-tauri/src/lib.rs
git commit -m "feat: persist native agent grants"
```

### Task 3: Authenticated sessions, replay defense, and resource authorization

**Files:**
- Modify: `src-tauri/src/agent_access/authority.rs`
- Modify: `src-tauri/src/agent_access/mod.rs`
- Modify: `src-tauri/src/agent_access/store.rs`

- [ ] **Step 1: Write failing authorization tests**

Create one active grant and prove:

```rust
#[test]
fn opens_only_an_exact_authenticated_agent_session() {}

#[test]
fn authorizes_one_fresh_bounded_request_and_yields_scope_capability() {}

#[test]
fn denies_spoofed_agent_session_token_tool_scope_and_replayed_request() {}

#[test]
fn denies_expired_revoked_and_resource_exhausted_requests() {}

#[test]
fn failed_authorization_never_consumes_or_mutates_a_filesystem_capability() {}
```

An authorized request must identify `agentId`, `grantId`, `sessionId`, `sessionToken`, `requestId`, `toolId`, optional `scopeId`, `requestBytes`, and `responseBudgetBytes`. Request IDs are 1–128 safe ASCII bytes and are single-use within a session even when downstream dispatch later fails.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml agent_access::authority::tests --lib`

Expected: new authorization tests fail.

- [ ] **Step 3: Implement one authorization choke point**

Expose Rust-only methods (not Tauri commands):

```rust
pub(crate) fn open_session(&self, request: OpenSessionRequest) -> Result<IssuedSession, Denial>;
pub(crate) fn authorize_request(&self, request: AuthorizeRequest) -> Result<AuthorizedRequest, Denial>;
pub(crate) fn close_session(&self, session_id: &str);
```

Session tokens are 32 random bytes returned once, with only their digest retained. Sessions expire at the grant expiry or after 30 minutes, whichever is earlier. Authorization is serialized under the authority lock, validates every dimension before cloning a requested `Dir`, inserts the request ID before returning, increments the request count, and emits a bounded audit event without token material. `AuthorizedRequest` contains the tool descriptor, optional cloned directory capability, and the approved response budget; it contains no ambient path authority.

Use stable denial codes: `invalidRequest`, `unauthenticated`, `unknownGrant`, `inactiveGrant`, `revokedGrant`, `expiredGrant`, `unknownSession`, `sessionMismatch`, `replayedRequest`, `toolDenied`, `scopeDenied`, `requestLimitExceeded`, `requestTooLarge`, `responseBudgetExceeded`, and `authorityUnavailable`.

- [ ] **Step 4: Run focused and full Rust tests**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml agent_access --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: all pass with no warnings.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agent_access
git commit -m "feat: authorize replay-safe agent sessions"
```

### Task 4: Honest typed frontend boundary

**Files:**
- Create: `src/features/agentAccess/types.ts`
- Create: `src/features/agentAccess/agentAccessClient.ts`
- Create: `src/features/agentAccess/agentAccessClient.test.ts`

- [ ] **Step 1: Write failing client contract tests**

Assert Tauri adapters invoke exactly `select_agent_grant_directories`, `inspect_agent_access`, `create_agent_grant`, and `revoke_agent_grant`; mutation calls carry one `{ request }` object and selection/inspection have no invented parameters. Assert the browser adapter rejects every method with `Desktop runtime is required for Agent access operations.` and never returns a fake selection, grant, token, or audit state.

- [ ] **Step 2: Run and verify RED**

Run: `npm test -- --run src/features/agentAccess/agentAccessClient.test.ts`

Expected: FAIL because the files do not exist.

- [ ] **Step 3: Implement exact TypeScript DTOs and adapters**

Mirror the Rust camelCase contract, including `AgentToolDescriptor`, `AgentScopeSummary`, `AgentGrantSummary`, `AgentAccessState`, `NativeScopeSelection`, `CreateAgentGrantRequest`, `IssuedAgentGrant`, and `RevokeAgentGrantRequest`. Plaintext `grantToken` appears only on `IssuedAgentGrant`, never on persistent summaries.

- [ ] **Step 4: Run and verify GREEN**

Run: `npm test -- --run src/features/agentAccess/agentAccessClient.test.ts`

Expected: all client tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/features/agentAccess
git commit -m "feat: add honest agent access client"
```

### Task 5: Agent Access settings panel

**Files:**
- Create: `src/features/agentAccess/AgentAccessPanel.tsx`
- Create: `src/features/agentAccess/AgentAccessPanel.test.tsx`
- Create: `src/features/settings/SettingsDialog.tsx`
- Create: `src/features/settings/SettingsDialog.test.tsx`
- Modify: `src/features/models/ModelSettingsDialog.tsx`
- Modify: `src/App.tsx`
- Modify: `src/app/AppShell.tsx`
- Modify: `src/styles.css`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Write failing accessible UI tests**

Prove:

1. Settings opens on `Model runtime` and preserves existing model configuration behavior.
2. `Agent access` shows the fixed six-tool catalog with effect labels and never offers cleanup execution, delete, move, rename, archive commit, or arbitrary command tools.
3. `Choose directories` obtains an opaque selection and displays its read-only directory labels.
4. Submitting sends only selection ID, identity/label, selected tool IDs, TTL, and resource limits—no path strings.
5. The issued bearer token appears once with a warning, disappears when dismissed, and is absent after reinspection.
6. Active/inactive/expired/revoked state is visible; revocation calls the exact grant ID.
7. Escape closes Settings and restores focus to the gear button.
8. Browser mode displays the desktop-runtime error and never claims a grant was issued.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
npm test -- --run src/features/agentAccess/AgentAccessPanel.test.tsx src/features/settings/SettingsDialog.test.tsx src/App.test.tsx
```

Expected: FAIL because Agent access UI is absent.

- [ ] **Step 3: Refactor model settings into a tab-safe panel**

Keep all existing labels, validation, state behavior, and tests. Move only the dialog shell/close handling to `SettingsDialog`; the model form/list becomes a child panel. Do not add provider discovery, key entry, or other model features.

- [ ] **Step 4: Implement Agent Access panel and workbench wiring**

Use immutable state updates. Default to a one-hour grant, 1,000 requests, 128 KiB request limit, and 256 KiB response budget. Require at least one native scope and one tool. The one-time token region must use `role="status"`, explain that it cannot be recovered, and offer only a dismiss action; do not persist it in localStorage/sessionStorage.

- [ ] **Step 5: Run focused and full frontend tests**

Run:

```bash
npm test -- --run src/features/agentAccess/AgentAccessPanel.test.tsx src/features/settings/SettingsDialog.test.tsx src/App.test.tsx
npm test -- --run
npm run build
```

Expected: all tests pass and production build succeeds.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -m "feat: manage agent grants in settings"
```

### Task 6: Browser E2E, documentation, and release gate

**Files:**
- Modify: `e2e/source-workbench.spec.ts`
- Modify: `README.md`
- Modify: this plan

- [ ] **Step 1: Add failing browser E2E assertions**

Open Settings, switch to Agent access, attempt directory selection, and assert the desktop-runtime error is visible. Assert there is no issued token, active grant, MCP connected claim, cleanup execution, move, rename, delete, or archive action. Preserve the existing model-settings browser checks.

- [ ] **Step 2: Run E2E and verify RED, then complete UI/doc wording**

Run: `npx playwright test --project=chromium`

Expected before final wiring: new assertions fail. Update only the necessary UI behavior and README wording, then rerun until all Chromium scenarios pass.

README must claim native capability selection, persistent secret-digest-only grant metadata, active/inactive lifecycle, fixed safe tool catalog, authenticated in-memory sessions, request replay defense, resource limits, revocation, and immutable audit events. It must explicitly state that stdio/Streamable HTTP transports, MCP JSON-RPC dispatch, automatic grant reactivation, external Agent runtime smoke tests, cleanup execution, keychain integration, GraphRAG, and 3D remain unimplemented.

- [ ] **Step 3: Run the full release gate**

```bash
npm test -- --run
npm run build
npm audit --audit-level=high
npx playwright test --project=chromium
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --manifest-path src-tauri/Cargo.toml --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
git diff --check
git status --short
```

Also search changed files for high-entropy literals, bearer/token values, direct deletion/cleanup execution tools, unsafe path acceptance, listener startup, and ambient filesystem opening outside the native selection boundary. The search must find no secret values and no transport claim.

- [ ] **Step 4: Mark the plan complete and commit**

```bash
git add README.md e2e/source-workbench.spec.ts docs/superpowers/plans/2026-07-28-agent-grant-kernel.md
git commit -m "docs: record governed agent grant milestone"
```

- [ ] **Step 5: Fast-forward merge after verification**

Confirm `main` and the worktree are clean and non-divergent. Because the user explicitly authorized tested feature branches to be merged unless interrupted, fast-forward `codex/mcp-grant-kernel` into `main`, rerun the full release gate on `main`, then remove only this worktree and delete only this merged branch.

## Self-review

- **Spec coverage:** Tasks 1–3 cover Agent identity, exact directory/tool/time/resource permission dimensions, revocation, authentication, session binding, replay denial, audit, and the absence of Agent cleanup execution. Task 5 makes grant state and user control visible. MCP-001 transport equivalence and loopback/CSRF behavior remain deliberately assigned to the immediately following transport plan and are not claimed here.
- **Trust boundary:** The frontend cannot submit a path, token verifiers never become plaintext output, restarted grants do not silently regain capabilities, and future transports receive no alternate authorization route.
- **Type consistency:** `selectionId`, `grantId`, `agentId`, `scopeId`, `toolId`, `expiresAtUnixMs`, `grantToken`, and resource-limit property names are fixed across Rust, TypeScript, UI tests, and commands.
- **No placeholders:** Every task names concrete files, commands, expected outcomes, limits, and excluded capabilities.
