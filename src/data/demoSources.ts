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
  items: [
    {
      path: "/review/meeting-notes.md",
      name: "meeting-notes.md",
      byteSize: 18432,
    },
    {
      path: "/review/research-summary.txt",
      name: "research-summary.txt",
      byteSize: 9728,
    },
    {
      path: "/review/source-index.csv",
      name: "source-index.csv",
      byteSize: 6144,
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
