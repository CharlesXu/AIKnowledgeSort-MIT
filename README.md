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
or become active. The full Ninebot taxonomy, URL import, model-assisted
conversion of future notices and drafts, canonical file naming, and application
of classified paths to archive transactions are not yet implemented or
claimed.
