import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import type { DiscoveryClient } from "./discoveryClient";
import type { DiscoveryProposal } from "./types";

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

import { tauriNativeDropBridge, useNativeDrop } from "./useNativeDrop";

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

  test("waits for a trusted grant after a mixed overlapping native drop", async () => {
    const deduplicatedProposal: DiscoveryProposal = {
      proposalId: "mixed-overlap-proposal",
      items: [
        {
          itemId: "mixed-readme",
          path: "/trusted/project/README.md",
          name: "README.md",
          byteSize: 128,
          identity: {
            algorithm: "SHA-256",
            digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0",
          },
        },
        {
          itemId: "mixed-day-one",
          path: "/trusted/project/notes/day-one.txt",
          name: "day-one.txt",
          byteSize: 256,
          identity: {
            algorithm: "SHA-256",
            digest: "ab5f329afb80f567b441324ad2d048ca910644b17c7426f9cc585307c5077496",
          },
        },
      ],
      counts: {
        included: 2,
        excluded: 1,
        unreadable: 1,
        symlink: 1,
        outOfScope: 1,
      },
      diagnostics: [],
    };
    const proposeLocalDrop = vi.fn(async ({ grantId }) => {
      expect(grantId).toBe("mixed-overlap-grant");
      return deduplicatedProposal;
    });
    const discoveryClient: DiscoveryClient = { proposeLocalDrop };
    const { result } = renderHook(() =>
      useNativeDrop({
        bridge: tauriNativeDropBridge,
        discoveryClient,
      }),
    );

    await waitFor(() => expect(tauri.onDragDropEvent).toHaveBeenCalledOnce());
    act(() => {
      tauri.dragHandler?.({
        payload: {
          type: "drop",
          paths: [
            "/raw/project",
            "/raw/project/README.md",
            "/raw/project/notes",
            "/raw/project/notes/day-one.txt",
          ],
          position: { x: 8, y: 13 },
        },
      });
    });

    expect(proposeLocalDrop).not.toHaveBeenCalled();
    expect(JSON.stringify(result.current)).not.toContain("/raw/project");

    act(() => {
      tauri.eventHandlers.get("local-drop-grant")?.({
        payload: { grantId: "mixed-overlap-grant" },
      });
    });

    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(proposeLocalDrop).toHaveBeenCalledOnce();
    expect(proposeLocalDrop).toHaveBeenCalledWith({
      grantId: "mixed-overlap-grant",
    });
    expect(result.current.proposal?.items).toEqual(
      deduplicatedProposal.items,
    );
    expect(result.current.proposal?.counts).toEqual({
      included: 2,
      excluded: 1,
      unreadable: 1,
      symlink: 1,
      outOfScope: 1,
    });
  });
});
