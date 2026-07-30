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
  proposalId: "live-proposal",
  items: [
    {
      itemId: "live-trusted",
      path: "/live/trusted.md",
      name: "trusted.md",
      byteSize: 1024,
      identity: {
        algorithm: "SHA-256",
        digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0",
      },
    },
    {
      itemId: "live-summary",
      path: "/live/reports/summary.txt",
      name: "summary.txt",
      byteSize: 2048,
      identity: {
        algorithm: "SHA-256",
        digest: "ab5f329afb80f567b441324ad2d048ca910644b17c7426f9cc585307c5077496",
      },
    },
  ],
  counts: {
    included: 2,
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
  test("loads a trusted proposal from the native add-files picker", async () => {
    const harness = createNativeDropHarness();
    const sourcePickerClient = {
      chooseFiles: vi.fn().mockResolvedValue({
        grantId: "opaque-picker-grant",
      }),
      chooseFolders: vi.fn(),
    };
    const appProps = {
      discoveryClient: harness.discoveryClient,
      dropBridge: harness.dropBridge,
      sourcePickerClient,
    } as Parameters<typeof App>[0];
    render(<App {...appProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Add source" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add files…" }));

    await waitFor(() =>
      expect(screen.getAllByText("trusted.md")).not.toHaveLength(0),
    );
    expect(sourcePickerClient.chooseFiles).toHaveBeenCalledTimes(1);
    expect(harness.proposeLocalDrop).toHaveBeenCalledWith({
      grantId: "opaque-picker-grant",
    });
    expect(screen.queryByText("Demo scan")).toBeNull();
  });

  test("treats picker cancellation as a no-op and prevents concurrent pickers", async () => {
    let finishSelection: (value: null) => void = () => {};
    const sourcePickerClient = {
      chooseFiles: vi.fn(
        () =>
          new Promise<null>((resolve) => {
            finishSelection = resolve;
          }),
      ),
      chooseFolders: vi.fn(),
    };
    const harness = createNativeDropHarness();
    const appProps = {
      discoveryClient: harness.discoveryClient,
      dropBridge: harness.dropBridge,
      sourcePickerClient,
    } as Parameters<typeof App>[0];
    render(<App {...appProps} />);

    const addSource = screen.getByRole("button", { name: "Add source" });
    fireEvent.click(addSource);
    fireEvent.click(screen.getByRole("menuitem", { name: "Add files…" }));

    expect(addSource).toBeDisabled();
    fireEvent.click(addSource);
    expect(sourcePickerClient.chooseFiles).toHaveBeenCalledTimes(1);

    act(() => finishSelection(null));

    await waitFor(() => expect(addSource).toBeEnabled());
    expect(screen.getByText("Demo scan")).toBeInTheDocument();
    expect(harness.proposeLocalDrop).not.toHaveBeenCalled();
  });

  test("reports that source picking requires the desktop runtime in browser mode", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Add source" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add folders…" }));

    await waitFor(() =>
      expect(
        screen.getByRole("status", { name: "Drop status" }),
      ).toHaveTextContent(/requires the desktop app/i),
    );
    expect(screen.getByText("Demo scan")).toBeInTheDocument();
  });

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
    ).toHaveTextContent("2");

    const sources = screen.getByRole("region", { name: "Sources" });
    const sourceRoot = within(sources).getByRole("checkbox", {
      name: "Select live directory",
    });
    expect(
      within(sources).queryByRole("checkbox", {
        name: "Select Roadmap.md file",
      }),
    ).toBeNull();

    fireEvent.click(sourceRoot);
    expect(
      screen.getByRole("checkbox", { name: "Include trusted.md" }),
    ).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: "Include summary.txt" }),
    ).toBeChecked();

    fireEvent.click(
      screen.getByRole("checkbox", { name: "Include trusted.md" }),
    );
    expect(sourceRoot).toBePartiallyChecked();
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
      /reviewing trusted local sources/i,
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
    expect(
      screen.getByText(
        "Ninebot document and electronic archive classification",
      ),
    ).toBeVisible();
    expect(
      screen.getByText("0 executable rules — semantic review required"),
    ).toBeVisible();
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

  test("resizes both side panes by dragging their vertical separators", () => {
    render(<App />);

    const sourceSeparator = screen.getByRole("separator", {
      name: "Resize Sources panel",
    });
    fireEvent.pointerDown(sourceSeparator, { clientX: 300, pointerId: 1 });
    fireEvent.pointerMove(sourceSeparator, { clientX: 360, pointerId: 1 });
    fireEvent.pointerUp(sourceSeparator, { clientX: 360, pointerId: 1 });
    expect(sourceSeparator).toHaveAttribute("aria-valuenow", "308");

    const contextSeparator = screen.getByRole("separator", {
      name: "Resize import review context",
    });
    fireEvent.pointerDown(contextSeparator, { clientX: 900, pointerId: 2 });
    fireEvent.pointerMove(contextSeparator, { clientX: 960, pointerId: 2 });
    fireEvent.pointerUp(contextSeparator, { clientX: 960, pointerId: 2 });
    expect(contextSeparator).toHaveAttribute("aria-valuenow", "500");
  });

  test("uses the narrow toolbar to navigate implemented workbench areas", async () => {
    render(<App />);

    const search = screen.getByRole("button", { name: "Search" });
    const graph = screen.getByRole("button", { name: "Graph" });
    const classification = screen.getByRole("button", {
      name: "Classification",
    });
    const archive = screen.getByRole("button", { name: "Archive" });
    const sources = screen.getByRole("button", { name: "Sources" });

    expect(search).toBeEnabled();
    expect(graph).toBeEnabled();
    expect(classification).toBeEnabled();
    expect(archive).toBeEnabled();

    fireEvent.click(search);
    expect(
      screen.getByRole("searchbox", { name: "Search sources" }),
    ).toHaveFocus();
    expect(search).toHaveAttribute("aria-current", "page");

    fireEvent.click(classification);
    expect(
      await screen.findByText(
        "Ninebot document and electronic archive classification",
      ),
    ).toBeVisible();
    expect(classification).toHaveAttribute("aria-current", "page");

    fireEvent.click(graph);
    expect(
      screen.getByRole("region", { name: "Proposal topology" }),
    ).toBeVisible();
    expect(graph).toHaveAttribute("aria-current", "page");

    fireEvent.click(archive);
    expect(screen.getByRole("region", { name: "Archive preview" })).toHaveFocus();
    expect(archive).toHaveAttribute("aria-current", "page");

    fireEvent.click(sources);
    expect(screen.getByRole("region", { name: "Sources" })).toHaveFocus();
    expect(sources).toHaveAttribute("aria-current", "page");
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
