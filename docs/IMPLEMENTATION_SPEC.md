# AI Knowledge Sort Clean Implementation Specification

> This document is the only product specification provided to implementation
> Agents. It contains no upstream source code, identifiers, module structure,
> internal algorithms, tests, resources, or source-reading conversation.

**License target:** MIT

**Copyright:** Copyright (c) 2026 Charles Xu and Segway-Ninebot

## Product goal

Build a local-first cross-platform desktop application that safely classifies
and canonically names files, preserves at least one original-format archival
copy, builds editable Markdown knowledge and an evidence-backed graph, and
offers controlled MCP access to Agent runtimes.

## Terms and invariants

- **Original** means a readable file retaining the imported format and bytes; Markdown, OCR, summaries, vectors, previews, and graph records are derived artifacts and shall not satisfy original preservation. [FILE-002, FILE-003]
- **Content identity** means the literal algorithm label `SHA-256` plus a lowercase 64-hex-character digest computed from file bytes; rename and relocation shall not alter that identity. [FILE-001]
- **Registered original** means an original whose readable location, content identity, original format, and archive commit are recorded in the authoritative Vault. At least one readable, freshly verified registered original shall survive every committed or undone destructive operation. [FILE-002, SAFE-005]
- **Vault** means the single authoritative local workspace for archive registrations, Markdown knowledge, graph claims, operation state, and audit evidence; a second Vault may become authoritative only through an explicit user-approved authority transfer. [ARCH-003]
- **Trusted safety core** means deterministic validation of grants, paths, identities, confirmations, atomic commits, and original-preservation invariants. Semantic models and Agents shall not bypass or replace this core. [AGENT-003, MCP-002]
- **Critical operation** means archive commit, rename, relocation, trash, permanent deletion, undo, profile approval, or authority transfer; each critical operation shall have persistent state and an attributable audit outcome. [SAFE-004]
- The local-first baseline shall keep archive operations, Vault browsing, Markdown editing, graph browsing, and deterministic safety checks usable without network access. [ARCH-001]
- Cleanup shall be disabled by default, user initiated, bounded to selected eligible copies, and incapable of removing the last valid original. [SAFE-001, SAFE-002]

## User journeys

1. The user selects local files or directories through the source tree or drops a mixed local batch, grants an exact scope, reviews a deduplicated discovery/import proposal, selects an approved profile version, reviews classification and canonical-name proposals, and confirms the archive plan; the application shall commit only verified safe operations. [ARCH-002, RULE-001, NAME-004, FILE-004, UI-006, UI-007, SAFE-009]
2. After archive confirmation, the user selects eligible originals for Markdown generation, edits them in source, live-preview, or reading mode, and reviews evidence-backed graph relations in the same continuous workflow. [ARCH-002, KNOW-001, KNOW-002, KNOW-005]
3. The user imports a declarative profile candidate from a local file or a user-provided URL, inspects provenance and diff, and explicitly approves or rejects it before it can affect committed classification. [RULE-003]
4. The user compares independent desktop-model and Agent-side proposals, inspects the Agent adjudication reason and evidence, and confirms any resulting critical filesystem operation after deterministic validation. [AGENT-001, AGENT-002, AGENT-003]
5. The user enables cleanup, selects verified duplicate copies, confirms the bounded selection, and receives trash behavior unless a separate permanent-delete confirmation is completed. [SAFE-001, SAFE-003]
6. The user or a locally connected Agent uses granted MCP capabilities; the user can inspect, expire, or revoke grants and shall retain final confirmation control over critical operations. [MCP-001, MCP-004]

## Functional requirements

- The product shall be a Tauri 2 desktop application with a React and TypeScript interface and Rust trusted capabilities, distributed for Windows, macOS, and Linux. [ARCH-001]
- Archive, knowledge creation, and graph review shall form one continuous two-stage workflow, with confirmed archival preceding knowledge generation for each item. [ARCH-002, KNOW-001]
- One authoritative Vault shall hold product state, while imported source locations remain preserved by default. [ARCH-003, FILE-005]
- Each file shall be addressed by its SHA-256 content identity independently of its name or location. [FILE-001]
- Every successful archive result shall expose a readable, SHA-256-verified registered original; derived artifacts shall remain separately identifiable and shall never replace it. [FILE-002, FILE-003]
- Filesystem-changing plans shall remain proposals until user confirmation and trusted-core validation; unapproved, failed, offline-dependent, or invalid operations shall produce no filesystem mutation. [FILE-004, SAFE-007]
- Classification shall use exact versions of declarative, non-executable profiles and attach that version to every proposal. [RULE-001]
- Authorized owned classification specifications may be rewritten into the clean profile format, while any embedded third-party material shall remain excluded until separately reviewed. [RULE-002, SAFE-008]
- Conflicting or ambiguous classification shall create a dedicated review item and shall not create pending, unclassified, or equivalent catch-all directories. [RULE-004]
- The Ninebot domain profile shall ship as draft/pilot rather than approved production policy. [RULE-005]

## Classification profile format

A profile shall use a documented declarative, non-executable, versioned representation that exposes profile identity, exact version, approval status, taxonomy, governance, rules, provenance, and evidence to users and conformance tests. A taxonomy node shall have a unique identity, canonical label, depth, parent identity, canonical path, and optional aliases. Governance shall distinguish the unique primary archive category from cross-domain knowledge links and generated indexes. Serialization, schema field names, rule vocabulary, and storage are implementation freedoms. [RULE-001]

Each classification proposal shall expose the selected profile identity and exact version, relevant rule identities, evidence references, proposed destination, and proposal or dedicated-review status; only an approved profile may support a committed classification. [RULE-001, RULE-004]

A candidate import record shall expose source kind, import time, literal `SHA-256` algorithm label, 64-hex digest of imported bytes, parsed profile identity and version, candidate status, a reviewable taxonomy-and-rule diff against the selected base version, and approval evidence. Stored source provenance shall use a minimized or redacted locator or locator digest sufficient for review and shall not retain embedded credentials or sensitive query and fragment values by default. Candidate status shall remain unapproved until a user decision records actor, time, decision, and reviewed digest. [RULE-003, SAFE-004]

Invalid, executable, unverifiable, unreachable, or unapproved candidate content shall leave the active profile and filesystem unchanged; URL import failure while offline shall be visible and non-mutating. [RULE-001, RULE-003, SAFE-007]

Profile matching technique, representation, parsing, and rule vocabulary are implementation freedoms, but committed results shall be reproducible from the recorded profile version and evidence, and missing evidence shall route to review rather than fabricated classification. [RULE-001, RULE-004]

The bundled Ninebot profile shall contain the authorized discussion taxonomy
version `0.3.0-draft`: 14 L1, 94 L2, 179 L3, and 179 L4 nodes. It shall retain
the canonical SN-02 label from the classification tree and the differing usage
manual term as an explicit alias. It shall remain draft, inactive, and
non-committable until a separately imported exact candidate digest is approved.
The discussion dictionary may narrow semantic candidates but shall not become
executable or literal keyword-placement rules. [RULE-001, RULE-002, RULE-005]

The Ninebot governance profile shall assign each source archive one primary
category supported by semantic evidence. Organization, business unit, product
line, project, IPD stage, product element, owner scope, and library
classification are metadata projections rather than parallel physical archive
copies. Knowledge nodes may link across domains after archive confirmation;
only high-value, cross-domain, or explicitly requested content requires an
independent knowledge node, and generated indexes shall link rather than copy
source or Markdown bodies. [ARCH-002, FILE-003, RULE-004, KNOW-001, KNOW-005]

## Canonical naming contract

- Canonical naming shall propose a meaningful replacement for unreadable, numeric-only, or business-meaningless names when sufficient evidence exists. [NAME-001]
- A proposal shall preserve the original extension and meaningful project, model, regulation, and version tokens supported by evidence. [NAME-002]
- Names shall use consistent Unicode and cross-platform normalization, reject unsafe path characters and reserved platform names, and resolve identical collisions deterministically on all supported platforms. The particular normalization library and collision suffix are implementation freedoms. [NAME-003]
- Required facts shall never be invented; missing or conflicting evidence shall produce a review item and no rename. [NAME-004]
- Every committed rename shall record original name and path, canonical name and path, applied naming rule, content identity, decision evidence, confirmation, and outcome. [NAME-004, FILE-001, SAFE-004]

## Archival transaction contract

An archive operation shall expose persistent states sufficient to distinguish `proposed`, `copying`, `verified`, `committed`, `failed`, and `abandoned`; each transition shall identify the operation, item, source, intended destination, content identity, confirmation state, and outcome. Equivalent names are allowed if these observable distinctions remain testable. [FILE-004, SAFE-004]

Commit shall follow this externally testable transaction: retain the source, write a temporary destination, confirm destination readability, independently compute source and destination SHA-256 identities, require equality with the `SHA-256` label, register the original, and atomically expose the committed destination. A mismatch, interruption, unreadable output, or registration failure shall prevent a committed archive result and leave the source unchanged. [FILE-002, FILE-004, FILE-005]

Archive source and destination authorization and containment shall hold at operation time through commit after platform-aware path resolution. Traversal, symlink or junction escape, parent replacement, target substitution, out-of-scope location, expired permission, or scope drift introduced before or during the operation shall reject or safely abort the transaction without out-of-scope mutation; the enforcement mechanism is an implementation freedom. [MCP-002, MCP-004, SAFE-007]

Every destructive confirmation shall be bound to one immutable reviewed plan snapshot that exposes exact SHA-256 identities, resolved source and destination paths, disposition, plan version, applicable grant, grant expiry, and a nonce or equivalent single-use replay-resistant binding. Any identity, path, scope, grant, expiry, plan, or relevant filesystem-state change shall invalidate the confirmation and require deterministic revalidation plus fresh user review. [FILE-001, MCP-004, SAFE-003, SAFE-004, SAFE-007]

After startup or post-crash reconciliation, each archive item shall be observably either not committed, with no authoritative registration or exposed authoritative destination, or fully committed, with a readable SHA-256-verified destination and a matching authoritative registration. In both outcomes, the source shall remain readable, byte-unchanged, and valid under an independently recomputed SHA-256 identity. A phantom registration or orphan authoritative destination shall not survive reconciliation, including crashes between registration and exposure or between exposure and final lifecycle completion. [FILE-002, FILE-004, FILE-005, SAFE-004]

Cleanup eligibility shall require two or more readable registered originals with freshly and independently verified matching SHA-256 identities. Derived artifacts, weaker digests, stale records, and the last valid original shall be ineligible. [FILE-002, SAFE-001, SAFE-005]

Cleanup shall move selected eligible copies to platform trash by default. Permanent deletion shall require a separate confirmation after the exact cleanup selection, and Agent suggestion shall never count as either confirmation. [SAFE-002, SAFE-003]

Undo shall be bounded to the recorded operation and shall rerun current path, permission, identity, and last-original checks; an unsafe undo shall be refused without partial mutation. [SAFE-005, SAFE-007]

## Knowledge and graph contract

- Knowledge generation shall accept only a confirmed, readable, SHA-256-valid registered original and shall leave that archive intact on derived-work failure. [KNOW-001, SAFE-006]
- Authoritative knowledge shall be UTF-8 Markdown stored in the single Vault; rendered views, indexes, and graph projections are derived. [ARCH-003, KNOW-002]
- A knowledge artifact shall retain provenance to the registered original identity and cited evidence spans or records sufficient for a user to inspect the basis of its claims. [KNOW-001, KNOW-005]
- A graph relation shall contain a stable relation identifier, source node, relation type, target node, status, and one or more archived-evidence references before it can be confirmed. Relations lacking evidence shall remain unconfirmed or enter review. [KNOW-005]
- Users shall be able to accept, revise, reject, and inspect graph relations without altering the archived original. [FILE-003, KNOW-005]

## Markdown and Mermaid editor

- The same authoritative Markdown source shall support source, live-preview, and reading modes with edits persisting across mode changes. [KNOW-002]
- The editor shall support GFM, frontmatter, tables, task lists, footnotes, callouts, wiki links, and block references with local representations documented by the implementation. [KNOW-003]
- Markdown and Mermaid blocks shall coexist in one document; valid diagrams shall render locally, while invalid diagrams shall retain source and show an actionable diagnostic near the affected content. [KNOW-004]
- Rendering shall treat document content as untrusted data, shall disable renderer network access by default, and shall not load external subresources. Navigable links shall use an explicit safe-scheme allowlist and require user-visible handling outside the renderer. [KNOW-004]
- Generated HTML, DOM, and SVG shall be sanitized before display; scripts, event handlers, `foreignObject`, unsafe URLs or schemes, unsafe Mermaid directives, and equivalent active content shall be blocked or escaped. [KNOW-004]
- Rendering shall be isolated from filesystem and application privileges, and malicious document content shall not invoke a privileged application bridge or bypass normal permission and confirmation checks. The isolation mechanism is an implementation freedom. [KNOW-004, MCP-002]
- A parse or render failure shall preserve source text and keep unaffected document content usable. [KNOW-002, KNOW-004]

## Agent adjudication and MCP

The desktop model and Agent-side model shall receive identical file identity, exact rule snapshot, and evidence, and shall produce independent, distinguishable proposals. Provider, prompt, and model choice are implementation freedoms. [AGENT-001]

Agent adjudication shall end in `accept`, `revise`, `reject`, or `review` and shall record a reason plus evidence references. It shall remain semantic advice until the deterministic core separately validates and the user confirms any critical operation. [AGENT-002, AGENT-003]

Missing, inconsistent, invalid, timed-out, or failed model output shall create a visible failure or review state and shall not authorize filesystem mutation. [AGENT-004, SAFE-007]

MCP shall support local stdio and loopback-only Streamable HTTP. Non-loopback binding or connection shall be prohibited. [MCP-001]

Every MCP request shall be bound to an authenticated caller and session, checked against Agent identity, directory scope, allowed tools, expiry, revocation, and resource limits at request time, and protected against replay. Caller spoofing, session substitution, reused requests, expired grants, and revoked grants shall be denied without filesystem mutation; authentication and replay-defense mechanisms are implementation freedoms. [MCP-004, SAFE-007]

Loopback Streamable HTTP shall reject browser cross-origin or CSRF-style requests unless the caller and origin are explicitly authenticated and allowed for that session. Loopback location alone shall not grant authorization. [MCP-001, MCP-004]

Agent-facing tools may read, analyze, plan, propose, and request semantic approval within grants, but all filesystem-changing requests shall pass through the same trusted core and user confirmations as desktop requests. [MCP-002, AGENT-003]

No Agent-only or direct duplicate-cleanup execution tool shall exist; Agents may only submit bounded cleanup suggestions for user review. [MCP-003, SAFE-002]

## UI and interaction requirements

- The UI may reuse or rewrite authorized owned first-party UI material only within recorded ownership scope; embedded third-party material shall require separate review. [UI-001, SAFE-008]
- The primary workbench shall use three adjustable, collapsible, and splittable panes for navigation, active content, and contextual graph or review work; the last valid layout shall persist across relaunch. [UI-002]
- Invalid persisted layout data shall fall back to a usable three-pane arrangement without losing Vault content. [UI-002]
- Graph history shall provide a compact playback bar with a logical height from 32 through 36 pixels across supported display scales. [UI-003]
- Figma may produce design artifacts but shall not be a runtime dependency or a source of unreviewed executable application behavior. [UI-004]
- Interaction may be inspired by Obsidian's three-pane knowledge workbench, but copied branding or proprietary visual assets shall be prohibited. [UI-005]
- The navigation area shall expose a narrow left toolbar beside a source tree. Every displayed file and directory shall have a checkbox; selecting a directory shall select all eligible discovered descendants, partial descendant selection shall display the directory as indeterminate, and changing a child shall update every displayed ancestor. A mixed selection of files and directories shall remain explicit and resolve to one deduplicated set of eligible discovered items. [UI-006]
- An operating-system Drop containing local files and directories shall create only a scoped discovery/import proposal. Overlapping roots shall resolve to one deduplicated item set, and the proposal shall visibly report included, excluded, unreadable, symlink, and out-of-scope items or counts before it may advance to later review. External text or URL drops shall not be treated as local files or directories. [UI-007, SAFE-009]
- Checkbox changes, directory expansion, and Drop discovery may enumerate and read metadata or content only within the active grant; by themselves they shall never move, rename, archive, or delete a source item. Failure, exclusion, unreadability, link detection, or scope denial shall remain visible and shall produce no filesystem mutation. [SAFE-009]
- File, profile, model, archive, knowledge, graph, cleanup, recovery, and permission states shall be visible enough for the user to distinguish proposals, confirmations, progress, failures, and committed outcomes. [ARCH-002, SAFE-004]

## Security and privacy

- File content, metadata, profiles, Markdown, graph data, permissions, and audits shall remain local by default; any network-dependent model or URL import shall be explicit and visibly fail without mutation when unavailable. [ARCH-001, SAFE-007]
- URL profile import shall accept only documented safe schemes, reject embedded credentials and loopback, private, link-local, local-file, or equivalent local targets, resolve and revalidate the target at connection time and every redirect, enforce bounded response size, duration, and accepted content, and shall not forward credentials across origins. [RULE-003, SAFE-007]
- Successful URL profile import shall record the fetched-byte SHA-256 and minimized provenance needed for review while redacting credentials and sensitive query or fragment values from persisted state, UI, diagnostics, and audit by default. [RULE-003, SAFE-004]
- Local rendering shall not execute document-supplied active content, and declarative profiles shall not execute imported instructions. [KNOW-004, RULE-001]
- Path authorization shall use resolved filesystem boundaries rather than string-prefix checks, and every filesystem operation shall be constrained to user-approved scope. [MCP-002, MCP-004]
- Critical operations shall require explicit user confirmation, and permanent deletion shall require its own separate confirmation. [SAFE-003, SAFE-004]
- Third-party code, assets, specifications, or embedded material shall be prohibited from MIT handoff until a separate license review records clearance. [SAFE-008]
- Source-tree selection and Drop discovery shall apply resolved active-grant boundaries during enumeration; unreadable, symlink, and out-of-scope entries shall be reported for review and excluded from proposed action without mutation. [SAFE-009]

## Recovery and audit

- Persistent lifecycle state shall allow interrupted work to resume safely or terminate visibly without an unregistered committed destination. [FILE-004, SAFE-004]
- Recovery shall recompute relevant readability, SHA-256, path, permission, confirmation, and original-preservation checks before commit or rollback. [FILE-002, FILE-004, SAFE-005]
- A safe archive commit shall survive knowledge-generation failure; recovery shall report failed derived work separately rather than roll back the original. [SAFE-006]
- Audit records for critical actions shall identify time, actor, action, selected scope, relevant content identities, decision and evidence, confirmation binding, invariant result, outcome, and failure reason when present, and unauthorized alteration or truncation shall be observably detectable during verification. The tamper-evidence mechanism is an implementation freedom. [SAFE-004]
- Audit storage, retention, and access mechanisms are implementation freedoms subject to preserving the required fields, recoverability, and observable tamper detection. [SAFE-004]
- Model or Agent failure, invalid input, permission denial, and unsafe undo shall be auditable and shall leave files unchanged unless a prior safe transaction was already committed. [AGENT-004, SAFE-007]

## Cross-platform requirements

- Windows, macOS, and Linux builds shall implement equivalent content identity, source preservation, transaction, naming, Vault, editing, graph, permission, and audit semantics. [ARCH-001, FILE-001, NAME-003]
- Platform trash integration may differ, but trash shall remain the default cleanup disposition and permanent deletion shall remain separately confirmed on every platform. [SAFE-003]
- Path normalization shall account for each platform's separators, case behavior, reserved names, Unicode handling, and link semantics while producing deterministic acceptance, rejection, and collision outcomes for identical logical inputs. [NAME-003, MCP-002]
- Network absence shall not disable the local-first baseline on any supported platform. [ARCH-001]

## Test requirements

- Automated tests shall verify every requirement ID in this specification through one or more deterministic vectors, with safety-critical paths including target-stack build metadata, SHA-256 mismatch, unreadable output, registration failure, crash reconciliation, stale or replayed confirmation, runtime path substitution, last-original refusal, separate permanent-delete confirmation, bounded undo, offline non-mutation, invalid or executable profile, bounded URL success and rejection, unapproved candidate, naming collision, missing evidence, model disagreement or failure, authenticated and replay-resistant MCP grants, archive-gated knowledge, isolated unsafe rendering, graph provenance, audit tamper detection, layout persistence, mixed file-and-directory tri-state selection, overlapping Drop deduplication, visible Drop boundary results with no mutation, and all three platforms. [ARCH-001, FILE-002, FILE-004, NAME-003, NAME-004, RULE-001, RULE-003, KNOW-001, KNOW-004, KNOW-005, AGENT-004, MCP-001, MCP-004, UI-002, UI-006, UI-007, SAFE-003, SAFE-004, SAFE-005, SAFE-007, SAFE-009]
- Test fixtures shall be original project fixtures or generated test data with recorded identities; blocked black-box cases shall not be asserted as observed behavior. [FILE-001, SAFE-007]
- Filesystem tests shall independently verify source and destination SHA-256 values and shall inspect externally visible files, registrations, state, and audits rather than internal algorithms. [FILE-002, FILE-004, SAFE-004]
- Cross-platform acceptance shall run against Windows, macOS, and Linux with deterministic logical inputs and platform-appropriate path and trash assertions. [ARCH-001, NAME-003, SAFE-003]

## First-release acceptance

First release shall be accepted only when the Tauri 2, React, TypeScript, and Rust application builds and launches on Windows, macOS, and Linux; the local-first baseline works offline; every matrix requirement has a passing deterministic test; and no safety-critical branch permits unapproved mutation or loss of the final valid original. [ARCH-001, FILE-002, AGENT-004, SAFE-005, SAFE-007]

The release shall demonstrate one continuous user-confirmed flow from scoped import through versioned classification, canonical naming, verified atomic archive, authoritative Markdown editing, and evidence-backed graph review in the single Vault. [ARCH-002, ARCH-003, FILE-004, NAME-001, RULE-001, KNOW-002, KNOW-005]

The release shall demonstrate the narrow toolbar and source tree, mixed file-and-directory tri-state selection with ancestor propagation, and a mixed local Drop that produces a deduplicated, reviewable discovery/import proposal with required boundary counts and no filesystem mutation before a separately reviewed operation. [UI-006, UI-007, SAFE-009]

The release shall demonstrate dual-model comparison with accountable Agent adjudication, deterministic validation, local stdio and loopback Streamable HTTP MCP, bounded expiring permissions, and no Agent-only cleanup execution. [AGENT-001, AGENT-002, AGENT-003, MCP-001, MCP-003, MCP-004]

The release shall pass asset and license provenance review, including authorized first-party inputs and separate clearance for every included third-party component or asset. [RULE-002, UI-001, SAFE-008]

## Explicit non-goals

- Automatic or Agent-only duplicate deletion is prohibited; first release provides user-controlled cleanup with deterministic safety checks. [SAFE-001, SAFE-002, MCP-003]
- Multiple simultaneously authoritative Vaults are prohibited. [ARCH-003]
- Derived Markdown, OCR, summaries, embeddings, previews, or graph data shall not replace or count as an original-format archive copy. [FILE-003]
- Executable classification profiles, silent candidate activation, fabricated classifications, and catch-all filesystem directories are prohibited. [RULE-001, RULE-003, RULE-004]
- A probabilistic third-model vote shall not replace Agent adjudication or deterministic safety validation. [AGENT-002, AGENT-003]
- Remote network exposure of MCP and non-loopback Streamable HTTP are prohibited. [MCP-001]
- Figma as a runtime dependency, copied third-party branding, and uncleared third-party material are prohibited. [UI-004, UI-005, SAFE-008]
