import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { MermaidBlock } from "./MermaidBlock";

const mermaidMock = vi.hoisted(() => ({
  initialize: vi.fn(),
  parse: vi.fn(),
  render: vi.fn(),
}));

vi.mock("mermaid", () => ({
  default: mermaidMock,
}));

describe("MermaidBlock", () => {
  beforeEach(() => {
    mermaidMock.initialize.mockClear();
    mermaidMock.parse.mockReset().mockResolvedValue({ diagramType: "flowchart" });
    mermaidMock.render.mockReset().mockResolvedValue({
      svg: `
        <svg xmlns="http://www.w3.org/2000/svg">
          <style>@import url(https://example.com/x.css)</style>
          <script>alert(1)</script>
          <foreignObject><div>unsafe</div></foreignObject>
          <image href="https://example.com/x.png" />
          <a href="javascript:alert(1)"><text>unsafe</text></a>
          <g id="safe-node" style="background:url(https://example.com/x)">
            <circle cx="8" cy="8" r="4" onclick="alert(1)" />
          </g>
        </svg>
      `,
      bindFunctions: vi.fn(),
    });
  });

  test("renders sanitized local SVG without binding active handlers", async () => {
    render(<MermaidBlock source={"graph TD\nA --> B"} />);

    const diagram = await screen.findByRole("img", {
      name: "Rendered Mermaid diagram",
    });
    expect(diagram.querySelector("circle")).not.toBeNull();
    expect(
      diagram.querySelector("script, style, foreignObject, image, a"),
    ).toBeNull();
    expect(diagram.querySelector("[onclick], [href], [style]")).toBeNull();
    expect(mermaidMock.parse).toHaveBeenCalledWith("graph TD\nA --> B", {
      suppressErrors: false,
    });
    expect(screen.getByText("Mermaid source").closest("details")).toHaveTextContent(
      "graph TD A --> B",
    );
  });

  test("preserves source and reports an actionable render diagnostic", async () => {
    mermaidMock.parse.mockRejectedValueOnce(new Error("Unexpected token"));
    render(<MermaidBlock source={"graph TD\nA ---"} />);

    await waitFor(() =>
      expect(
        screen.getByRole("alert", { name: "Mermaid diagnostic" }),
      ).toHaveTextContent(/check the diagram syntax/i),
    );
    expect(screen.getByText("Mermaid source").closest("details")).toHaveTextContent(
      "graph TD A ---",
    );
    expect(screen.queryByRole("img", { name: "Rendered Mermaid diagram" })).toBeNull();
  });

  test("rejects unsafe directives before loading the renderer", () => {
    render(
      <MermaidBlock
        source={'graph TD\nclick A href "https://example.com"'}
      />,
    );

    expect(
      screen.getByRole("alert", { name: "Mermaid diagnostic" }),
    ).toHaveTextContent(/click/i);
    expect(mermaidMock.parse).not.toHaveBeenCalled();
    expect(mermaidMock.render).not.toHaveBeenCalled();
  });
});
