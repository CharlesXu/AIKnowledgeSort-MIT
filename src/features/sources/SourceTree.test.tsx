import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { demoSources } from "../../data/demoSources";
import { SourceTree } from "./SourceTree";

describe("SourceTree", () => {
  test("gives every visible directory and file a labelled checkbox", () => {
    render(<SourceTree tree={demoSources} />);

    const rows = screen.getAllByRole("treeitem");
    expect(rows.length).toBeGreaterThan(4);

    for (const row of rows) {
      expect(within(row).getByRole("checkbox")).toHaveAccessibleName();
    }
  });

  test("selects a directory, shows child deselection as indeterminate, and restores it", () => {
    render(<SourceTree tree={demoSources} />);

    const projects = screen.getByRole("checkbox", {
      name: "Select Projects directory",
    });
    const roadmap = screen.getByRole("checkbox", {
      name: "Select Roadmap.md file",
    });

    fireEvent.click(projects);
    expect(projects).toBeChecked();
    expect(roadmap).toBeChecked();

    fireEvent.click(roadmap);
    expect(projects).toBePartiallyChecked();
    expect(screen.getByText("1 unique eligible file selected")).toBeInTheDocument();

    fireEvent.click(roadmap);
    expect(projects).toBeChecked();
    expect(screen.getByText("2 unique eligible files selected")).toBeInTheDocument();
  });

  test("deduplicates mixed explicit file and directory selections in its summary", () => {
    render(
      <SourceTree
        tree={demoSources}
        initialSelectionIds={[
          "workspace/projects",
          "workspace/projects/Roadmap.md",
          "workspace/README.md",
        ]}
      />,
    );

    expect(screen.getByText("3 unique eligible files selected")).toBeInTheDocument();
  });

  test("exposes labelled disclosure controls and keyboard-operable native inputs", () => {
    render(<SourceTree tree={demoSources} />);

    const disclosure = screen.getByRole("button", {
      name: "Collapse Projects directory",
    });
    expect(disclosure).toHaveAttribute("aria-expanded", "true");

    fireEvent.click(disclosure);
    expect(
      screen.queryByRole("checkbox", { name: "Select Roadmap.md file" }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: "Expand Projects directory" }),
    ).toHaveAttribute("aria-expanded", "false");

    const readme = screen.getByRole("checkbox", {
      name: "Select README.md file",
    });
    readme.focus();
    expect(readme).toHaveFocus();
    fireEvent.keyDown(readme, { key: " " });
    fireEvent.click(readme);
    expect(readme).toBeChecked();
  });

  test("filters matching sources while retaining their ancestor context", () => {
    render(<SourceTree tree={demoSources} />);

    fireEvent.change(screen.getByRole("searchbox", { name: "Search sources" }), {
      target: { value: "synthesis" },
    });

    expect(screen.getByText("Local workspace")).toBeInTheDocument();
    expect(screen.getByText("Projects")).toBeInTheDocument();
    expect(screen.getByText("Research")).toBeInTheDocument();
    expect(screen.getByText("synthesis.txt")).toBeInTheDocument();
    expect(screen.queryByText("Roadmap.md")).toBeNull();
    expect(screen.queryByText("Notes")).toBeNull();
  });

  test("preserves selection while filtering and restoring the full tree", () => {
    render(<SourceTree tree={demoSources} />);

    fireEvent.click(
      screen.getByRole("checkbox", { name: "Select Projects directory" }),
    );
    expect(screen.getByText("2 unique eligible files selected")).toBeInTheDocument();

    const search = screen.getByRole("searchbox", { name: "Search sources" });
    fireEvent.change(search, { target: { value: "synthesis" } });
    fireEvent.change(search, { target: { value: "" } });

    expect(
      screen.getByRole("checkbox", { name: "Select Projects directory" }),
    ).toBeChecked();
    expect(screen.getByText("2 unique eligible files selected")).toBeInTheDocument();
  });

  test("shows a clear empty result without removing the selection summary", () => {
    render(<SourceTree tree={demoSources} />);

    fireEvent.change(screen.getByRole("searchbox", { name: "Search sources" }), {
      target: { value: "no-such-source" },
    });

    expect(
      screen.getByRole("status", { name: "Source filter status" }),
    ).toHaveTextContent("No sources match");
    expect(screen.getByText("0 unique eligible files selected")).toBeInTheDocument();
  });
});
