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

The desktop runtime can import a local, declarative JSON classification profile
as an unapproved candidate. The trusted Rust boundary rejects unknown and
executable-shaped fields, limits input to 1 MiB, preserves the exact source
bytes by SHA-256 in the authorized Vault, and records candidate provenance
without persisting the source's absolute path.

Approval and rejection require the exact candidate digest and create immutable
decision records. An approved version can produce exact-version classification
proposals; missing or conflicting semantic evidence is routed to
`classificationReview` without inventing a destination. Profile operations in
the browser preview fail visibly instead of simulating persistence or
activation.

The bundled `Ninebot electronic archive` entry is intentionally a zero-rule
draft shell. It documents the clean implementation boundary but cannot classify
or become active. The full Ninebot taxonomy, URL import, and model-assisted
conversion of future notices and drafts are not yet implemented or claimed.

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

Secure keychain credential entry, model discovery, provider-specific APIs,
applying model suggestions to graph relations, model-generated naming facts,
automatic classification or naming, physical source renaming, user-controlled
original cleanup, classified destination paths, automatic graph inference,
model-generated knowledge, MCP stdio/Streamable HTTP transports, MCP JSON-RPC
dispatch, automatic grant reactivation, external Agent-runtime smoke tests,
GraphRAG indexing/retrieval, a 3D graph, secure keychain integration, and URL
profile import remain unimplemented.
Those future integrations must use the same cited-fact, single-use batch, exact
identity, explicit archive-confirmation, and Agent-grant boundaries.
