export const PANE_LAYOUT_STORAGE_KEY = "ai-knowledge-sort:pane-layout";

export const NAVIGATION_WIDTH_MIN = 220;
export const NAVIGATION_WIDTH_MAX = 380;
export const CONTEXT_WIDTH_MIN = 260;
export const CONTEXT_WIDTH_MAX = 420;

export interface PaneLayout {
  readonly version: 1;
  readonly navigationWidth: number;
  readonly contextWidth: number;
  readonly navigationCollapsed: boolean;
  readonly contextCollapsed: boolean;
}

export const DEFAULT_PANE_LAYOUT: PaneLayout = {
  version: 1,
  navigationWidth: 286,
  contextWidth: 300,
  navigationCollapsed: false,
  contextCollapsed: false,
};

function isWidthInRange(
  value: unknown,
  minimum: number,
  maximum: number,
): value is number {
  return (
    typeof value === "number" &&
    Number.isFinite(value) &&
    value >= minimum &&
    value <= maximum
  );
}

function isPaneLayout(value: unknown): value is PaneLayout {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  return (
    candidate.version === 1 &&
    isWidthInRange(
      candidate.navigationWidth,
      NAVIGATION_WIDTH_MIN,
      NAVIGATION_WIDTH_MAX,
    ) &&
    isWidthInRange(
      candidate.contextWidth,
      CONTEXT_WIDTH_MIN,
      CONTEXT_WIDTH_MAX,
    ) &&
    typeof candidate.navigationCollapsed === "boolean" &&
    typeof candidate.contextCollapsed === "boolean"
  );
}

export function parsePaneLayout(value: string | null): PaneLayout {
  if (value === null) {
    return DEFAULT_PANE_LAYOUT;
  }

  try {
    const parsed: unknown = JSON.parse(value);
    if (!isPaneLayout(parsed)) {
      return DEFAULT_PANE_LAYOUT;
    }

    return {
      version: 1,
      navigationWidth: parsed.navigationWidth,
      contextWidth: parsed.contextWidth,
      navigationCollapsed: parsed.navigationCollapsed,
      contextCollapsed: parsed.contextCollapsed,
    };
  } catch {
    return DEFAULT_PANE_LAYOUT;
  }
}

export function readPaneLayout(): PaneLayout {
  try {
    return parsePaneLayout(localStorage.getItem(PANE_LAYOUT_STORAGE_KEY));
  } catch {
    return DEFAULT_PANE_LAYOUT;
  }
}

export function persistPaneLayout(layout: PaneLayout): void {
  try {
    localStorage.setItem(PANE_LAYOUT_STORAGE_KEY, JSON.stringify(layout));
  } catch {
    // Storage can be unavailable in hardened webviews; the in-memory layout
    // remains fully usable for the current session.
  }
}
