import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import App from "./App";
import type { DiscoveryClient } from "./features/drop/discoveryClient";
import type {
  NativeDropBridge,
  NativeDropCallbacks,
} from "./features/drop/useNativeDrop";
import type { DiscoveryProposal } from "./features/drop/types";

const liveProposal: DiscoveryProposal = {
  items: [{ path: "/live/trusted.md", name: "trusted.md", byteSize: 1024 }],
  counts: {
    included: 7,
    excluded: 6,
    unreadable: 5,
    symlink: 4,
    outOfScope: 3,
  },
  diagnostics: [],
};

function createNativeDropHarness() {
  let callbacks: NativeDropCallbacks | undefined;
  const dropBridge: NativeDropBridge = {
    subscribe(nextCallbacks) {
      callbacks = nextCallbacks;
      return Promise.resolve(() => {});
    },
  };
  const proposeLocalDrop = vi.fn().mockResolvedValue(liveProposal);
  const discoveryClient: DiscoveryClient = { proposeLocalDrop };

  return {
    discoveryClient,
    dropBridge,
    proposeLocalDrop,
    grant(grantId: string) {
      callbacks?.onGrant({ grantId });
    },
    error(message: string) {
      callbacks?.onGrantError(message);
    },
    drag(type: "over" | "drop" | "cancel") {
      callbacks?.onDragState({ type });
    },
  };
}

describe("source workbench shell", () => {
  test("clearly labels the deterministic browser fixture", () => {
    render(<App />);

    expect(screen.getByText("Demo proposal")).toBeInTheDocument();
    expect(screen.getByText(/deterministic browser fixture/i)).toBeInTheDocument();
  });

  test("replaces the demo with a trusted native proposal", async () => {
    const harness = createNativeDropHarness();
    render(
      <App
        discoveryClient={harness.discoveryClient}
        dropBridge={harness.dropBridge}
      />,
    );

    act(() => harness.grant("opaque-live-grant"));

    await waitFor(() =>
      expect(screen.getAllByText("trusted.md")).not.toHaveLength(0),
    );
    expect(harness.proposeLocalDrop).toHaveBeenCalledWith({
      grantId: "opaque-live-grant",
    });
    expect(screen.queryByText("Demo proposal")).toBeNull();
    expect(screen.getByText("Live proposal")).toBeInTheDocument();
    expect(
      screen.getByRole("status", { name: "Included" }),
    ).toHaveTextContent("7");
  });

  test("shows a full-workbench native hover overlay and removes it on cancel", () => {
    const harness = createNativeDropHarness();
    render(
      <App
        discoveryClient={harness.discoveryClient}
        dropBridge={harness.dropBridge}
      />,
    );

    act(() => harness.drag("over"));
    expect(
      screen.getByRole("status", { name: "Native drop target" }),
    ).toHaveTextContent(/release to review/i);

    act(() => harness.drag("cancel"));
    expect(
      screen.queryByRole("status", { name: "Native drop target" }),
    ).toBeNull();
  });

  test("surfaces loading, native errors, and ignored external URL drops", () => {
    const harness = createNativeDropHarness();
    harness.proposeLocalDrop.mockReturnValueOnce(new Promise(() => {}));
    render(
      <App
        discoveryClient={harness.discoveryClient}
        dropBridge={harness.dropBridge}
      />,
    );

    act(() => harness.grant("slow-grant"));
    expect(screen.getByRole("status", { name: "Drop status" })).toHaveTextContent(
      /reviewing trusted local drop/i,
    );

    act(() => harness.error("Native grant failed"));
    expect(screen.getByRole("status", { name: "Drop status" })).toHaveTextContent(
      "Native grant failed",
    );

    fireEvent.drop(screen.getByRole("main", { name: "Source workbench" }), {
      dataTransfer: {
        files: { length: 0 },
        types: ["text/uri-list"],
        getData: () => "https://example.test",
      },
    });
    expect(screen.getByRole("status", { name: "Drop status" })).toHaveTextContent(
      /external text and URL drops are ignored/i,
    );
    expect(harness.proposeLocalDrop).toHaveBeenCalledTimes(1);
  });

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
