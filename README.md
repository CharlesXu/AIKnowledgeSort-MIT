# AI Knowledge Sort

AI Knowledge Sort is an independently implemented, local-first file archival
and knowledge workspace.

This new-history repository contains only a sanitized, implementation-clean
specification handoff. Its test vectors are self-contained and require no
external fixture, source checkout, source index, or specification-room path.

The repository now contains the first independently implemented desktop
milestones described below.

License: MIT

## Drop discovery platform scope

Local drop discovery opens operating-system drop roots once in Rust and then
walks only capability-relative, no-follow handles. Unix tests cover symlink
replacement during traversal and replacement of a dropped file name after its
handle is issued. Windows builds additionally reject symlink-file and
symlink-directory reparse types, with a platform-gated test when the test
account may create links. Full Windows junction and non-symlink reparse-point
acceptance remains a Windows CI and manual acceptance item; it is not claimed
by the current macOS verification.

Drop-root issuance and discovery run on a bounded blocking worker pool rather
than the Tauri window-event thread. Each request has a visible deadline and
cooperative traversal checks. An individual operating-system metadata call on a
hostile or stalled network filesystem cannot be forcibly cancelled; timed-out
workers retain their bounded concurrency permit until that call returns, so the
UI thread remains responsive and additional work is rejected at the configured
limit.

## Declarative classification profiles

The desktop runtime can import a local file or an explicitly entered HTTPS URL
as an unapproved declarative JSON classification-profile candidate. The trusted
Rust boundary rejects unknown and executable-shaped fields, limits input to
1 MiB, preserves the exact source bytes by SHA-256 in the authorized Vault, and
records candidate provenance without persisting a local absolute path or a
remote URL.

Remote import validates every resolved address as public and pins those
addresses for the connection. It disables ambient proxies and automatic
redirects, manually revalidates at most five redirects, and permanently strips
credentials after a cross-origin hop. Requests have a 5-second connection
timeout, a 15-second total timeout, a 1 MiB streaming limit, and accept only
`application/json` or `application/*+json`. Diagnostics are bounded constants;
query and fragment values are cleared from the UI and excluded from persisted
provenance.

A successful remote fetch records only the fetched-byte SHA-256, bounded byte
size, `remoteUrl` source kind, safe basename, and the SHA-256 of the final
locator after query and fragment removal. The same diff and exact-digest review
gate used by local import remains mandatory. Remote import never approves or
activates a profile, and a network or validation failure leaves Vault state
unchanged. Local discovery, archive, Markdown, graph, and local-profile
operations remain usable without network access.

Approval and rejection require the exact candidate digest and create immutable
decision records. An approved version can produce exact-version classification
proposals; missing or conflicting semantic evidence is routed to
`classificationReview` without inventing a destination. Profile operations in
the browser preview fail visibly instead of simulating persistence or
activation.

Profile schema version 2 adds a bounded parent-linked taxonomy and declarative
governance policy while preserving schema version 1 candidate compatibility.
Candidate diffs separately report added, removed, and changed taxonomy nodes
and executable rules, so a future formal notice or draft cannot appear
unchanged merely because it contains no literal rules.

The bundled Ninebot profile is `0.3.0-draft`. It contains the complete
owner-authorized discussion taxonomy: 14 L1, 94 L2, 179 L3, and 179 L4 nodes
(466 total). `SN-02 IPMS 集成营销服` is canonical and the usage manual's
`SN-02 IPMS 管理营销闭环` wording is retained as an alias. Its governance requires
semantic evidence, one primary archive category, dedicated review for
conflicting evidence, `importantIndexed` for insufficient evidence,
archive-first processing, cross-domain knowledge links, selected independent
knowledge nodes, and link-only generated indexes.

The bundled discussion profile has zero executable rules and remains visibly
draft, inactive, and non-committable. Dictionary terms are candidate-recall
vocabulary, not deterministic keyword placement rules. Model-assisted
conversion of future notices and drafts, the governed semantic classification
adapter, and activation of any unapproved profile are not implemented or
claimed. The supplied discussion material is not represented as EMT-approved
or effective company policy.

## Canonical naming and source-preserving archive

The desktop runtime now accepts bounded, cited naming facts for Project, Model,
Regulation, Version, and Subject. A code-owned Rust policy normalizes those
facts to Unicode NFC, preserves the original extension, checks the actual
authorized Vault namespace for case-insensitive collisions, and either returns
a deterministic canonical filename or routes the item to `namingReview`.
Missing or conflicting evidence never produces an invented name.

Naming proposals are held in bounded, five-minute, single-use batches bound to
the exact reviewed discovery item and SHA-256 identity. Archive planning cannot
accept a filename from the frontend: it must consume that naming batch, and any
review outcome blocks the plan. A confirmed transaction copies the verified
source into `Originals/<sha256>/<canonical-name>` without renaming or deleting
the source. Its operation journal and original registration record the original
and canonical names, paths, policy/version, applied rule, cited facts, evidence
locations, confirmation binding, identity, and outcome. Existing v1 archive
journals remain readable during recovery.

The compact local-evidence and canonical-name review lives in the archive side
pane; it does not replace the Markdown, Mermaid, and code workspace. Browser
preview clients fail visibly and never simulate naming, Vault selection, archive
planning, or archive commits.

## User-controlled source cleanup

Cleanup remains off by default and appears only for successfully archived
sources. Enabling it creates a five-minute, single-use plan bound to the exact
source path, retained Vault path, archive operation, authoritative Vault,
disposition, and SHA-256 identity. Confirmation independently reopens and
rehashes both the source and the registered retained original before changing
the source.

The default disposition uses the operating-system Trash or Recycle Bin.
Permanent deletion cannot reuse that confirmation: requesting it consumes the
reviewed trash plan and creates a new plan, nonce, audit lifecycle, warning, and
second confirmation. Agent tools have no cleanup execution capability. A
changed, unreadable, missing, linked, or no-longer-registered retained original
rejects cleanup without source mutation.

## Archive-gated authoritative Markdown

Each successfully committed archive item can now become an explicit knowledge
target in the central editor. The trusted Rust boundary accepts only the opaque
Vault authority and archive operation identities, independently re-verifies the
committed registration and archived original SHA-256, and then opens a
deterministic starter note or the latest committed revision. Unconfirmed,
missing, replaced, or byte-changed originals cannot create knowledge.

Authoritative UTF-8 Markdown is stored as immutable, append-only revisions under
the single Vault. Each revision has a separate immutable commit record binding
the Markdown SHA-256 to the archived-original SHA-256. Optimistic revision
checks reject stale saves without discarding the editor text, and an
uncommitted Markdown orphan can be safely retried without affecting the archive.
The browser preview retains a local unsaved draft and never claims a Vault save.

## Evidence-backed knowledge graph

An authoritative saved Markdown revision can now produce manually proposed
source–relation–target claims in the right pane. The frontend submits only node
fields and committed Markdown line ranges. Rust reopens that exact immutable
revision, extracts the evidence text itself, binds it to both Markdown and
archived-original SHA-256 identities, and stores append-only relation versions
inside the authorized Vault.

Each relation starts in review. Accept, revise, and reject require the exact
latest version plus a reason; accepted and rejected versions are terminal and
cannot be replayed. The two-dimensional graph, accessible relation list,
evidence inspector, and compact 34 px timeline are projections of persisted
relation events. The browser preview keeps showing proposal topology and cannot
simulate relation persistence or acceptance.

## Configurable model comparison and Agent adjudication

The desktop runtime can persist up to 32 secret-free local or OpenAI-compatible
model configurations. Local endpoints must be literal loopback HTTP addresses;
remote endpoints must use HTTPS and reject literal loopback/private addresses,
embedded credentials, queries, fragments, and redirects. Configuration stores
only endpoint, model, timeout, location, and authentication metadata. An
authenticated configuration derives an `AIKS_MODEL_API_KEY_<CONFIG_ID>`
environment-variable reference; the UI never accepts, reads, displays, or
persists the credential value.

Agent Review operates only on an authoritative saved Markdown revision. Rust
reopens that exact revision, re-verifies both Markdown and archived-original
SHA-256 identities, extracts the selected lines itself, and creates one bounded
`knowledge-relations-v1` envelope. Two distinct stored configurations receive
byte-identical envelope JSON on independent blocking workers. Only after both
strict, evidence-bound proposals succeed does the Agent configuration receive
the same envelope plus both recorded proposals for adjudication.

The OpenAI-compatible transport disables redirects and proxy inheritance, uses
bounded connection/total timeouts, caps responses at 256 KiB, and strictly
validates proposal and adjudication JSON. Provider timeouts, non-success
responses, malformed output, unknown semantic fields, invalid evidence IDs, and
Agent adjudication failures become visible `review` or `failed` outcomes. Each
valid comparison attempt is stored as one immutable Vault record; this workflow
does not call archive, naming, knowledge-save, graph-write, cleanup, move,
rename, or deletion operations. Settings and the right-pane Agent Review are
available only as honest native boundaries; the browser preview rejects all
model operations and fabricates no configurations, proposals, or decisions.

## Governed Agent access kernel

Settings now includes an Agent access panel backed by a trusted Rust permission
kernel. Directory authority can originate only from the native multiple-folder
picker: Rust opens each selected directory as a no-follow capability and gives
the frontend only an opaque, five-minute, single-use selection identity plus
display labels. Grant creation consumes that selection and persists at most 32
bounded records with Agent identity, opaque scope identities, an exact safe-tool
set, expiry, revocation state, and request/input/output limits. It never accepts
frontend-supplied paths.

The code-owned `agent-tools-v1` catalog contains only capability/knowledge/graph
reads and semantic comparison, classification, or cleanup suggestions. It has
no cleanup execution, archive commit, move, rename, delete, arbitrary command,
or ambient filesystem capability. Restarted grants remain visible for audit but
become inactive; persisted display paths are never used to silently reopen
filesystem authority.

Grant and session bearer tokens use 32 bytes of operating-system randomness and
are returned only at issuance. The runtime retains or persists only SHA-256
verifiers. Each Agent request must pass one Rust authorization boundary that
rechecks the exact Agent, grant, active session, bearer token, tool, optional
scope, expiry, revocation, single-use request identity, request count, input
size, and response budget before yielding a cloned directory capability.
Authorized session/request events and desktop grant changes create immutable,
token-free local audit records. The browser preview rejects every Agent-access
operation and never fabricates a grant or token.

## Governed local MCP transports

The desktop can explicitly start and stop one stateful MCP Streamable HTTP
broker at `http://127.0.0.1:<assigned-port>/mcp`. It binds only the literal IPv4
loopback interface, validates the exact bound `Host`, rejects `OPTIONS`, applies
a 1 MiB global body limit plus the grant request limit, emits no permissive CORS
policy, and requires all of these headers on every direct request:

- `Authorization: Bearer <one-time-grant-token>`
- `X-AIKS-Agent-Id: <agent-id>`
- `X-AIKS-Grant-Id: <grant-id>`
- `Mcp-Session-Id: <broker-issued-session>` after initialization

Non-browser clients may omit `Origin`. If an `Origin` header is present, it must
be one of at most eight canonical literal-loopback HTTP origins explicitly
stored in the grant and remain identical for the authenticated session. DNS
names such as `localhost`, wildcard/null origins, credentials, paths, queries,
fragments, implicit ports, HTTPS, LAN addresses, and origin drift are rejected.

The same application executable supports a standard line-delimited stdio relay:

```text
ai-knowledge-sort --mcp-stdio-relay --broker-url http://127.0.0.1:<port>/mcp
```

The relay requires `AIKS_MCP_AGENT_ID`, `AIKS_MCP_GRANT_ID`, and
`AIKS_MCP_GRANT_TOKEN` in its environment. It accepts no token CLI argument,
disables proxy inheritance and redirects, bounds request/response frames, and
holds only HTTP/session state. It never opens a scope or creates an alternate
permission authority. Settings shows reviewable direct-HTTP and stdio templates
only while the one-time token remains in component memory; dismissing the token
removes them, and no third-party runtime configuration is modified automatically.

The official Rust MCP SDK owns protocol negotiation, JSON-RPC framing, SSE, and
stateful session routing. Every tool call still crosses the same Rust grant,
session, scope, replay, expiry, revocation, request-count, and byte-budget
authorization boundary. `capabilities.read`, bounded no-follow `knowledge.read`,
parsed bounded `graph.read`, and exact-SHA-256 review-only `cleanup.suggest` are
implemented. `comparison.run` and `classification.propose` return a structured
`notReady` result until dedicated governed adapters exist; no semantic output is
fabricated. There is no MCP move, rename, delete, archive commit, cleanup
execution, arbitrary command, ambient path, or automatic grant reactivation.

Secure keychain credential entry, model discovery, provider-specific APIs,
applying model suggestions to graph relations, model-generated naming facts,
automatic classification or naming, physical source renaming, user-controlled
cleanup undo and crash reconciliation, classified destination paths, automatic graph inference,
model-generated knowledge, automatic grant reactivation, write-capable MCP
tools, third-party runtime installation/configuration, GraphRAG
indexing/retrieval, a 3D graph, secure keychain integration, and URL profile
import through MCP remain unimplemented. Explicit review-only HTTPS profile
import in the desktop UI is implemented as described above.
Those future integrations must use the same cited-fact, single-use batch, exact
identity, explicit archive-confirmation, and Agent-grant boundaries.
