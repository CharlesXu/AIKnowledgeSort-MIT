import { describe, expect, test } from "vitest";
import {
  DEFAULT_PANE_LAYOUT,
  parsePaneLayout,
} from "./paneLayout";

describe("pane layout persistence", () => {
  test("accepts a valid versioned pane layout", () => {
    expect(
      parsePaneLayout(
        JSON.stringify({
          version: 1,
          navigationWidth: 320,
          contextWidth: 360,
          navigationCollapsed: true,
          contextCollapsed: false,
        }),
      ),
    ).toEqual({
      version: 1,
      navigationWidth: 320,
      contextWidth: 360,
      navigationCollapsed: true,
      contextCollapsed: false,
    });
  });

  test.each([
    null,
    "not-json",
    JSON.stringify({ version: 2 }),
    JSON.stringify({
      version: 1,
      navigationWidth: "286",
      contextWidth: 300,
      navigationCollapsed: false,
      contextCollapsed: false,
    }),
    JSON.stringify({
      version: 1,
      navigationWidth: 219,
      contextWidth: 421,
      navigationCollapsed: false,
      contextCollapsed: false,
    }),
  ])("falls back atomically for invalid persisted input", (input) => {
    expect(parsePaneLayout(input)).toEqual(DEFAULT_PANE_LAYOUT);
  });
});
