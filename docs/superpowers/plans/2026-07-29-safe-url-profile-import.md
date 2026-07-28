# Safe URL Profile Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Import a strict declarative classification-profile candidate from an explicitly entered HTTPS URL without permitting SSRF, DNS rebinding, credential forwarding, unbounded network work, secret persistence, automatic approval, or filesystem mutation outside the existing Vault candidate boundary.

**Architecture:** A new Rust `profiles::remote` module owns URL validation, public-address resolution, DNS pinning, manual redirect processing, response bounds, and minimized provenance. The existing `ProfileAuthority` remains the only candidate persistence and approval authority; local and remote bytes enter one shared import path, while remote candidates record a distinct source kind and byte size. Tauri and React expose one explicit URL field and import action, but browser mode remains non-persistent and no URL value is stored after completion.

**Tech Stack:** Tauri 2, Rust 2021, Reqwest 0.12.28, Tokio, URL 2, React 19, TypeScript, Vitest, Playwright.

---

## Scope and safety decisions

- Production URL import accepts only canonical `https` URLs with a domain or literal public IP host. Embedded authority credentials are rejected. Query and fragment input is permitted because signed publication URLs may require a query, but neither value is persisted, rendered, audited, or copied into diagnostics.
- Every direct target and redirect is parsed and validated before a request. DNS is resolved before connection, every returned address must be public, and the validated address set is pinned into a fresh redirect-free Reqwest client with `resolve_to_addrs`. Redirect targets repeat the full validation and resolution process.
- IPv4 unspecified, loopback, private, link-local, carrier-grade NAT, documentation, benchmarking, multicast, reserved, and broadcast ranges are denied. IPv6 unspecified, loopback, unique-local, link-local, multicast, documentation, IPv4-mapped, transition, and non-global-unicast ranges are denied.
- Automatic redirects and system proxies are disabled. At most five redirects are followed manually. No production Authorization, Cookie, Proxy-Authorization, or Referer value is accepted from the UI. Test-only synthetic origin-scoped credentials prove they are permanently stripped after a cross-origin redirect.
- Total fetch duration is 15 seconds, connection duration is at most 5 seconds, and the accepted body is at most the existing `MAX_PROFILE_BYTES` value of 1 MiB. Only `application/json` and `application/*+json`, with optional parameters, are accepted.
- Network errors use bounded constant diagnostics and never format a Reqwest error or URL. Candidate provenance stores only the final fetched-byte SHA-256, byte size, source kind, safe basename, and SHA-256 of a minimized final locator with query and fragment removed.
- A remote candidate remains `unapproved`. Only the existing exact-digest desktop decision can install and activate it. Fetch, parse, validation, timeout, redirect, content-type, and persistence failures leave the active profile and existing Vault records unchanged.

## File map

- Create `src-tauri/src/profiles/remote.rs`: strict URL policy, address classification, resolver pinning, manual redirect loop, bounded body reader, and minimized provenance.
- Modify `src-tauri/src/profiles/mod.rs`: request DTO, async Tauri command, and routing from fetched bytes into `ProfileAuthority`.
- Modify `src-tauri/src/profiles/store.rs`: shared local/remote byte import, `remoteUrl` source kind, byte-size field, backward-compatible record decoding, and remote-specific candidate binding.
- Modify `src-tauri/src/lib.rs`: register only `import_url_profile_candidate`.
- Modify `src/features/profiles/types.ts`: remote source kind, byte-size field, and URL client operation.
- Modify `src/features/profiles/profileClient.ts` and `.test.ts`: exact Tauri invocation and honest browser rejection.
- Modify `src/features/profiles/ProfileReview.tsx` and `.test.tsx`: explicit URL input/action, in-memory clearing, source/size review details, and unchanged approval gate.
- Modify `e2e/source-workbench.spec.ts`: browser URL import fails visibly and creates no candidate.
- Modify `README.md`: document delivered URL boundary and retain model-assisted notice conversion as unimplemented.

### Task 1: Strict network target and fetch boundary

**Files:**
- Create: `src-tauri/src/profiles/remote.rs`
- Modify: `src-tauri/src/profiles/mod.rs`

- [x] **Step 1: Write address and URL policy tests**

Add table-driven tests for:

```rust
for rejected in [
    "file:///tmp/profile.json",
    "http://example.com/profile.json",
    "https://user:secret@example.com/profile.json",
    "https://localhost/profile.json",
    "https://127.0.0.1/profile.json",
    "https://10.0.0.1/profile.json",
    "https://169.254.1.1/profile.json",
    "https://100.64.0.1/profile.json",
    "https://192.0.2.1/profile.json",
    "https://[::1]/profile.json",
    "https://[fc00::1]/profile.json",
    "https://[fe80::1]/profile.json",
    "https://[2001:db8::1]/profile.json",
] {
    assert!(validate_initial_url(rejected, NetworkPolicy::production()).is_err());
}
assert!(validate_initial_url(
    "https://profiles.example.com/ninebot.json?signature=secret#review-secret",
    NetworkPolicy::production(),
).is_ok());
```

Also test the address classifier directly for boundary addresses around every prohibited IPv4 and IPv6 range. Assert mixed public/private DNS results reject the entire target instead of selecting only a public address.

- [x] **Step 2: Run policy tests and verify RED**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib profiles::remote::tests::rejects_unsafe_network_targets
```

Expected: FAIL because `profiles::remote` does not exist.

- [x] **Step 3: Implement canonical URL and public-address validation**

Create these focused internal types:

```rust
pub(crate) struct FetchedProfile {
    pub bytes: Vec<u8>,
    pub source_basename: String,
    pub minimized_locator: String,
}

#[derive(Clone, Copy)]
struct NetworkPolicy {
    scheme: &'static str,
    allow_test_loopback: bool,
    max_redirects: usize,
    connect_timeout: Duration,
    total_timeout: Duration,
}

impl NetworkPolicy {
    fn production() -> Self {
        Self {
            scheme: "https",
            allow_test_loopback: false,
            max_redirects: 5,
            connect_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(15),
        }
    }
}
```

`validate_initial_url` must reject whitespace/control characters, non-canonical parse failures, embedded username/password, missing host, non-HTTPS production schemes, and fragments only after accepting them for later redaction. `resolve_target` must use `tokio::net::lookup_host`, require at least one address, validate every result, deduplicate deterministically, and return `SocketAddr` values using the explicit or scheme-default port.

- [x] **Step 4: Write real redirect, credential, bound, and provenance tests**

Use Axum listeners on literal loopback only under `NetworkPolicy::test_loopback()` to prove actual Reqwest behavior:

1. A valid JSON candidate with `application/json; charset=utf-8` succeeds.
2. A relative redirect is followed and the target is revalidated.
3. A redirect to `http://10.0.0.1/profile.json` fails before a second connection.
4. Two loopback origins record headers; synthetic Authorization and Cookie sent to the first origin never reach the second origin and are not restored if a later redirect returns to the first origin.
5. Six redirects fail.
6. `text/plain`, a declared body above 1 MiB, a chunked body crossing 1 MiB, and a delayed response crossing the test deadline fail.
7. `?signature=synthetic-secret#review-secret` is used for the request but neither secret appears in `minimized_locator` or any error.

- [x] **Step 5: Run fetch tests and verify RED**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib profiles::remote
```

Expected: FAIL because the fetch loop is not implemented.

- [x] **Step 6: Implement pinned, redirect-free, bounded fetching**

For each hop:

1. Validate URL and resolve all addresses.
2. Build a fresh client with:

```rust
reqwest::Client::builder()
    .redirect(reqwest::redirect::Policy::none())
    .no_proxy()
    .connect_timeout(policy.connect_timeout)
    .timeout(policy.total_timeout)
    .resolve_to_addrs(host, &validated_addrs)
```

3. Send GET with only `Accept: application/json, application/*+json`; production provides no credential/cookie headers.
4. For 301, 302, 303, 307, or 308, read exactly one bounded `Location`, resolve it relative to the current URL, permanently disable synthetic test credentials after any origin change, then repeat from step 1.
5. For success, require a JSON media type, reject oversized `Content-Length`, and consume `response.chunk()` while checking the cumulative 1 MiB limit.
6. Wrap the complete redirect loop in one `tokio::time::timeout`.

Return only constant bounded errors such as `Remote profile target is not allowed`, `Remote profile redirect is invalid`, `Remote profile response type is not JSON`, `Remote profile exceeds 1 MiB`, `Remote profile request timed out`, and `Remote profile could not be fetched`.

- [x] **Step 7: Run focused tests and commit**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib profiles::remote
```

Expected: all focused tests pass. Strict all-target Clippy runs in Task 3 after
the production Tauri command references the fetch entry point; before that
connection the intentionally staged production module is dead code outside its
unit-test target.

Commit:

```bash
git add src-tauri/src/profiles/remote.rs src-tauri/src/profiles/mod.rs
git commit -m "feat: fetch remote profiles through a pinned network boundary"
```

### Task 2: Remote candidate persistence and backward compatibility

**Files:**
- Modify: `src-tauri/src/profiles/store.rs`
- Test: `src-tauri/src/profiles/store.rs`

- [x] **Step 1: Write failing persistence tests**

Add tests proving:

```rust
let candidate = authority.import_remote_bytes(
    &vault,
    "ninebot.json",
    "https://profiles.example.com/ninebot.json",
    bytes,
    now,
)?;
assert_eq!(candidate.source_kind, ProfileSourceKind::RemoteUrl);
assert_eq!(candidate.source_byte_size, bytes.len() as u64);
assert_eq!(candidate.status, CandidateStatus::Unapproved);
assert!(candidate.approval.is_none());
```

Verify the persisted candidate JSON contains `sourceKind: "remoteUrl"` and `sourceByteSize`, contains neither query nor fragment secrets, and retains literal fetched-byte `SHA-256`. Verify active state and installed profiles are unchanged until the existing exact-digest approval call. Verify identical remote bytes and minimized locator are idempotent; a different remote provenance creates a separate candidate rather than colliding with a local import.

Deserialize a version-1 local candidate fixture without `sourceByteSize` and assert it loads as zero/unknown without changing its candidate identity or approval state.

- [x] **Step 2: Run store tests and verify RED**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib profiles::store
```

Expected: FAIL because `RemoteUrl`, `source_byte_size`, and `import_remote_bytes` do not exist.

- [x] **Step 3: Implement one shared byte-import core**

Extend:

```rust
pub enum ProfileSourceKind {
    LocalFile,
    RemoteUrl,
}

pub struct ProfileCandidateRecord {
    // existing fields
    #[serde(default)]
    pub source_byte_size: u64,
}
```

Keep `import_local_bytes` as a thin wrapper. Add `import_remote_bytes` and one private `import_bytes` that parses and validates the same strict candidate schema, computes fetched-byte and minimized-locator identities, calculates the same reviewable diff, writes the content-addressed source once, and creates only an unapproved candidate.

Keep the existing local candidate binding unchanged. Bind remote candidate identity to source kind, fetched-byte digest, profile id/version, and minimized-locator digest so the same bytes imported locally or from a distinct reviewed remote provenance cannot conflict.

- [x] **Step 4: Run store/full Rust tests and commit**

Run:

```bash
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib profiles::store
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
```

Expected: all tests pass, including existing local import/decision vectors.

Commit:

```bash
git add src-tauri/src/profiles/store.rs
git commit -m "feat: persist review-only remote profile candidates"
```

### Task 3: Tauri command and typed client boundary

**Files:**
- Modify: `src-tauri/src/profiles/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/features/profiles/types.ts`
- Modify: `src/features/profiles/profileClient.ts`
- Modify: `src/features/profiles/profileClient.test.ts`

- [x] **Step 1: Write failing typed-client tests**

Extend the expected invocation sequence with:

```ts
await client.importUrlCandidate(
  "https://profiles.example.com/ninebot.json?signature=synthetic#review",
);

expect(invoke).toHaveBeenCalledWith("import_url_profile_candidate", {
  request: {
    url: "https://profiles.example.com/ninebot.json?signature=synthetic#review",
  },
});
```

Assert browser mode rejects `importUrlCandidate` with the same desktop-runtime error and returns no fabricated candidate.

- [x] **Step 2: Run client tests and verify RED**

Run:

```bash
npm test -- --run src/features/profiles/profileClient.test.ts
```

Expected: FAIL because the client operation does not exist.

- [x] **Step 3: Implement the native command**

Add a strict request DTO:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportUrlProfileCandidateRequest {
    url: String,
}
```

`import_url_profile_candidate` must:

1. Call `remote::fetch_profile_url(&request.url).await`.
2. Acquire the current authorized Vault only after a successful bounded fetch.
3. Pass only `source_basename`, `minimized_locator`, fetched bytes, and current time to `ProfileAuthority::import_remote_bytes`.
4. Return the unapproved `ProfileCandidateRecord`.

Register only this command in `lib.rs`. It must not approve, activate, classify, rename, archive, or expose a generic fetch API.

- [x] **Step 4: Implement exact TypeScript types and adapter**

Change:

```ts
export type ProfileSourceKind = "localFile" | "remoteUrl";

export interface ProfileCandidateRecord {
  readonly sourceByteSize: number;
}

export interface ProfileClient {
  importUrlCandidate(url: string): Promise<ProfileCandidateRecord>;
}
```

The Tauri adapter invokes `import_url_profile_candidate` with `{ request: { url } }`. The browser adapter rejects without network access.

- [x] **Step 5: Run focused/full tests and commit**

Run:

```bash
npm test -- --run src/features/profiles/profileClient.test.ts
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib profiles
npm test -- --run
```

Expected: all Rust profile and frontend tests pass.

Commit:

```bash
git add src-tauri/src/profiles/mod.rs src-tauri/src/lib.rs src/features/profiles/types.ts src/features/profiles/profileClient.ts src/features/profiles/profileClient.test.ts
git commit -m "feat: expose explicit URL profile import"
```

### Task 4: Reviewable UI without URL persistence

**Files:**
- Modify: `src/features/profiles/ProfileReview.tsx`
- Modify: `src/features/profiles/ProfileReview.test.tsx`
- Modify: `src/styles.css`
- Modify: `e2e/source-workbench.spec.ts`

- [x] **Step 1: Write failing component and browser tests**

Component tests must enter an HTTPS URL, click `Import URL`, and assert the exact client call. After success, assert the URL input is empty, the candidate shows `Remote URL`, fetched byte size, and exact SHA-256 review checkbox, while approval remains disabled until the checkbox is selected.

Error tests must assert the entered URL is cleared after an attempt and a synthetic query/fragment secret is absent from the rendered DOM and error message.

Browser E2E must click `Import URL`, see `Desktop runtime is required for profile operations.`, and confirm no candidate, digest, source size, or active-profile change is fabricated.

- [x] **Step 2: Run UI tests and verify RED**

Run:

```bash
npm test -- --run src/features/profiles/ProfileReview.test.tsx
npx playwright test e2e/source-workbench.spec.ts --project=chromium
```

Expected: FAIL because the URL form does not exist.

- [x] **Step 3: Implement the explicit in-memory form**

Add controlled `urlText` state with:

- `<input aria-label="Profile URL" type="url" autoComplete="off" spellCheck={false}>`
- `<button aria-label="Import profile URL">Import URL</button>`
- helper text: `HTTPS JSON only. Query and fragment values are never retained.`

Disable both import actions while busy. Copy `urlText` into a local variable, clear React state before awaiting the client, then refresh state through `client.inspect()`. Never log, persist, reflect, or place the URL in a title, query string, clipboard, storage API, or error.

Candidate review must display `Local file` or `Remote URL`, a bounded formatted byte count, profile identity/version, truncated fetched-byte SHA-256, diff, and the unchanged exact-digest approval checkbox.

- [x] **Step 4: Run frontend tests/build and commit**

Run:

```bash
npm test -- --run
npm run build
npx playwright test --project=chromium
```

Expected: 100% existing and new tests pass; browser preview remains honest.

Commit:

```bash
git add src/features/profiles/ProfileReview.tsx src/features/profiles/ProfileReview.test.tsx src/styles.css e2e/source-workbench.spec.ts
git commit -m "feat: review remote profile imports in context"
```

### Task 5: Security regression, documentation, and release gate

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-07-29-safe-url-profile-import.md`

- [x] **Step 1: Add a no-mutation failure regression**

In the remote/store test fixture, snapshot installed, candidate, decision, activation, and source record directories before invalid URL, redirect, timeout, content-type, oversized, malformed JSON, and executable-shape attempts. Assert every failure leaves the snapshot byte-for-byte identical. For a successful fetch followed by invalid profile parsing, assert no candidate/source record is written.

- [x] **Step 2: Update README honestly**

Document:

- explicit user-entered HTTPS import;
- public-address validation and DNS pinning at every hop;
- manual five-hop redirects, proxy/automatic-redirect disabling, and cross-origin credential stripping;
- 15-second/1-MiB/JSON bounds;
- fetched-byte SHA-256, byte size, remote source kind, minimized locator digest, reviewable diff, and unapproved state;
- query/fragment redaction and constant diagnostics;
- offline/non-network local features remain usable and unchanged.

Retain full Ninebot taxonomy, model-assisted notice/document conversion, automatic classification, and unapproved-profile activation as unimplemented.

- [x] **Step 3: Run security-sensitive source scan**

Run:

```bash
rg -n "Authorization|Cookie|Proxy-Authorization|Referer|redirect|no_proxy|resolve_to_addrs|query|fragment|println!|eprintln!|tracing::|log::" src-tauri/src/profiles src/features/profiles
rg -n "localStorage|sessionStorage|clipboard|location\\.href|console\\." src/features/profiles
git diff --check
```

Review every match. No production path may forward credentials, format URLs into errors, use automatic redirects/proxies, persist query/fragment values, or create approval/activation during import.

- [x] **Step 4: Run the full release gate**

Run:

```bash
npm test -- --run
npm run build
npm audit --audit-level=high
npx playwright test --project=chromium
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo fmt -- --check
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --lib
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo test --test mcp_runtime_smoke -- --nocapture
PATH=/opt/homebrew/bin:/usr/bin:/bin cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

Expected: all tests, build, audit, format, smoke, and strict lint gates pass.

- [ ] **Step 5: Commit, fast-forward merge, and reverify main**

Commit:

```bash
git add README.md docs/superpowers/plans/2026-07-29-safe-url-profile-import.md
git commit -m "docs: record safe URL profile import milestone"
```

Confirm the feature worktree and `main` are clean, the branch is ahead-only, and `git merge --ff-only codex/url-profile-import` succeeds. Rerun the complete release gate on `main`, then remove only `.worktrees/url-profile-import`, prune worktree metadata, and delete the merged branch.

## Completion audit

- **RULE-003:** Tasks 1–4 cover URL source form, fetched-byte identity/size, minimized provenance, diff, unapproved state, user decision, offline failure, SSRF/redirect handling, credential stripping, and bounds.
- **SAFE-004:** Candidate records expose source kind, byte size, SHA-256, profile/version, diff, and later attributable exact-digest decision without storing sensitive URL values.
- **SAFE-007:** Every URL and redirect is revalidated, every resolved address is public and pinned for connection, network work is bounded, and every failure is visible/non-mutating.
- **ARCH-001:** Only an explicit user action starts network work. Existing local archive, Vault, Markdown, graph, and deterministic safety functions are unchanged and remain offline-capable.
- **No scope expansion:** This milestone does not scrape arbitrary announcements, invoke a model, convert prose into rules, approve a profile, classify a file, rename/archive/delete a source, expose URL import through MCP, or add generic network access.
- **Type consistency:** `remoteUrl`, `sourceByteSize`, `importUrlCandidate`, `ImportUrlProfileCandidateRequest`, `source_basename`, `minimized_locator`, and `FetchedProfile` retain one meaning across Rust, persisted JSON, TypeScript, tests, and UI.
- **Placeholder scan:** Every implementation and verification step above names exact files, APIs, limits, expected failure/pass state, and commands; no undefined follow-up implementation is delegated.
