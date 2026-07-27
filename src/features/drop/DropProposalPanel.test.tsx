import { render, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import type { DiscoveryProposal } from "./types";
import { DropProposalPanel } from "./DropProposalPanel";

const proposal: DiscoveryProposal = {
  items: [
    { path: "/demo/guide.md", name: "guide.md", byteSize: 8400 },
    { path: "/demo/notes.txt", name: "notes.txt", byteSize: 2100 },
  ],
  counts: {
    included: 2,
    excluded: 3,
    unreadable: 1,
    symlink: 4,
    outOfScope: 5,
  },
  diagnostics: [],
};

describe("DropProposalPanel", () => {
  test("shows all five discovery counts and the non-mutating review state", () => {
    render(<DropProposalPanel proposal={proposal} />);

    const panel = screen.getByRole("region", {
      name: "Discovery proposal",
    });
    const expectations = {
      Included: "2",
      Excluded: "3",
      Unreadable: "1",
      Symlinks: "4",
      "Out of scope": "5",
    };

    for (const [label, value] of Object.entries(expectations)) {
      expect(
        within(panel).getByRole("status", { name: label }),
      ).toHaveTextContent(value);
    }

    expect(within(panel).getByText("Review only")).toBeInTheDocument();
    expect(
      within(panel).getByText("No files have been changed"),
    ).toBeInTheDocument();
  });

  test("keeps a passive drop surface visible beneath the proposal table", () => {
    render(<DropProposalPanel proposal={proposal} />);

    const surface = screen.getByRole("region", {
      name: "Passive drop surface",
    });
    expect(surface).toHaveTextContent(
      "Drop files or folders anywhere in this window",
    );
    expect(surface).toHaveTextContent(
      "A trusted, review-only proposal will appear here",
    );
    expect(within(surface).queryByRole("button")).toBeNull();
  });
});
