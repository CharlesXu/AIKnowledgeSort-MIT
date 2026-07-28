# Authorized first-party inputs

The project owner authorized the MIT release project to reuse or rewrite:

- KLOrgFromFile classification specifications;
- KL-Man UI code, design tokens, and components.

Approved MIT copyright line:

```text
Copyright (c) 2026 Charles Xu and Segway-Ninebot
```

This record covers only material for which Charles Xu and Segway-Ninebot hold
the necessary rights. Third-party content embedded in either project requires
separate license review before handoff.

## Owner decision provenance — 2026-07-27

The following stable records capture owner decisions supplied directly for the
clean specification on 2026-07-27. The ranges identify the requirements
governed by each record.

| Decision record | Requirement range | Decision subject |
|---|---|---|
| OD-2026-07-27-ARCH | ARCH-001–ARCH-003 | Local-first cross-platform application, continuous workflow, and single authoritative Vault |
| OD-2026-07-27-FILE | FILE-001–FILE-005 | SHA-256 file identity, original-format preservation, verified atomic commit, and source preservation |
| OD-2026-07-27-NAME | NAME-001–NAME-004 | Evidence-based canonical naming, normalization, collision handling, and rename audit |
| OD-2026-07-27-RULE | RULE-001, RULE-003–RULE-005 | Declarative versioned profiles, draft/pilot status, candidate review, and conflict routing |
| OD-2026-07-27-KNOW | KNOW-001–KNOW-005 | Archive-gated Markdown knowledge, editing modes, Mermaid, and evidence-backed graph relations |
| OD-2026-07-27-AGENT | AGENT-001–AGENT-004 | Independent dual-model proposals, Agent adjudication, deterministic core, and failure safety |
| OD-2026-07-27-MCP | MCP-001–MCP-004 | Local transports, trusted-core enforcement, cleanup restriction, and bounded permissions |
| OD-2026-07-27-UI | UI-002–UI-005 | Interaction inspiration, three-pane workbench, graph controls, and design-tool boundary |
| OD-2026-07-27-SAFE | SAFE-001–SAFE-008 | User-controlled cleanup, deletion safeguards, lifecycle, undo, audit, and license review |
| D-012 | UI-006–UI-007, SAFE-009 | Source-tree selection and non-mutating local file and directory drop discovery |

`D-012` records the following textual interpretation of the owner's clean UI
decision:

- the user-owned KL-Man UI visual sample supplied at the clean project root is
  a visual reference only; it is not a release asset and must not be copied,
  embedded, or shipped;
- the workbench has a narrow left toolbar and an adjacent source tree;
- the source tree supports checkbox selection of both files and directories,
  including observable parent, child, and indeterminate states;
- the application accepts operating-system drag-and-drop batches containing
  local files and directories for scoped discovery and import review.

## Authorization provenance — 2026-07-27

- `AUTH-2026-07-27-CLASSIFICATION`: owner authorization to reuse or rewrite
  owned KLOrgFromFile classification specifications under MIT; applies to
  RULE-002. Embedded third-party material remains excluded pending separate
  review.
- `AUTH-2026-07-27-UI`: owner authorization to reuse or rewrite owned KL-Man
  UI code, design tokens, and components under MIT; applies to UI-001.
  Embedded third-party material remains excluded pending separate review.

## Authorization provenance — 2026-07-29

- `AUTH-2026-07-29-NINEBOT-DRAFT`: Charles Xu supplied and authorized reuse or
  clean-room rewriting under MIT of the owned Ninebot classification and
  knowledge-organization material listed in
  `classification/ninebot-draft-sources.json`; applies to RULE-002 and
  RULE-005. This authorization covers the classification tree, classification
  rules and dictionary, electronic-archive discussion draft, usage guidance,
  and cross-domain conflict analysis.
- This authorization does not convert a discussion draft into effective
  Segway-Ninebot policy and does not represent EMT approval. The bundled
  profile must remain visibly `draft`, inactive, and non-committable until a
  separately reviewed formal candidate is approved.
- Third-party material embedded in the supplied package remains excluded from
  the MIT handoff unless separately cleared. The implementation derives only
  the owned classification and governance data identified in the source
  manifest.
