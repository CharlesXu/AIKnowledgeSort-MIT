import {
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { describe, expect, test } from "vitest";
import App from "./App";

describe("source workbench shell", () => {
  test("renders the workbench landmark with a narrow toolbar beside Sources", () => {
    render(<App />);

    const workbench = screen.getByRole("main", { name: "Source workbench" });
    const toolbar = within(workbench).getByRole("toolbar", {
      name: "Workbench tools",
    });
    const sources = within(workbench).getByRole("region", {
      name: "Sources",
    });

    expect(toolbar).toHaveClass("tool-rail");
    expect(toolbar).toHaveAttribute("data-width", "44");
    expect(sources).toHaveClass("source-panel");
    expect(toolbar.nextElementSibling).toBe(sources);
  });

  test("provides adjustable pane separators and responsive collapse hooks", () => {
    render(<App />);

    const separators = screen.getAllByRole("separator");
    expect(separators).toHaveLength(2);

    const sourceSeparator = screen.getByRole("separator", {
      name: "Resize Sources panel",
    });
    expect(sourceSeparator).toHaveAttribute("aria-orientation", "vertical");
    expect(sourceSeparator).toHaveAttribute("tabindex", "0");

    const originalWidth = sourceSeparator.getAttribute("aria-valuenow");
    fireEvent.keyDown(sourceSeparator, { key: "ArrowRight" });
    expect(sourceSeparator.getAttribute("aria-valuenow")).not.toBe(originalWidth);

    expect(screen.getByRole("region", { name: "Sources" })).toHaveAttribute(
      "data-collapse-at",
      "760",
    );
    expect(
      screen.getByRole("complementary", { name: "Import review context" }),
    ).toHaveAttribute("data-collapse-at", "1120");
  });

  test("labels deferred tools honestly and exposes no fake primary action", () => {
    render(<App />);

    expect(screen.getByRole("button", { name: /Graph.*coming later/i })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: /Classification.*coming later/i }),
    ).toBeDisabled();
    expect(screen.queryByRole("button", { name: /import files/i })).toBeNull();
  });

  test("persists the last valid resized pane layout across remounts", () => {
    const first = render(<App />);
    const sourceSeparator = screen.getByRole("separator", {
      name: "Resize Sources panel",
    });
    const contextSeparator = screen.getByRole("separator", {
      name: "Resize import review context",
    });

    fireEvent.keyDown(sourceSeparator, { key: "ArrowRight" });
    fireEvent.keyDown(contextSeparator, { key: "ArrowLeft" });
    expect(sourceSeparator).toHaveAttribute("aria-valuenow", "294");
    expect(contextSeparator).toHaveAttribute("aria-valuenow", "308");

    first.unmount();
    render(<App />);

    expect(
      screen.getByRole("separator", { name: "Resize Sources panel" }),
    ).toHaveAttribute("aria-valuenow", "294");
    expect(
      screen.getByRole("separator", {
        name: "Resize import review context",
      }),
    ).toHaveAttribute("aria-valuenow", "308");
  });

  test("persists explicit pane collapse states and lets users restore both panes", () => {
    const first = render(<App />);

    fireEvent.click(
      screen.getByRole("button", { name: "Collapse Sources panel" }),
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: "Collapse Import review context",
      }),
    );

    expect(
      screen.getByRole("button", { name: "Expand Sources panel" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "Expand Import review context",
      }),
    ).toBeInTheDocument();

    first.unmount();
    render(<App />);

    fireEvent.click(
      screen.getByRole("button", { name: "Expand Sources panel" }),
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: "Expand Import review context",
      }),
    );

    expect(
      screen.getByRole("tree", { name: "Local source folders" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("complementary", { name: "Import review context" }),
    ).toHaveTextContent("Proposal context");
  });

  test.each([
    ["invalid JSON", "not-json"],
    [
      "wrong schema",
      JSON.stringify({
        version: 1,
        navigationWidth: "286",
        contextWidth: 300,
        navigationCollapsed: false,
        contextCollapsed: false,
      }),
    ],
    [
      "out-of-range widths",
      JSON.stringify({
        version: 1,
        navigationWidth: 42,
        contextWidth: 900,
        navigationCollapsed: true,
        contextCollapsed: true,
      }),
    ],
  ])(
    "falls back to a usable expanded layout for %s without losing content",
    (_label, persistedValue) => {
      localStorage.setItem(
        "ai-knowledge-sort:pane-layout",
        persistedValue,
      );

      render(<App />);

      expect(
        screen.getByRole("separator", { name: "Resize Sources panel" }),
      ).toHaveAttribute("aria-valuenow", "286");
      expect(
        screen.getByRole("separator", {
          name: "Resize import review context",
        }),
      ).toHaveAttribute("aria-valuenow", "300");
      expect(
        screen.getByRole("tree", { name: "Local source folders" }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("region", { name: "Discovery proposal" }),
      ).toHaveTextContent("No files have been changed");
      expect(
        screen.getByRole("complementary", {
          name: "Import review context",
        }),
      ).toHaveTextContent("Proposal context");
    },
  );
});
