import { describe, expect, test } from "vitest";
import type { DiscoveryProposal } from "../drop/types";
import { sourceTreeFromProposal } from "./sourceTreeFromProposal";

const proposal: DiscoveryProposal = {
  proposalId: "trusted-proposal",
  items: [
    {
      itemId: "meeting",
      path: "/Users/example/Drop/Projects/meeting.md",
      name: "meeting.md",
      byteSize: 12,
      identity: {
        algorithm: "SHA-256",
        digest: "0".repeat(64),
      },
    },
    {
      itemId: "report",
      path: "/Users/example/Drop/Projects/Reports/report.pdf",
      name: "report.pdf",
      byteSize: 34,
      identity: {
        algorithm: "SHA-256",
        digest: "1".repeat(64),
      },
    },
  ],
  counts: {
    included: 2,
    excluded: 0,
    unreadable: 0,
    symlink: 0,
    outOfScope: 0,
  },
  diagnostics: [],
};

describe("source tree from a discovery proposal", () => {
  test("retains the discovered hierarchy and exact item identities", () => {
    expect(sourceTreeFromProposal(proposal)).toEqual({
      id: "proposal:trusted-proposal",
      name: "Projects",
      kind: "directory",
      eligible: false,
      children: [
        {
          id: "meeting",
          name: "meeting.md",
          kind: "file",
          eligible: true,
          children: [],
        },
        {
          id: "directory:trusted-proposal:Reports",
          name: "Reports",
          kind: "directory",
          eligible: false,
          children: [
            {
              id: "report",
              name: "report.pdf",
              kind: "file",
              eligible: true,
              children: [],
            },
          ],
        },
      ],
    });
  });

  test("normalizes Windows separators without changing file item ids", () => {
    const windows = {
      ...proposal,
      proposalId: "windows-proposal",
      items: [
        {
          ...proposal.items[0],
          itemId: "windows-item",
          path: String.raw`C:\Drop\Specs\readme.md`,
          name: "readme.md",
        },
      ],
    };

    expect(sourceTreeFromProposal(windows)).toMatchObject({
      id: "proposal:windows-proposal",
      name: "Specs",
      children: [{ id: "windows-item", name: "readme.md" }],
    });
  });
});
