# AI Knowledge Sort

AI Knowledge Sort is an independently implemented, local-first file archival
and knowledge workspace.

This new-history repository contains only a sanitized, implementation-clean
specification handoff. Its test vectors are self-contained and require no
external fixture, source checkout, source index, or specification-room path.

No product implementation code is included in this initial commit.

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
