import { beforeEach, describe, expect, test, vi } from "vitest";

const tauri = vi.hoisted(() => {
  const eventHandlers = new Map<string, (event: { payload: unknown }) => void>();
  let dragHandler:
    | ((event: {
        payload:
          | { type: "drop"; paths: string[]; position: { x: number; y: number } }
          | { type: "over"; position: { x: number; y: number } }
          | { type: "leave" };
      }) => void)
    | undefined;
  const unlistenGrant = vi.fn();
  const unlistenError = vi.fn();
  const unlistenDrag = vi.fn();

  return {
    eventHandlers,
    get dragHandler() {
      return dragHandler;
    },
    set dragHandler(handler) {
      dragHandler = handler;
    },
    listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
      eventHandlers.set(name, handler);
      return name === "local-drop-grant" ? unlistenGrant : unlistenError;
    }),
    onDragDropEvent: vi.fn(async (handler) => {
      dragHandler = handler;
      return unlistenDrag;
    }),
    unlistenDrag,
    unlistenError,
    unlistenGrant,
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: tauri.listen,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: tauri.onDragDropEvent,
  }),
}));

import { tauriNativeDropBridge } from "./useNativeDrop";

describe("tauriNativeDropBridge", () => {
  beforeEach(() => {
    tauri.eventHandlers.clear();
    tauri.dragHandler = undefined;
    vi.clearAllMocks();
  });

  test("subscribes to trusted grant events and keeps raw drop paths out of callbacks", async () => {
    const onGrant = vi.fn();
    const onGrantError = vi.fn();
    const onDragState = vi.fn();

    const cleanup = await tauriNativeDropBridge.subscribe({
      onGrant,
      onGrantError,
      onDragState,
    });

    expect(tauri.listen.mock.calls.map(([name]) => name)).toEqual([
      "local-drop-grant",
      "local-drop-grant-error",
    ]);
    tauri.eventHandlers.get("local-drop-grant")?.({
      payload: { grantId: "opaque-grant" },
    });
    tauri.eventHandlers.get("local-drop-grant-error")?.({
      payload: "grant failed",
    });
    tauri.dragHandler?.({
      payload: {
        type: "drop",
        paths: ["/raw/path/must/not/escape"],
        position: { x: 1, y: 2 },
      },
    });

    expect(onGrant).toHaveBeenCalledWith({ grantId: "opaque-grant" });
    expect(onGrantError).toHaveBeenCalledWith("grant failed");
    expect(onDragState).toHaveBeenCalledWith({ type: "drop" });
    expect(JSON.stringify(onDragState.mock.calls)).not.toContain(
      "/raw/path/must/not/escape",
    );

    cleanup();
    expect(tauri.unlistenGrant).toHaveBeenCalledOnce();
    expect(tauri.unlistenError).toHaveBeenCalledOnce();
    expect(tauri.unlistenDrag).toHaveBeenCalledOnce();
  });
});
