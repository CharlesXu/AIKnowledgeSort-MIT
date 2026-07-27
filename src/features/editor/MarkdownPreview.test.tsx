import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { MarkdownPreview } from "./MarkdownPreview";

const extendedFixture = `---
title: Demo
owner: Local
---

# Review note

| Feature | State |
| --- | --- |
| Table | Ready |

- [x] Done
- [ ] Pending

Footnote reference[^1].

[^1]: Local evidence.

> [!WARNING]
> Verify the source.

[[MCU reset|Reset note]]

Evidence paragraph ^evidence-1

<script>window.__unsafe = true</script>

[Unsafe script](javascript:alert(1))
[Local file](file:///tmp/private.txt)
[Project site](https://example.com)
`;

afterEach(() => {
  vi.restoreAllMocks();
});

describe("MarkdownPreview", () => {
  test("renders extended local Markdown representations", () => {
    render(<MarkdownPreview source={extendedFixture} />);

    expect(screen.getByRole("heading", { name: "Review note" })).toBeVisible();
    expect(screen.getByRole("table")).toBeVisible();
    expect(screen.getByRole("checkbox", { name: "Task done" })).toBeDisabled();
    expect(
      screen.getByRole("checkbox", { name: "Task pending" }),
    ).toBeDisabled();
    expect(screen.getByText("title: Demo")).toBeVisible();
    expect(screen.getByText("owner: Local")).toBeVisible();
    expect(screen.getByText("Local evidence.")).toBeVisible();
    expect(screen.getByText("^evidence-1").tagName).toBe("CODE");
    expect(screen.getByText("Reset note")).toHaveAttribute(
      "data-link-kind",
      "wiki",
    );
    const warning = document.querySelector('blockquote[data-callout="warning"]');
    expect(warning).toHaveTextContent("Verify the source.");
  });

  test("removes raw HTML and keeps every link inside a local handoff", () => {
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockRejectedValue(new Error("network must remain unused"));
    render(<MarkdownPreview source={extendedFixture} />);

    expect(document.querySelector("script")).toBeNull();
    expect(document.querySelector("a")).toBeNull();

    const preview = screen.getByRole("region", { name: "Document preview" });
    for (const label of ["Unsafe script", "Local file", "Project site"]) {
      fireEvent.click(within(preview).getByRole("button", { name: label }));
    }

    expect(open).not.toHaveBeenCalled();
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(
      screen.getByRole("status", { name: "Link handoff status" }),
    ).toHaveTextContent(/link opening is disabled/i);
    expect(screen.getByText("Unsafe script")).toHaveAttribute(
      "data-link-kind",
      "blocked",
    );
    expect(screen.getByText("Local file")).toHaveAttribute(
      "data-link-kind",
      "blocked",
    );
    expect(screen.getByText("Project site")).toHaveAttribute(
      "data-link-kind",
      "web",
    );
  });

  test("renders ordinary code fences as escaped text", () => {
    render(
      <MarkdownPreview source={"```html\n<img src=x onerror=alert(1)>\n```"} />,
    );

    expect(screen.getByText("<img src=x onerror=alert(1)>")).toBeVisible();
    expect(document.querySelector("img")).toBeNull();
  });

  test("routes Mermaid fences through the strict diagram policy", () => {
    render(
      <MarkdownPreview
        source={'```mermaid\ngraph TD\nclick A href "https://example.com"\n```'}
      />,
    );

    expect(
      screen.getByRole("alert", { name: "Mermaid diagnostic" }),
    ).toHaveTextContent(/click directives are disabled/i);
    expect(screen.getByText("Mermaid source")).toBeVisible();
  });
});
