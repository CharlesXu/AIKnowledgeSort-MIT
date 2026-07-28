# Governed Local MCP Transports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver real local MCP connectivity over loopback-only stateful Streamable HTTP and a standard stdio relay, with both transports sharing the existing desktop `AgentAccessAuthority`, authenticated grants, replay defense, resource bounds, revocation, and audit.

**Architecture:** The Tauri desktop process owns the only filesystem capabilities and starts an embedded HTTP broker bound to a literal loopback address. The application executable also supports a headless `--mcp-stdio-relay` mode that translates newline-delimited stdio MCP into authenticated requests to that broker; it never reads persisted paths or creates a second authority. The official stable `rmcp = 2.2.0` server handles MCP lifecycle, JSON-RPC, stateful sessions, and Streamable HTTP framing, while project-owned middleware and handlers enforce exact Host, Origin, Agent, grant, session, request-size, response-size, and replay boundaries before dispatch.

**Tech Stack:** Rust 2021, Tauri 2, Tokio, official `rmcp` 2.2.0, Axum 0.8, cap-std/cap-fs-ext, React/TypeScript/Vitest, Playwright.

---

## Scope and invariants

- Covers MCP-001 and the transport-facing portions of MCP-002 through MCP-004.
- Supports stable MCP protocol negotiation through the official SDK; no project-owned fork of JSON-RPC or Streamable HTTP framing is introduced.
- The listener binds only to `127.0.0.1`; `0.0.0.0`, unspecified IPv6, LAN, DNS names, and non-loopback addresses are rejected before bind.
- Every HTTP request requires an exact bearer grant token, Agent ID, grant ID, and stateful `Mcp-Session-Id` after initialization. The plaintext grant token is never logged or stored by the desktop.
- Missing-Origin non-browser clients may proceed after authentication. Requests carrying `Origin` must match an explicitly grant-allowed literal loopback HTTP origin and remain bound to that origin for the Agent session. `Origin: null`, wildcards, DNS names including `localhost`, credentials, paths, queries, fragments, default-port ambiguity, and non-loopback origins are rejected.
- The stdio relay reads credentials only from `AIKS_MCP_AGENT_ID`, `AIKS_MCP_GRANT_ID`, and `AIKS_MCP_GRANT_TOKEN`; the token is never accepted as a CLI argument. It forwards to a literal loopback URL supplied by `--broker-url`, disables proxy inheritance and redirects, bounds frames and responses, and writes protocol messages only to stdout.
- Exact replays are keyed from the MCP session identity plus JSON-RPC request ID and pass through the existing grant-wide replay cache. A captured request carrying an old MCP session ID cannot be rebound to a replacement session.
- `capabilities.read`, bounded no-follow `knowledge.read`, bounded no-follow `graph.read`, and non-mutating `cleanup.suggest` are real. `comparison.run` and `classification.propose` remain visible but return an explicit structured `notReady` tool result until their existing desktop workflows receive dedicated MCP adapters; they never fabricate model or classification output.
- No move, rename, delete, archive commit, cleanup execution, arbitrary command, ambient path, or automatic grant-reactivation surface is added.

## File map

- Modify `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`: pin stable official MCP and HTTP dependencies and expand Tokio features.
- Modify `src-tauri/src/agent_access/schema.rs`, `store.rs`, `authority.rs`, and `mod.rs`: explicit allowed HTTP origins and transport-bound session verification.
- Create `src-tauri/src/mcp_transport/auth.rs`: strict header, origin, bind-address, broker-URL, and request-size policy.
- Create `src-tauri/src/mcp_transport/tools.rs`: schemas and bounded read/suggestion dispatch.
- Create `src-tauri/src/mcp_transport/service.rs`: authenticated `rmcp::ServerHandler` and per-request trusted-core authorization.
- Create `src-tauri/src/mcp_transport/http.rs`: loopback listener lifecycle and stateful Streamable HTTP service.
- Create `src-tauri/src/mcp_transport/stdio_relay.rs`: bounded newline-to-HTTP relay.
- Create `src-tauri/src/mcp_transport/mod.rs`: public transport authority, DTOs, and Tauri commands.
- Modify `src-tauri/src/lib.rs` and `src-tauri/src/main.rs`: manage broker state, register commands, and enter headless relay mode before GUI startup.
- Create `src-tauri/tests/mcp_runtime_smoke.rs`: real subprocess stdio plus direct HTTP smoke coverage against an active grant.
- Modify `src/features/agentAccess/types.ts`, `agentAccessClient.ts`, and tests: origin and transport DTOs.
- Modify `src/features/agentAccess/AgentAccessPanel.tsx` and tests plus `src/styles.css`: broker lifecycle, allowed-origin input, and copyable direct HTTP/stdio templates while the one-time token remains in memory.
- Modify `e2e/source-workbench.spec.ts`: browser mode remains honest and does not claim a running MCP server.
- Modify `README.md`: delivered transport boundary, configuration examples, and remaining domain adapters.

### Task 1: Grant-bound HTTP origin and transport identity

**Files:**
- Modify: `src-tauri/src/agent_access/schema.rs`
- Modify: `src-tauri/src/agent_access/store.rs`
- Modify: `src-tauri/src/agent_access/authority.rs`
- Modify: `src/features/agentAccess/types.ts`

- [x] **Step 1: Write failing schema and authority tests**

Add vectors proving an empty origin list is valid; at most eight exact `http://127.0.0.1:<port>` or `http://[::1]:<port>` origins are accepted; duplicates, wildcard, `null`, `localhost`, credentials, path/query/fragment, missing explicit port, non-HTTP, non-loopback, control characters, and unknown JSON fields are rejected. Open one Agent session with an allowed origin and prove every authorization request must carry that identical origin; spoofed, missing, or changed origin is denied. Prove no-Origin stdio/CLI sessions remain allowed but cannot later acquire an Origin.

- [x] **Step 2: Run the focused tests and verify RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib agent_access`

Expected: FAIL because grant requests/records and sessions do not contain origin policy or binding.

- [x] **Step 3: Implement the minimal policy**

Add `allowed_http_origins: Vec<String>` to grant requests, persisted records, summaries, and TypeScript DTOs. Add `transport_origin: Option<String>` to `OpenSessionRequest` and `AuthorizeRequest`; validate with one shared strict parser. Retain the normalized origin in `AgentSession`. Add `verify_transport_credentials(config_root, grant_id, agent_id, grant_token, origin, now)` for subsequent HTTP requests; it must re-read current persisted grant state under the authority lock, verify the token digest and active state, and reject origin drift without returning secrets or capabilities.

- [x] **Step 4: Run focused/full tests and commit**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib agent_access
npm test -- --run src/features/agentAccess/agentAccessClient.test.ts
```

Expected: all focused tests pass and persisted fixtures contain only token digests.

Commit: `feat: bind agent grants to explicit HTTP origins`

### Task 2: Stable MCP handler and safe tool dispatcher

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Create: `src-tauri/src/mcp_transport/auth.rs`
- Create: `src-tauri/src/mcp_transport/tools.rs`
- Create: `src-tauri/src/mcp_transport/service.rs`
- Create: `src-tauri/src/mcp_transport/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] **Step 1: Pin dependencies and write failing handler tests**

Pin `rmcp = "=2.2.0"` with only `server`, `transport-io`, `transport-streamable-http-server`, `transport-streamable-http-server-session`, and `tower`; add Axum 0.8 and the minimum HTTP/body utilities required by the official service. Tests create an active grant and assert:

1. initialize without exact credentials fails;
2. valid initialize opens one Agent session;
3. `tools/list` returns only fixed granted tools and no cleanup-execution or filesystem-mutation tool;
4. the same MCP session/request ID is rejected on replay;
5. changed Agent/grant/token/origin headers are denied per request;
6. revocation and expiry take effect without restarting the MCP session;
7. input/output/request-count limits are enforced.

- [x] **Step 2: Run handler tests and verify RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib mcp_transport::service`

Expected: FAIL because `mcp_transport` does not exist.

- [x] **Step 3: Implement the authenticated ServerHandler**

Override `initialize`, `list_tools`, and `call_tool`. Read HTTP request parts from `RequestContext.extensions`, parse exact headers without logging values, and store only the issued Agent session token inside the per-MCP-session handler. Derive a safe grant-wide replay identity as lowercase SHA-256 over a length-prefixed tuple of MCP session ID and JSON-RPC request ID. Every list/call first re-verifies transport credentials and then calls `AgentAccessAuthority::authorize_request`; there is no alternate tool path.

Return MCP protocol errors only for malformed/unknown methods. Authorization and tool failures use visible `CallToolResult::error` content containing stable denial codes but no paths beyond already granted display metadata and no secret values.

- [x] **Step 4: Implement bounded real tools**

`capabilities.read` returns the exact granted tool IDs, scope IDs/display labels, status, expiry, and limits. `knowledge.read` accepts `{scopeId, relativePath}` with a normalized relative path, `.md` extension, no parent/absolute/prefix component, no symlink at any opened component, regular-file requirement, and an output cap bounded by the grant. `graph.read` applies the same policy to `.json` and returns parsed JSON only after full bounded validation. `cleanup.suggest` accepts at most 1,000 digest/name/size facts, validates literal SHA-256, groups exact duplicate identities, and returns review-only suggestions with `executionAvailable: false`; it performs no filesystem access. `comparison.run` and `classification.propose` return a visible `notReady` semantic-advice result with zero mutation.

- [x] **Step 5: Run tests, strict lints, and commit**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib mcp_transport
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin RUSTFMT=/opt/homebrew/bin/rustfmt cargo fmt --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
```

Commit: `feat: dispatch governed MCP tools`

### Task 3: Loopback-only stateful Streamable HTTP broker

**Files:**
- Create: `src-tauri/src/mcp_transport/http.rs`
- Modify: `src-tauri/src/mcp_transport/auth.rs`
- Modify: `src-tauri/src/mcp_transport/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] **Step 1: Write failing listener and HTTP tests**

Cover literal loopback acceptance and denial of wildcard/LAN/DNS bind inputs; fixed `/mcp` routing; exact bound Host authority; POST content type/Accept requirements; stateful initialize with `Mcp-Session-Id`; subsequent POST/GET/DELETE routing; a global 1 MiB body cap plus smaller grant-specific cap; disabled CORS/preflight; untrusted Origin and CSRF-style requests; one explicitly grant-allowed authenticated origin; port collision; idempotent stop; and revocation while connected.

- [x] **Step 2: Run and verify RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib mcp_transport::http`

Expected: FAIL because no broker exists.

- [x] **Step 3: Implement listener lifecycle**

`McpTransportAuthority` binds `127.0.0.1:<requested-port>` where port `0` requests an OS-assigned port, constructs `StreamableHttpService` with `LocalSessionManager`, stateful mode, exact bound Host, no external session restoration, and a cancellation token. An outer bounded-auth service buffers at most 1 MiB, authenticates credentials before passing bytes onward, applies the grant-specific input limit, rejects `OPTIONS`, and never emits permissive CORS headers. Start/stop/inspect commands expose only status, loopback URL, current executable path, and errors; they expose no grant token.

- [x] **Step 4: Run tests and commit**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib mcp_transport::http`

Commit: `feat: host loopback streamable HTTP MCP`

### Task 4: Standard stdio relay through the trusted broker

**Files:**
- Create: `src-tauri/src/mcp_transport/stdio_relay.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/Cargo.toml`

- [x] **Step 1: Write failing relay unit tests**

Prove strict literal-loopback broker URLs, required environment credentials, no token CLI argument, one JSON object per newline, a 1 MiB line cap, invalid/batch JSON rejection, no stdout logs, stateful session-header retention, 202 notification handling, JSON and bounded SSE response extraction, redirect denial, proxy-disabled client construction, timeout behavior, and token redaction from every error.

- [x] **Step 2: Run and verify RED**

Run: `PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib mcp_transport::stdio_relay`

Expected: FAIL because the relay does not exist.

- [x] **Step 3: Implement headless relay mode**

Before Tauri startup, `main` recognizes exactly `--mcp-stdio-relay --broker-url <literal-loopback-http-url>`. It builds a redirect-free, proxy-free, bounded Reqwest client, copies static credential headers from the three environment variables, retains the broker-issued `Mcp-Session-Id`, forwards protocol-version headers after negotiation, and translates request/notification responses back to newline-delimited stdout. stderr may contain bounded operational diagnostics but never credential/header values or request bodies.

- [x] **Step 4: Run tests and commit**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib mcp_transport::stdio_relay
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo build --bin ai-knowledge-sort
```

Commit: `feat: relay stdio MCP through desktop authority`

### Task 5: Agent Access transport controls and configuration handoff

**Files:**
- Modify: `src/features/agentAccess/types.ts`
- Modify: `src/features/agentAccess/agentAccessClient.ts`
- Modify: `src/features/agentAccess/agentAccessClient.test.ts`
- Modify: `src/features/agentAccess/AgentAccessPanel.tsx`
- Modify: `src/features/agentAccess/AgentAccessPanel.test.tsx`
- Modify: `src/styles.css`

- [x] **Step 1: Write failing client and accessible UI tests**

Assert exact Tauri calls `inspect_mcp_transport`, `start_mcp_transport`, and `stop_mcp_transport`; browser adapters reject all three. UI tests prove explicit start/stop, literal loopback URL, no running claim in browser mode, optional allowed-origin entry with explanation, and direct HTTP plus stdio templates shown only while the one-time token remains in component memory. Dismissing the token removes every credential-bearing template. No token reaches localStorage, sessionStorage, logs, persistent grant summaries, clipboard, or query strings.

- [x] **Step 2: Run and verify RED**

Run:

```bash
npm test -- --run src/features/agentAccess/agentAccessClient.test.ts src/features/agentAccess/AgentAccessPanel.test.tsx
```

- [x] **Step 3: Implement minimal controls and templates**

The default start request uses port `0`. The direct HTTP template shows URL plus three required header names. The stdio template shows current executable, `--mcp-stdio-relay`, loopback broker URL, and the three environment variable names/values; it remains a visible user-reviewed template and is never written to third-party config automatically. Keep the existing one-time-token warning and dismiss action.

- [x] **Step 4: Run frontend tests/build and commit**

Run:

```bash
npm test -- --run
npm run build
```

Commit: `feat: manage local MCP transports in settings`

### Task 6: Real transport smoke, documentation, quality gate, and merge

**Files:**
- Create: `src-tauri/tests/mcp_runtime_smoke.rs`
- Modify: `e2e/source-workbench.spec.ts`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-07-28-governed-local-mcp-transports.md`

- [x] **Step 1: Add direct HTTP and subprocess stdio smoke tests**

Start a real broker on port `0` with an active temporary grant. Direct HTTP must complete initialize, initialized, tools/list, capabilities.read, replay denial, untrusted-Origin denial, and DELETE. Spawn `CARGO_BIN_EXE_ai-knowledge-sort` in relay mode with credentials in environment, exchange the same MCP lifecycle over stdin/stdout, and prove revoked access fails without restarting either transport. Capture stderr and assert it contains neither grant nor session token.

- [x] **Step 2: Update browser E2E and README honestly**

Browser E2E must see the desktop-runtime error and no running broker, URL, token, or config template. README documents the loopback broker, stdio relay, exact credential headers/env names, explicit Origin policy, start/stop lifecycle, four real/read-only tools, two `notReady` semantic adapters, and no cleanup execution. Automatic grant reactivation, model/classification MCP adapters, write-capable MCP tools, GraphRAG, 3D graph, secure keychain, URL profile import, and external third-party runtime installation remain unimplemented.

- [x] **Step 3: Run the full release gate**

Run:

```bash
npm test -- --run
npm run build
npm audit --audit-level=high
npx playwright test --project=chromium
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --test mcp_runtime_smoke
PATH=/opt/homebrew/bin:/usr/bin:/bin RUSTFMT=/opt/homebrew/bin/rustfmt cargo fmt --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

Also scan the branch diff for bearer values, header/body logging, listener addresses, permissive CORS, ambient path opening, direct cleanup execution, command execution beyond the exact relay mode, and any path supplied by the frontend. Review the official SDK lockfile additions with `cargo tree -i rmcp` and `cargo audit` when available; do not claim exhaustive dependency coverage if the audit database is unavailable.

- [ ] **Step 4: Commit, fast-forward merge, and reverify main**

Commit: `docs: record governed local MCP milestone`

Confirm main and the worktree are clean and non-divergent. Under the owner's standing authorization, fast-forward `codex/mcp-local-transports` into `main`, rerun the complete gate on `main`, then remove only `.worktrees/mcp-local-transports`, prune, and delete only the merged branch.

## Self-review

- **Spec coverage:** Task 3 proves MCP-001 loopback/Host/Origin/CSRF transport behavior; Tasks 1–3 bind every request to the existing MCP-004 identity, grant, session, replay, expiry, revocation, scope, tool, and resource checks; Task 2 preserves MCP-002 trusted capability paths and MCP-003 no-execution cleanup; Task 4 supplies stdio without a second authority; Task 6 proves transport equivalence with real subprocess bytes.
- **Trust boundary:** Neither transport reopens persisted display paths. The stdio process has no capability or app-config authority. All actual reads receive a cloned `Dir` only after current authorization and perform no-follow relative opens.
- **Honesty:** Two semantic adapters remain explicit `notReady` results. Browser preview fabricates no runtime. No external Agent configuration is modified automatically.
- **Type consistency:** `allowedHttpOrigins`, `transportOrigin`, `agentId`, `grantId`, `grantToken`, `sessionId`, `sessionToken`, `requestId`, `scopeId`, and limit fields retain exact camelCase across Rust, TypeScript, persisted JSON, MCP handler tests, and UI.
