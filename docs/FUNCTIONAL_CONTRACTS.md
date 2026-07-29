# Functional contracts

These contracts specify externally verifiable behavior and safety boundaries.
They do not prescribe internal architecture beyond the owner-approved
technology, transport, and trust boundaries. Existing black-box cases are
blocked and provide no confirmed behavioral claims; references to them in the
source matrix identify pending future confirmation only.

## Continuous two-stage workflow

**Purpose.** Provide one continuous local workflow that discovers a
user-selected scope, then organizes, classifies, canonically names, and
archives originals before creating knowledge artifacts only from confirmed
archived originals.

**Inputs.** Local files and directories selected through the source tree or an
operating-system drop; an active classification profile and rule snapshot; user
confirmations; readable file content and metadata available within the granted
scope.

**Externally observable outputs.** A scoped, deduplicated discovery/import
proposal showing included, excluded, unreadable, symlink, and
out-of-scope counts; after review and later confirmation, a confirmed archive
result with registered original identities, followed by Markdown knowledge and
evidence-backed graph results for eligible archived originals. Each result
visibly indicates its stage and status.

**User decisions.** The user selects scope, reviews classification and naming
proposals and all discovery exclusions or failures, confirms archive
operations, and chooses whether eligible archived items proceed to knowledge
creation.

**Safety invariants.** Knowledge creation cannot begin for an item until its
original-format archive copy is confirmed, SHA-256-verified, and registered.
Knowledge artifacts cannot become substitutes for an original. Selection and
drop discovery may enumerate and read local metadata or content within the
active grant, but cannot move, rename, archive, delete, or otherwise mutate any
filesystem item.

**Failure behavior.** A classification, naming, or archive failure leaves the
item out of the knowledge stage and reports a reviewable reason. A knowledge
failure leaves a previously safe archive intact and reports the failed derived
work separately. Directory expansion failures, exclusions, permission
failures, symlinks, and out-of-scope items remain visible and reviewable
in the proposal without triggering a file operation.

**Measurable acceptance tests.**

1. Given an item without a confirmed archive, requesting knowledge creation
   produces no Markdown or graph result and reports the archive prerequisite.
2. Given a confirmed archived original, the user can continue to knowledge
   creation without starting a separate import workflow.
3. If knowledge creation fails after archive confirmation, the registered
   archived original remains readable and its recorded SHA-256 digest still
   verifies.
4. Drop an overlapping file-and-directory batch and verify one deduplicated
   discovery/import proposal appears with included, excluded, unreadable,
   symlink, and out-of-scope counts, while all source paths and digests
   remain unchanged.
5. Drop external text and a URL and verify neither is treated as a local file
   or directory and no filesystem mutation occurs.

**Implementation freedoms.** Queueing, scheduling, progress representation,
drop-event mechanism, traversal mechanism, and stage orchestration are
unconstrained if the observable ordering and safety invariants hold.

**Requirement IDs.** ARCH-002, FILE-002, FILE-003, KNOW-001, UI-007, SAFE-006,
SAFE-009.

## Original-format preservation

**Purpose.** Preserve durable, readable evidence in its original format while
allowing safe organization and derived knowledge.

**Inputs.** Source bytes; original filename and location; destination scope;
the `SHA-256` algorithm identifier and its 64-hex-character digest;
registration metadata; proposed normalized name and path.

**Externally observable outputs.** At least one readable, SHA-256-verified,
registered original-format copy; an operation result that records both the
`SHA-256` algorithm identifier and digest as the stable content identity;
separate derived artifacts.

**User decisions.** The user approves the archive destination and any
filesystem-changing operation after reviewing the proposed result.

**Safety invariants.** A rename or relocation does not change file identity.
Markdown, OCR, summaries, vectors, and graph data never count as an
original-format copy. A file operation commits only after a temporary copy is
readable and its independently calculated SHA-256 digest matches the source.
No weaker or alternative digest satisfies identity or duplicate-safety
verification. The source remains preserved by default.

**Failure behavior.** A missing or invalid SHA-256 algorithm record, SHA-256
digest mismatch, unreadable output, interrupted copy, or registration failure
prevents commit and leaves the source unchanged. Partial temporary output is
not reported as archived.

**Measurable acceptance tests.**

1. Rename and relocate a test original without changing its bytes; its identity
   remains `SHA-256` plus the same 64-hex-character digest before and after
   registration.
2. Independently calculate source and destination SHA-256 digests, simulate a
   destination mismatch, and verify no committed archive result appears while
   the source digest remains unchanged.
3. Create every supported derived artifact, remove those derived artifacts,
   and verify the registered original remains readable and SHA-256-valid.
4. Present a matching value labeled with a weaker digest algorithm and verify
   it cannot establish file identity, archive validity, or duplicate
   eligibility.

**Implementation freedoms.** SHA-256 library choice, temporary-file naming,
registration storage, and atomic-commit mechanism are unconstrained if they
meet the measurable guarantees on supported platforms. The identity algorithm
is not an implementation freedom.

**Requirement IDs.** FILE-001, FILE-002, FILE-003, FILE-004, FILE-005.

## User-controlled duplicate cleanup

**Purpose.** Let the user remove redundant original-format copies without
risking the final valid original.

**Inputs.** A user-enabled cleanup request; selected scope; file identities;
registered locations; recorded `SHA-256` algorithm identifiers and digests;
fresh SHA-256 verification results; proposed disposition.

**Externally observable outputs.** An eligibility report and bounded cleanup
proposal; after explicit user confirmation, either a move to trash or a
separately confirmed permanent deletion; an audit result for each selected
copy.

**User decisions.** `清理原件` is off by default. The user enables it, chooses
individual eligible copies, confirms the exact scope, and separately confirms
permanent deletion when requested.

**Safety invariants.** Only confirmed archived duplicates with another valid
original-format copy are eligible. The Agent may suggest but cannot execute
cleanup or expand its scope. Trash is the default. The last readable,
SHA-256-verified original-format copy can never be deleted. Duplicate identity
requires matching SHA-256 digests with the algorithm recorded as `SHA-256`;
weaker or alternative digests do not establish eligibility.

**Failure behavior.** Missing verification, stale identity data, scope drift,
permission expiry, or an attempt to remove the last valid original rejects the
operation without deleting any selected item.

**Measurable acceptance tests.**

1. On first use, verify cleanup is disabled and no cleanup action occurs
   without the user enabling it.
2. Present one valid original-format copy and any number of derived artifacts;
   verify the original is ineligible for cleanup.
3. Present two registered original-format copies with independently verified
   matching SHA-256 digests; after selecting one and confirming, verify the
   selected copy moves to trash by default and the other remains readable and
   SHA-256-valid.
4. Request permanent deletion; verify a separate confirmation is required
   after the cleanup selection.
5. Present copies that match only under a weaker digest and verify neither is
   eligible for duplicate cleanup.

**Implementation freedoms.** Duplicate discovery strategy, proposal layout,
and platform trash integration are unconstrained if eligibility and
confirmation are externally testable.

**Requirement IDs.** SAFE-001, SAFE-002, SAFE-003, FILE-002, MCP-003.

## Configurable classification profiles

**Purpose.** Classify files through reviewable domain profiles that can evolve
without executing embedded instructions.

**Inputs.** A versioned declarative profile; file evidence; profile provenance;
optional candidate profile or diff imported from a local file or user-provided
URL.

**Externally observable outputs.** Classification proposals tied to an exact
profile version; dedicated review entries for conflicts; a reviewable,
parent-linked taxonomy with per-level coverage; declarative archive/knowledge
governance; candidate import records containing source, SHA-256 algorithm and
digest, taxonomy-and-rule diff, and approval evidence.

**User decisions.** The user selects a profile, approves or rejects profile
versions and candidate diffs, and resolves conflicts. The Ninebot profile
begins in draft/pilot status. AI may prepare a candidate but cannot declare it
approved.

**Safety invariants.** Profiles are declarative, non-executable, and versioned.
No pending, unclassified, or equivalent catch-all directory is created.
Conflicts are routed to dedicated review. Unapproved candidates cannot affect
committed classification. A source archive has one primary category; knowledge
derived after archive confirmation may link across domains, while generated
indexes link to authoritative bodies instead of copying them. Dictionary terms
may narrow model candidates but cannot establish formal placement without
semantic evidence.

**Failure behavior.** Invalid, executable, unverifiable, or unapproved profile
content is rejected without changing active rules or files. Ambiguous matches
produce review items instead of fabricated classification.

**Measurable acceptance tests.**

1. Load a valid draft profile and verify every proposal records its exact
   version while no archive change occurs before approval.
2. Attempt to import executable content as a profile and verify rejection with
   no active-profile change.
3. Import a candidate from each allowed source form and verify source,
   `SHA-256` algorithm identifier, digest, diff, and approval state are
   preserved while status remains unapproved.
4. Produce conflicting classification evidence and verify a dedicated review
   result appears without creation of a catch-all directory.
5. Load the bundled Ninebot discussion profile and verify `0.3.0-draft`, 466
   unique nodes, level counts 14/94/179/179, maximum depth four, zero executable
   rules, and no active profile.
6. Seed the immutable `0.1.0-draft` shell, inspect state, and verify its bytes
   remain unchanged while the complete `0.3.0-draft` is installed separately.
7. Import a candidate containing taxonomy-only changes and verify added,
   removed, and changed category identities are reviewable even when rule
   changes are empty.

**Implementation freedoms.** Declarative schema, matching technique, profile
storage, and review presentation are unconstrained within these boundaries.

**Requirement IDs.** RULE-001, RULE-002, RULE-003, RULE-004, RULE-005.

## Canonical naming

**Purpose.** Replace unreadable, numeric-only, or business-meaningless names
with deterministic, meaningful names grounded in available evidence.

**Inputs.** Original filename and extension; content identity; supported file
evidence; active naming rules; meaningful project, model, regulation, and
version tokens; destination namespace.

**Externally observable outputs.** A proposed canonical name; either a
collision-free committed name or a review item; an audit record containing the
original name and path, canonical name, applied rule, and the `SHA-256`
algorithm identifier and digest.

**User decisions.** The user reviews proposed names and resolves cases where
required facts are missing or evidence conflicts.

**Safety invariants.** Meaningful project, model, regulation, and version
tokens are preserved. The extension is unchanged. Unicode and cross-platform
normalization are applied consistently. Collisions are handled
deterministically. Missing facts are never fabricated.

**Failure behavior.** Insufficient evidence, unsafe characters, unsupported
normalization, or unresolved ambiguity routes the item to review without
renaming it.

**Measurable acceptance tests.**

1. Supply an unreadable or numeric-only name with sufficient evidence and
   verify a meaningful proposal that preserves the original extension.
2. Supply meaningful project, model, regulation, and version tokens and verify
   all remain present after normalization.
3. Generate the same collision twice from identical inputs and verify the same
   deterministic resolution on each supported platform.
4. Omit a required naming fact and verify review is requested, no invented fact
   appears, and no rename commits.

**Implementation freedoms.** Name templates, evidence ranking, normalization
library, and collision suffix format are unconstrained if results satisfy the
invariants and remain deterministic.

**Requirement IDs.** NAME-001, NAME-002, NAME-003, NAME-004, FILE-001.

## Dual-model comparison and Agent adjudication

**Purpose.** Compare independent semantic proposals while keeping one
accountable Agent decision and a deterministic safety boundary.

**Inputs.** The identical file identity, exact approved profile/policy snapshot,
and bounded extracted evidence for a desktop model and an Agent-side model;
both independently produced proposals; applicable permissions and safety
state. Knowledge-relation comparisons use the exact committed Markdown
revision and rule snapshot.

File identity consists of the recorded `SHA-256` algorithm identifier and
digest.

**Externally observable outputs.** Two distinguishable proposals and an Agent
decision of accept, revise, reject, or review, accompanied by reason and
evidence. A separate deterministic validation result reports whether an
approved operation is safe to execute.

**User decisions.** The user may inspect both proposals, review the Agent
decision, revise the requested scope, and confirm any resulting critical file
operation.

**Safety invariants.** Neither model receives hidden semantic advantages over
the other. The trusted deterministic core enforces permissions, paths,
SHA-256 identities, and original preservation. It does not act as a third
probabilistic semantic judge or replace Agent adjudication.

**Failure behavior.** Missing, inconsistent, timed-out, or invalid model output
produces a visible review or failure state and cannot trigger a filesystem
change. A failed deterministic safety check blocks execution regardless of the
semantic decision.

For file classification and naming, the desktop persists the validated
comparison as an immutable Vault record. Only `accept` and `revise` decisions
produce a resolved suggestion. A classification batch records the semantic
comparison ID separately from rule IDs; the two evidence modes cannot be mixed
or substituted for one another.

**Measurable acceptance tests.**

1. Capture both model requests and verify file identity, rule snapshot, and
   evidence are identical while outputs are produced independently.
2. For each allowed Agent decision, verify a reason and evidence reference are
   required before the decision is complete.
3. Simulate one model timeout and verify no file mutation occurs.
4. Submit a semantically accepted proposal that violates the last-original
   invariant and verify the deterministic core blocks it without generating a
   replacement semantic vote.

**Implementation freedoms.** Model providers, prompt form, comparison layout,
and adjudication workflow are unconstrained if independence, accountability,
and the trust boundary remain testable.

**Requirement IDs.** AGENT-001, AGENT-002, AGENT-003, AGENT-004, SAFE-007.

## Local MCP boundary

**Purpose.** Expose bounded local capabilities to Agents without creating a
path around the trusted deterministic core.

**Inputs.** A local stdio connection or loopback-only Streamable HTTP
connection; Agent identity; directory and tool grants; expiry; resource limits;
requested read, analysis, planning, semantic approval, or safe operation.

**Externally observable outputs.** Authorized local results, explicit denial
reasons, expiring permission state, and auditable requests for critical
actions.

**User decisions.** The user grants or revokes access by Agent, directory,
tool, expiry, and resource limit, and confirms critical filesystem actions.

**Safety invariants.** Network transport is loopback-only. Agents may read,
analyze, plan, and semantically approve within grants but cannot bypass trusted
permission, path, SHA-256 identity, or original-preservation enforcement. No
Agent-only duplicate-cleanup execution tool exists.

**Failure behavior.** Non-loopback connection attempts, expired or excessive
grants, out-of-scope paths, unsupported tools, and core validation failures are
denied without filesystem mutation.

**Measurable acceptance tests.**

1. Connect by stdio and loopback HTTP and verify equivalent bounded
   authorization; attempt a non-loopback connection and verify denial.
2. Exercise each permission dimension independently and verify an
   out-of-scope Agent, directory, tool, time, or resource request is denied.
3. Enumerate Agent-facing capabilities and verify no direct duplicate-cleanup
   execution capability is present.
4. Attempt to execute a semantically approved but SHA-256-invalid operation and
   verify the core blocks it.

**Implementation freedoms.** Protocol library, grant representation,
authentication mechanism, and audit storage are unconstrained within the
local-only and trusted-core boundaries.

**Requirement IDs.** MCP-001, MCP-002, MCP-003, MCP-004, AGENT-003.

## Single Vault and knowledge graph

**Purpose.** Maintain one authoritative local knowledge workspace whose graph
claims remain traceable to archived originals and evidence.

**Inputs.** Confirmed archived originals; content identities; approved
knowledge-generation requests; Markdown documents; relationship claims and
their evidence references.

**Externally observable outputs.** Markdown knowledge in one authoritative
Vault and a graph in which each relation exposes its supporting source or
evidence reference.

**User decisions.** The user selects eligible archived originals for knowledge
creation, reviews generated knowledge, and accepts, revises, or rejects graph
relations.

**Safety invariants.** There is exactly one authoritative Vault. Markdown is
the authoritative knowledge format. Knowledge and graph data are derived and
cannot replace originals. Relations without traceable archived evidence cannot
be confirmed.

**Failure behavior.** Missing archive confirmation, ambiguous Vault authority,
or absent relation evidence blocks the affected knowledge or relation result
and leaves confirmed archives unchanged.

**Measurable acceptance tests.**

1. Attempt to configure two authoritative Vaults and verify the second cannot
   become authoritative without an explicit authority transfer.
2. Create knowledge from a confirmed archived original and verify its Markdown
   and every confirmed graph relation trace to the original identity or cited
   evidence.
3. Submit a relation without evidence and verify it remains unconfirmed or is
   routed to review.

**Implementation freedoms.** Vault indexing, graph storage, relation taxonomy,
and visualization technique are unconstrained if Markdown authority and
evidence traceability are preserved.

**Requirement IDs.** ARCH-003, KNOW-001, KNOW-002, KNOW-005, FILE-003.

## Markdown and Mermaid editing

**Purpose.** Provide safe local authoring and review of authoritative Markdown,
including mixed Markdown and Mermaid content.

**Inputs.** Vault Markdown with optional frontmatter, tables, tasks, footnotes,
callouts, wiki links, block references, and Mermaid blocks; user edits; local
render settings.

**Externally observable outputs.** Source, live preview, and reading modes;
rendered supported Markdown and diagrams; actionable diagnostics that identify
invalid content without corrupting the source.

**User decisions.** The user chooses the viewing mode, edits source, resolves
diagnostics, and decides whether generated content is accepted into
authoritative Markdown.

**Safety invariants.** Rendering is local and treats document content as data,
not trusted executable instructions. Invalid Mermaid or Markdown does not
destroy source text. The Markdown source remains authoritative over rendered
views.

**Failure behavior.** Parse or render failures preserve source, display
diagnostics near the affected content, and keep unaffected content readable.
Unsafe local render content is blocked or escaped.

**Measurable acceptance tests.**

1. Open one document in source, live preview, and reading modes and verify edits
   persist through the same authoritative Markdown source.
2. Render examples covering GFM, frontmatter, tables, tasks, footnotes,
   callouts, wiki links, and block references and verify their documented local
   representations.
3. Edit Markdown and Mermaid in one document and verify valid mixed content
   renders while an invalid diagram produces a diagnostic without source loss.
4. Provide active or unsafe embedded content and verify it cannot execute
   through local rendering.

**Implementation freedoms.** Editor component, parser, preview strategy,
diagnostic presentation, and safe-render isolation are unconstrained if the
observable modes and safety guarantees hold.

**Requirement IDs.** KNOW-002, KNOW-003, KNOW-004.

## Obsidian-inspired workbench

**Purpose.** Provide a local-first desktop workbench on Windows, macOS, and
Linux with a narrow left toolbar and adjacent source tree for selecting files
and directories, plus knowledge and graph review, without copying third-party
branding or proprietary assets.

**Inputs.** Local workspace state; pane contents; user resize, collapse, split,
and playback actions; persisted layout preferences; discovered file and
directory hierarchy; checkbox changes; operating-system drops; active access
grant; current network availability.

**Externally observable outputs.** A Tauri 2 desktop application using React,
TypeScript, and Rust with supported desktop builds for Windows, macOS, and
Linux; an adjustable, collapsible, splittable three-pane workbench whose layout
persists; a narrow left toolbar and source tree; a graph playback bar
approximately 32–36 px high. Every displayed file and directory is individually
selectable. Selecting a directory applies to all eligible discovered
descendants; a directory with only some eligible descendants selected displays
an indeterminate state; changing a child updates every displayed ancestor.
Mixed file-and-directory selection remains explicit and deduplicated.
Dropping a mixed local batch creates a scoped discovery/import proposal,
deduplicates overlapping roots, and reports included, excluded, unreadable,
symlink, and out-of-scope counts.

**User decisions.** The user controls pane sizes, visibility, splits, restored
layout, graph playback, each file or directory checkbox, and whether a
drop-created proposal advances to later review. The user reviews exclusions,
permission failures, symlinks, and scope boundaries before confirming
any later filesystem-changing operation. Figma may be used to create design
artifacts but is not an application runtime dependency.

**Safety invariants.** Inspiration is limited to interaction patterns. No
copied brand assets or proprietary visual assets are included. Authorized
first-party UI material is used only within the recorded ownership scope, and
embedded third-party material requires separate review. The owner-supplied
visual sample is reference-only and is not copied, embedded, or shipped.
Checkbox selection, directory expansion, and drop discovery are non-mutating.
They may discover or read within the active grant but cannot move, rename,
archive, or delete an item. External text and URL drops are not treated as
local files.

**Failure behavior.** Invalid or unavailable persisted layout data falls back
to a usable three-pane layout without losing workspace content. When the
network is unavailable, network-dependent model or remote profile-import
operations fail visibly and produce no filesystem mutation; local archive
operations, Vault browsing, Markdown editing, graph browsing, and deterministic
safety operations remain available. Unreviewed third-party material is
excluded from handoff. Directory expansion errors, exclusions, permission
failures, symlinks, and out-of-scope items are counted, identified for
review, and excluded from action rather than silently accepted.

**Measurable acceptance tests.**

1. Build and launch on Windows, macOS, and Linux and verify the approved
   application technology target on each platform.
2. Resize, collapse, split, and restore all three panes; relaunch and verify the
   last valid layout returns.
3. Measure the graph playback bar across supported display scales and verify
   its logical height remains within 32–36 px.
4. Run asset provenance review and verify every shipped non-original asset is
   either authorized first-party material or separately cleared.
5. Disconnect network access and verify local archive operations, Vault
   browsing, Markdown editing, graph browsing, and deterministic permission,
   path, SHA-256, and original-preservation checks remain available.
6. While offline, request a network-dependent model operation and a
   user-URL profile import; verify each fails visibly and neither causes a
   filesystem mutation.
7. Select a directory, deselect one eligible child, and reselect it; verify the
   directory transitions from selected to indeterminate and back to selected,
   and that child changes update every displayed ancestor.
8. Select overlapping directories and files and verify the explicit mixed
   selection resolves to one deduplicated set of eligible discovered items.
9. Drop one batch containing local files, directories, overlapping roots,
   unreadable items, symlinks, and out-of-scope items; verify a proposal
   reports all required counts and no source is moved, renamed, archived,
   deleted, or modified.
10. Drop external text and a URL; verify neither enters the local-item proposal.
11. Inspect release assets and verify the owner-supplied visual sample is not
   copied, embedded, or shipped.

**Implementation freedoms.** Visual styling, component composition,
interaction details, tree virtualization, checkbox-state calculation
mechanism, operating-system drop integration, discovery traversal, and
persistence format are unconstrained subject to the stated target and safety
boundaries. Windows, macOS, and Linux support, the observable selection
semantics, non-mutating discovery, and offline behavior are not implementation
freedoms.

**Requirement IDs.** ARCH-001, UI-001, UI-002, UI-003, UI-004, UI-005, UI-006,
UI-007, SAFE-008, SAFE-009.

## Recovery, undo, and audit

**Purpose.** Make critical operations recoverable and attributable while
preserving originals across interruption and failure.

**Inputs.** Persistent operation state; source and destination identities and
recorded `SHA-256` algorithm identifiers and digests; temporary-copy status;
user confirmations; permission state; audit context; undo request.

**Externally observable outputs.** Resumable or safely failed operation states;
atomic committed results; bounded undo results; and audit records for critical
actions, decisions, confirmations, failures, and invariant checks.

**User decisions.** The user confirms critical operations, chooses whether to
resume or abandon recoverable work, requests undo, and separately confirms any
permanent deletion.

**Safety invariants.** Copy uses temporary output, SHA-256 verification, and
atomic commit. Source preservation is the default. Undo cannot violate the
last-valid-original rule. Model or Agent failure cannot trigger unapproved
filesystem changes. A knowledge-stage failure cannot roll back a safe archive.

**Failure behavior.** On interruption, the persistent lifecycle exposes a
recoverable state or a safe terminal failure. Missing or mismatched SHA-256,
permission, confirmation, or invariant failure blocks commit or undo,
preserves valid originals, and records the reason.

**Measurable acceptance tests.**

1. Interrupt an operation before and after temporary-copy verification; restart
   and verify it resumes safely or fails without an unregistered committed
   destination.
2. Attempt undo when it would remove the last valid original-format copy and
   verify rejection with all valid originals intact.
3. Simulate model and Agent failures before confirmation and verify no
   filesystem mutation.
4. Fail knowledge creation after a safe archive commit and verify the archive
   remains registered, readable, and SHA-256-valid.
5. For each critical action, verify the audit record identifies the action,
   scope, decision, confirmation, outcome, and relevant identities without
   relying on derived artifacts as originals.

**Implementation freedoms.** Lifecycle state representation, recovery journal,
undo mechanism, audit encoding, and retention policy are unconstrained if
critical actions remain recoverable, bounded, and testable.

**Requirement IDs.** FILE-004, FILE-005, SAFE-004, SAFE-005, SAFE-006, SAFE-007,
AGENT-004.
