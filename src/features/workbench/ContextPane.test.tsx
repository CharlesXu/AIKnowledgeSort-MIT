import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { DiscoveryProposal } from "../drop/types";
import { ContextPane } from "./ContextPane";

const proposal: DiscoveryProposal = {
  items: [{ path: "/review/guide.md", name: "guide.md", byteSize: 1024 }],
  counts: {
    included: 2,
    excluded: 3,
    unreadable: 1,
    symlink: 4,
    outOfScope: 5,
  },
  diagnostics: [],
};

describe("ContextPane", () => {
  test("shows five compact proposal statuses and no-mutation evidence", () => {
    render(
      <ContextPane
        collapsed={false}
        isDemo
        onCollapsedChange={vi.fn()}
        proposal={proposal}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: "Import Review" }));
    const statuses = screen.getByRole("list", {
      name: "Proposal status counts",
    });
    const expected = {
      Included: "2",
      Excluded: "3",
      Unreadable: "1",
      Symlink: "4",
      "Out of scope": "5",
    };

    for (const [label, count] of Object.entries(expected)) {
      expect(
        within(statuses).getByRole("listitem", { name: label }),
      ).toHaveTextContent(count);
    }

    expect(screen.getByText("Mutation").nextElementSibling).toHaveTextContent(
      "None",
    );
  });
});
