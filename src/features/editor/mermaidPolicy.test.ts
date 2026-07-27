import { describe, expect, test } from "vitest";
import {
  sanitizeMermaidSvg,
  validateMermaidSource,
} from "./mermaidPolicy";

describe("validateMermaidSource", () => {
  test("accepts a bounded local diagram", () => {
    expect(validateMermaidSource("graph TD\nA --> B")).toEqual({ ok: true });
  });

  test.each([
    ["%%{init: {'theme':'dark'}}%%\ngraph TD\nA-->B", /directive/i],
    ["%%{config: {'theme':'dark'}}%%\ngraph TD\nA-->B", /directive/i],
    ["graph TD\nclick A href \"https://example.com\"", /click/i],
    ["", /empty/i],
    ["a".repeat(50_001), /too large/i],
  ])("rejects unsafe or invalid source", (source, expectedMessage) => {
    const result = validateMermaidSource(source);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.message).toMatch(expectedMessage);
    }
  });
});

describe("sanitizeMermaidSvg", () => {
  test("keeps inert diagram geometry and removes active SVG content", () => {
    const clean = sanitizeMermaidSvg(`
      <svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)">
        <style>@import url(https://example.com/x.css)</style>
        <script>alert(1)</script>
        <foreignObject><div>unsafe</div></foreignObject>
        <image href="https://example.com/x.png" />
        <a href="javascript:alert(1)"><text>unsafe link</text></a>
        <g id="node-1" style="background:url(https://example.com/x)">
          <circle cx="8" cy="8" r="4" onclick="alert(1)" />
        </g>
      </svg>
    `);
    const document = new DOMParser().parseFromString(clean, "image/svg+xml");

    expect(document.querySelector("circle")).not.toBeNull();
    expect(
      document.querySelector("script, style, foreignObject, image, a"),
    ).toBeNull();
    expect(document.querySelector("[onload], [onclick], [href], [style]")).toBeNull();
  });
});
