import { fireEvent, render, screen, within } from "@testing-library/react";
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
});
