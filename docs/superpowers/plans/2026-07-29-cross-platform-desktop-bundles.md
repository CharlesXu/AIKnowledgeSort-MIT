# Cross-platform Desktop Bundles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every pull request and `main` push prove that AI Knowledge Sort builds into unsigned, downloadable Tauri desktop bundles and starts its desktop runtime on Windows, macOS, and Linux.

**Architecture:** Extend the existing least-privilege CI matrix instead of creating a publishing workflow with write permissions. A small Node verifier treats bundle configuration, platform matrix coverage, startup smoke execution, and artifact upload as a release contract; a bounded `--desktop-smoke` process mode starts the real Tauri runtime and exits on `RunEvent::Ready` without changing normal application behavior.

**Tech Stack:** Tauri 2, Rust, React/TypeScript, Node.js 24, GitHub Actions, `actions/upload-artifact`

---

## Scope and success evidence

This milestone implements the build-and-start portion of `ARCH-001` and the
first-release acceptance gate in `docs/IMPLEMENTATION_SPEC.md`. It does not
claim installer signing, notarization, updater publication, or end-user release
distribution.

Success requires all of the following:

- `bundle.active` is true.
- CI builds `app,dmg` on macOS, `deb,appimage` on Ubuntu, and `nsis` on Windows.
- Every matrix entry starts the compiled desktop runtime in bounded smoke mode.
- Every matrix entry uploads only its platform bundle and fails if no bundle is
  produced.
- CI retains `contents: read`; it creates no release and uses no signing secret.
- The full local frontend, Rust, Clippy, formatting, E2E, and desktop no-bundle
  gates pass.
- The pull-request and subsequent `main` CI runs pass on all three platforms.

### Task 1: Add a failing release-contract verifier

**Files:**
- Create: `scripts/verify-release-contract.mjs`
- Modify: `package.json`

- [ ] **Step 1: Write the verifier before changing CI or Tauri configuration**

Create a standard-library-only Node script that reads `package.json`,
`src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and
`.github/workflows/ci.yml`. It must collect all violations and exit nonzero
unless:

```text
package, Cargo, and Tauri versions are identical
bundle.active is true
macos-14 maps to app,dmg
ubuntu-22.04 maps to deb,appimage
windows-2022 maps to nsis
each platform runs scripts/run-desktop-smoke.mjs
actions/upload-artifact is pinned to a full commit SHA
if-no-files-found is error
permissions remain contents: read
```

The verifier must never parse or emit secrets and must report only fixed
contract labels plus safe file names.

- [ ] **Step 2: Register and run the verifier**

Add:

```json
"verify:release": "node scripts/verify-release-contract.mjs"
```

Run:

```bash
npm run verify:release
```

Expected: FAIL because bundling, the platform matrix, startup smoke, and
artifact upload are not configured yet.

- [ ] **Step 3: Commit the red contract**

```bash
git add package.json scripts/verify-release-contract.mjs
git commit -m "test: define cross-platform release contract"
```

### Task 2: Add a bounded real-runtime desktop smoke mode

**Files:**
- Create: `scripts/run-desktop-smoke.mjs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add failing Rust argument-selection tests**

Add a pure function that selects normal desktop, MCP stdio relay, or desktop
smoke mode from exact process arguments. Tests must prove:

```rust
assert_eq!(mode(["app"]), ProcessMode::Desktop);
assert_eq!(mode(["app", "--desktop-smoke"]), ProcessMode::DesktopSmoke);
assert_eq!(
    mode([
        "app",
        "--mcp-stdio-relay",
        "--broker-url",
        "http://127.0.0.1:3000/mcp"
    ]),
    ProcessMode::McpStdioRelay
);
assert!(mode(["app", "--desktop-smoke", "extra"]).is_err());
assert!(mode(["app", "--desktop-smoke", "--mcp-stdio-relay"]).is_err());
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib process_mode
```

Expected: FAIL because the mode parser does not exist.

- [ ] **Step 2: Implement the minimal exact process-mode parser**

Expose only the enum and parser needed by `main.rs`. Existing relay environment
validation remains owned by the relay. Unknown arguments continue into normal
desktop mode only when no reserved AIKS process switch is present. The existing
exact relay shape `--mcp-stdio-relay --broker-url <url>` is preserved;
conflicting or malformed reserved switches exit with code 2.

- [ ] **Step 3: Build the same Tauri application in both desktop modes**

Extract the existing builder chain into one private builder function. Normal
mode retains existing `.run(...)` behavior. Smoke mode uses the same plugins,
managed authorities, window handler, invoke handler, generated context, and
configured main window, then exits successfully only after receiving
`tauri::RunEvent::Ready`.

The smoke path must not create a second implementation of application setup and
must not bypass Tauri initialization.

- [ ] **Step 4: Add a cross-platform timeout wrapper**

`scripts/run-desktop-smoke.mjs` must:

- accept exactly one relative executable path;
- resolve it beneath the repository root;
- spawn it with only `--desktop-smoke`;
- inherit stdio;
- fail if it exits nonzero;
- terminate it after 30 seconds and fail with a bounded diagnostic;
- reject missing, outside-root, or non-file targets.

- [ ] **Step 5: Run focused and local desktop checks**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib process_mode
npm run tauri build -- --debug --no-bundle
node scripts/run-desktop-smoke.mjs src-tauri/target/debug/ai-knowledge-sort
```

On macOS the last command must exit zero after the real Tauri runtime reaches
`RunEvent::Ready`.

- [ ] **Step 6: Commit desktop startup smoke**

```bash
git add src-tauri/src/lib.rs src-tauri/src/main.rs scripts/run-desktop-smoke.mjs
git commit -m "test: smoke desktop runtime startup"
```

### Task 3: Build and retain unsigned platform bundles in CI

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Enable Tauri bundling**

Change only:

```json
"bundle": {
  "active": true,
  "icon": ["icons/icon.png"]
}
```

Platform bundle targets remain explicit CI inputs so the repository does not
silently choose a different installer format.

- [ ] **Step 2: Convert the Rust matrix to explicit platform entries**

Use this semantic matrix:

```yaml
include:
  - os: macos-14
    bundles: app,dmg
    executable: src-tauri/target/release/ai-knowledge-sort
    artifact_name: AIKnowledgeSort-macOS
    artifact_paths: |
      src-tauri/target/release/bundle/macos/*.app
      src-tauri/target/release/bundle/dmg/*.dmg
  - os: ubuntu-22.04
    bundles: deb,appimage
    executable: src-tauri/target/release/ai-knowledge-sort
    artifact_name: AIKnowledgeSort-Linux
    artifact_paths: |
      src-tauri/target/release/bundle/deb/*.deb
      src-tauri/target/release/bundle/appimage/*.AppImage
  - os: windows-2022
    bundles: nsis
    executable: src-tauri/target/release/ai-knowledge-sort.exe
    artifact_name: AIKnowledgeSort-Windows
    artifact_paths: |
      src-tauri/target/release/bundle/nsis/*.exe
```

Keep the existing platform tests. Add Node setup and `npm ci`, then run:

```bash
npm run tauri build -- --bundles ${{ matrix.bundles }}
node scripts/run-desktop-smoke.mjs ${{ matrix.executable }}
```

Linux dependencies must include `xvfb`; run the smoke command under `xvfb-run`
through an explicit matrix prefix or a dedicated Linux-only step. macOS and
Windows run the same wrapper directly.

- [ ] **Step 3: Upload exact bundle outputs**

Use the full commit SHA for `actions/upload-artifact@v4`, set
`if-no-files-found: error`, and retain artifacts for 14 days. Keep workflow
permissions at:

```yaml
permissions:
  contents: read
```

No release, tag, signing, notarization, updater, or GitHub write permission is
added.

- [ ] **Step 4: Make the release contract green**

Run:

```bash
npm run verify:release
```

Expected: PASS and a one-line summary naming the three platform bundle groups.

- [ ] **Step 5: Commit the CI bundle matrix**

```bash
git add .github/workflows/ci.yml src-tauri/tauri.conf.json
git commit -m "ci: build cross-platform desktop bundles"
```

### Task 4: Document the evidence boundary and run all gates

**Files:**
- Modify: `README.md`
- Modify: this plan

- [ ] **Step 1: Document delivered behavior without overclaiming**

Add a “Cross-platform desktop build evidence” section stating:

- pull requests and `main` build unsigned macOS app/DMG, Linux DEB/AppImage,
  and Windows NSIS artifacts;
- each runner starts the real runtime in bounded smoke mode;
- artifacts are CI evidence, not signed public releases;
- signing, notarization, updater publication, and hands-on installation
  acceptance remain separate release work.

- [ ] **Step 2: Run the complete local quality gate**

```bash
npm run verify:release
npm test -- --run
npm run build
npm audit --audit-level=high
npm run e2e
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run tauri build -- --debug --no-bundle
node scripts/run-desktop-smoke.mjs src-tauri/target/debug/ai-knowledge-sort
```

Expected: all commands pass; the existing Vite chunk-size warning is
non-blocking.

- [ ] **Step 3: Inspect the release diff and provenance**

Verify:

```bash
git diff origin/main...HEAD
git status --short
rg -n "(BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY|sk-[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,})" \
  .github scripts src-tauri README.md
```

Expected: only milestone files differ, status is clean after commit, and the
secret scan has no credential hit.

- [ ] **Step 4: Commit documentation**

```bash
git add README.md docs/superpowers/plans/2026-07-29-cross-platform-desktop-bundles.md
git commit -m "docs: record desktop bundle evidence"
```

- [ ] **Step 5: Push, open the pull request, and require remote evidence**

Push `codex/cross-platform-desktop-bundles`, open a ready pull request, and wait
for frontend plus all macOS, Ubuntu, and Windows jobs to finish. Any missing
artifact, smoke timeout, bundle failure, or test failure blocks merge.

- [ ] **Step 6: Fast-forward exact tested bytes to main**

After the pull-request run is green, use the owner-authorized non-force
fast-forward merge. Verify the PR merge SHA equals the branch head, verify
remote `main` equals that SHA, then wait for the `main` push CI run to finish
green before closing this milestone.

## Self-review

- **Spec coverage:** This plan covers only cross-platform build and bounded
  startup evidence for `ARCH-001` and first-release acceptance. It explicitly
  does not claim signed distribution or manual installer acceptance.
- **Trust boundary:** CI remains read-only, smoke mode reuses the full Tauri
  builder, artifacts are unsigned, and no secret or publishing path is added.
- **Type consistency:** The same exact `--desktop-smoke` switch is used by the
  Rust parser, Node timeout wrapper, release verifier, and all platform jobs.
- **Placeholder scan:** No task contains an undefined implementation
  placeholder or a deferred code step.
