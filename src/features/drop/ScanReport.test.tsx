import { render, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import type { DiscoveryProposal } from "./types";
import { ScanReport } from "./ScanReport";

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

describe("ScanReport", () => {
  test("shows all five discovery counts and the non-mutating review state", () => {
    render(
      <ScanReport
        isDemo
        proposal={proposal}
        status="idle"
        statusMessage="Browser fixture ready."
      />,
    );

    const panel = screen.getByRole("region", {
      name: "Scan report",
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

    expect(within(panel).getByText("Demo scan")).toBeInTheDocument();
    expect(
      within(panel).getByText("No files have been changed"),
    ).toBeInTheDocument();
  });

  test("shows scan feedback in the fixed report instead of the workspace", () => {
    render(
      <ScanReport
        proposal={proposal}
        status="loading"
        statusMessage="Reviewing trusted local drop…"
      />,
    );

    expect(screen.getByRole("status", { name: "Drop status" })).toHaveTextContent(
      "Reviewing trusted local drop",
    );
    expect(screen.getByText("Live scan")).toBeInTheDocument();
  });
});
