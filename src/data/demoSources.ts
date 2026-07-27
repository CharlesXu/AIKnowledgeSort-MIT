import type { DiscoveryProposal } from "../features/drop/types";
import type { SourceNode } from "../features/sources/types";

export const demoSources: SourceNode = {
  id: "workspace",
  name: "Local workspace",
  kind: "directory",
  eligible: false,
  children: [
    {
      id: "workspace/projects",
      name: "Projects",
      kind: "directory",
      eligible: false,
      children: [
        {
          id: "workspace/projects/Roadmap.md",
          name: "Roadmap.md",
          kind: "file",
          eligible: true,
          children: [],
        },
        {
          id: "workspace/projects/research",
          name: "Research",
          kind: "directory",
          eligible: false,
          children: [
            {
              id: "workspace/projects/research/synthesis.txt",
              name: "synthesis.txt",
              kind: "file",
              eligible: true,
              children: [],
            },
          ],
        },
      ],
    },
    {
      id: "workspace/notes",
      name: "Notes",
      kind: "directory",
      eligible: false,
      children: [
        {
          id: "workspace/notes/Field notes.md",
          name: "Field notes.md",
          kind: "file",
          eligible: true,
          children: [],
        },
        {
          id: "workspace/notes/drafts",
          name: "Drafts",
          kind: "directory",
          eligible: false,
          children: [
            {
              id: "workspace/notes/drafts/outline.md",
              name: "outline.md",
              kind: "file",
              eligible: true,
              children: [],
            },
          ],
        },
      ],
    },
    {
      id: "workspace/README.md",
      name: "README.md",
      kind: "file",
      eligible: true,
      children: [],
    },
    {
      id: "workspace/reference.zip",
      name: "reference.zip",
      kind: "file",
      eligible: false,
      children: [],
    },
  ],
};

export const demoDiscoveryProposal: DiscoveryProposal = {
  proposalId: "demo-proposal",
  items: [
    {
      itemId: "demo-meeting-notes",
      path: "/review/meeting-notes.md",
      name: "meeting-notes.md",
      byteSize: 18432,
      identity: {
        algorithm: "SHA-256",
        digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0",
      },
    },
    {
      itemId: "demo-research-summary",
      path: "/review/research-summary.txt",
      name: "research-summary.txt",
      byteSize: 9728,
      identity: {
        algorithm: "SHA-256",
        digest: "ab5f329afb80f567b441324ad2d048ca910644b17c7426f9cc585307c5077496",
      },
    },
    {
      itemId: "demo-source-index",
      path: "/review/source-index.csv",
      name: "source-index.csv",
      byteSize: 6144,
      identity: {
        algorithm: "SHA-256",
        digest: "09ad9f38f1197e357f7e0363947282ca8ebfb371131d2353752b8ed5ed16fba4",
      },
    },
  ],
  counts: {
    included: 3,
    excluded: 2,
    unreadable: 1,
    symlink: 1,
    outOfScope: 1,
  },
  diagnostics: [
    {
      category: "excluded",
      path: "/review/.DS_Store",
      message: "Excluded from this proposal",
    },
  ],
};
