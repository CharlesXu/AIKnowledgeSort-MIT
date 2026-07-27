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

    const scanReport = screen.getByRole("region", { name: "Scan report" });
    expect(within(scanReport).getByText("Demo scan")).toBeInTheDocument();
    expect(within(scanReport).getByText(/browser fixture/i)).toBeInTheDocument();
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
    expect(screen.queryByText("Demo scan")).toBeNull();
    expect(screen.getByText("Live scan")).toBeInTheDocument();
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
    const header = within(workbench).getByRole("banner", {
      name: "Application header",
    });
    const toolbar = within(workbench).getByRole("toolbar", {
      name: "Workbench tools",
    });
    const sources = within(workbench).getByRole("region", {
      name: "Sources",
    });

    expect(header).toHaveTextContent("AI Knowledge Sort");
    expect(header).toHaveTextContent("Local");
    expect(toolbar).toHaveClass("tool-rail");
    expect(toolbar).toHaveAttribute("data-width", "44");
    expect(sources).toHaveClass("source-panel");
    expect(toolbar.nextElementSibling).toBe(sources);
  });

  test("reserves the central workspace for Markdown, Mermaid, and code", () => {
    render(<App />);

    const sources = screen.getByRole("region", { name: "Sources" });
    const archivePreview = screen.getByRole("region", {
      name: "Archive preview",
    });
    const documentWorkspace = screen.getByRole("region", {
      name: "Document workspace",
    });

    expect(sources).toHaveTextContent("IndexedSource");
    expect(within(sources).getByRole("region", { name: "Scan report" })).toHaveTextContent(
      "No files have been changed",
    );
    expect(archivePreview).toHaveTextContent("Archive Preview");
    expect(archivePreview).toHaveTextContent("Uncommitted");
    expect(documentWorkspace).toHaveTextContent("Markdown");
    expect(documentWorkspace).toHaveTextContent("Mermaid");
    expect(documentWorkspace).toHaveTextContent("Code");
    expect(
      within(documentWorkspace).queryByRole("region", {
        name: "Discovery proposal",
      }),
    ).toBeNull();
    expect(archivePreview.compareDocumentPosition(documentWorkspace)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  test("keeps one exact draft through source, live preview, and reading modes", () => {
    render(<App />);

    const editor = screen.getByRole("textbox", {
      name: "Markdown, Mermaid, and code editor",
    });
    const changedDraft =
      "# Workspace note\n\nA changed paragraph.\n\n```mermaid\ngraph TD\nclick A href \"https://example.com\"\n```\n\n```ts\nconst ready = true;\n```";
    fireEvent.change(editor, {
      target: { value: changedDraft },
    });
    fireEvent.click(screen.getByRole("tab", { name: "Live preview" }));

    const preview = screen.getByRole("region", { name: "Document preview" });
    expect(
      screen.getByRole("textbox", {
        name: "Markdown, Mermaid, and code editor",
      }),
    ).toHaveValue(changedDraft);
    expect(within(preview).getByRole("heading", { name: "Workspace note" })).toBeVisible();
    expect(preview).toHaveTextContent("A changed paragraph.");
    expect(
      within(preview).getByRole("alert", { name: "Mermaid diagnostic" }),
    ).toHaveTextContent(/click directives are disabled/i);
    expect(preview).toHaveTextContent("const ready = true;");
    expect(within(preview).getByText("TypeScript")).toBeVisible();

    fireEvent.click(screen.getByRole("tab", { name: "Reading" }));
    expect(
      screen.queryByRole("textbox", {
        name: "Markdown, Mermaid, and code editor",
      }),
    ).toBeNull();
    expect(
      screen.getByRole("region", { name: "Document preview" }),
    ).toHaveTextContent("Workspace note");

    fireEvent.click(screen.getByRole("tab", { name: "Source" }));
    expect(
      screen.getByRole("textbox", {
        name: "Markdown, Mermaid, and code editor",
      }),
    ).toHaveValue(changedDraft);
    expect(
      screen.queryByRole("region", { name: "Document preview" }),
    ).toBeNull();
  });

  test("switches the right context between proposal topology and import review", () => {
    render(<App />);

    const topology = screen.getByRole("region", { name: "Proposal topology" });
    expect(topology).toHaveTextContent("Not yet ingested");
    expect(within(topology).getByText("meeting-notes.md")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Play knowledge timeline" }),
    ).toBeDisabled();
    expect(screen.getByText(/available after confirmed ingestion/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "Import Review" }));
    expect(screen.getByRole("list", { name: "Proposal status counts" })).toBeVisible();
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
    ).toHaveAttribute("data-collapse-at", "1440");
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
    expect(sourceSeparator).toHaveAttribute("aria-valuenow", "256");
    expect(contextSeparator).toHaveAttribute("aria-valuenow", "568");

    first.unmount();
    render(<App />);

    expect(
      screen.getByRole("separator", { name: "Resize Sources panel" }),
    ).toHaveAttribute("aria-valuenow", "256");
    expect(
      screen.getByRole("separator", {
        name: "Resize import review context",
      }),
    ).toHaveAttribute("aria-valuenow", "568");
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
    ).toHaveTextContent("Knowledge Graph");
  });

  test.each([
    ["invalid JSON", "not-json"],
    [
      "wrong schema",
      JSON.stringify({
        version: 1,
        navigationWidth: "248",
        contextWidth: 560,
        navigationCollapsed: false,
        contextCollapsed: false,
      }),
    ],
    [
      "out-of-range widths",
      JSON.stringify({
        version: 1,
        navigationWidth: 42,
        contextWidth: 901,
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
      ).toHaveAttribute("aria-valuenow", "248");
      expect(
        screen.getByRole("separator", {
          name: "Resize import review context",
        }),
      ).toHaveAttribute("aria-valuenow", "560");
      expect(
        screen.getByRole("tree", { name: "Local source folders" }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("region", { name: "Scan report" }),
      ).toHaveTextContent("No files have been changed");
      expect(
        screen.getByRole("complementary", {
          name: "Import review context",
        }),
      ).toHaveTextContent("Knowledge Graph");
    },
  );
});
