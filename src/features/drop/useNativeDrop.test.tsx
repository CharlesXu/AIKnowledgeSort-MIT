import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { DiscoveryClient } from "./discoveryClient";
import type { DiscoveryProposal } from "./types";
import {
  useNativeDrop,
  type NativeDropBridge,
  type NativeDropCallbacks,
} from "./useNativeDrop";

const proposal: DiscoveryProposal = {
  items: [{ path: "/trusted/result.md", name: "result.md", byteSize: 12 }],
  counts: {
    included: 1,
    excluded: 2,
    unreadable: 3,
    symlink: 4,
    outOfScope: 5,
  },
  diagnostics: [],
};

function createBridge() {
  let callbacks: NativeDropCallbacks | undefined;
  const cleanup = vi.fn();
  const bridge: NativeDropBridge = {
    subscribe(nextCallbacks) {
      callbacks = nextCallbacks;
      return Promise.resolve(cleanup);
    },
  };

  return {
    bridge,
    cleanup,
    emitGrant(grantId: string) {
      callbacks?.onGrant({ grantId });
    },
    emitGrantError(message: string) {
      callbacks?.onGrantError(message);
    },
    emitDrag(type: "over" | "drop" | "cancel", _paths: readonly string[] = []) {
      callbacks?.onDragState({ type });
    },
  };
}

function createClient() {
  const proposeLocalDrop = vi.fn().mockResolvedValue(proposal);
  const client: DiscoveryClient = { proposeLocalDrop };
  return { client, proposeLocalDrop };
}

describe("useNativeDrop", () => {
  test("discovers only from the opaque grant event", async () => {
    const native = createBridge();
    const discovery = createClient();
    const { result } = renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );

    act(() => native.emitGrant("trusted-grant"));

    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(discovery.proposeLocalDrop).toHaveBeenCalledWith({
      grantId: "trusted-grant",
    });
    expect(result.current.proposal).toEqual(proposal);
  });

  test("ignores duplicate grant ids", async () => {
    const native = createBridge();
    const discovery = createClient();
    renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );

    act(() => {
      native.emitGrant("same-grant");
      native.emitGrant("same-grant");
    });

    await waitFor(() =>
      expect(discovery.proposeLocalDrop).toHaveBeenCalledTimes(1),
    );
  });

  test("bounds native and discovery errors for display", async () => {
    const native = createBridge();
    const discovery = createClient();
    discovery.proposeLocalDrop.mockRejectedValueOnce(
      new Error(`private detail ${"x".repeat(500)}`),
    );
    const { result } = renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );

    act(() => native.emitGrant("failing-grant"));

    await waitFor(() => expect(result.current.status).toBe("error"));
    expect(result.current.message.length).toBeLessThanOrEqual(240);

    act(() => native.emitGrantError(`native detail ${"y".repeat(500)}`));
    expect(result.current.status).toBe("error");
    expect(result.current.message.length).toBeLessThanOrEqual(240);
  });

  test("cleans the bridge subscription on unmount", async () => {
    const native = createBridge();
    const discovery = createClient();
    const rendered = renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );

    await waitFor(() => expect(native.cleanup).not.toHaveBeenCalled());
    rendered.unmount();

    await waitFor(() => expect(native.cleanup).toHaveBeenCalledOnce());
  });

  test("uses webview paths only for hover and cancel visual state", async () => {
    const native = createBridge();
    const discovery = createClient();
    const { result } = renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );

    act(() => native.emitDrag("over", ["/must/not/be/invoked"]));
    expect(result.current.status).toBe("hovering");

    act(() => native.emitDrag("cancel", ["/still/not/invoked"]));
    expect(result.current.status).toBe("idle");
    expect(discovery.proposeLocalDrop).not.toHaveBeenCalled();
  });

  test.each([
    ["text/plain", "https://example.test/from-plain-text"],
    ["text/uri-list", "https://example.test/from-uri-list"],
  ])("ignores external %s DOM drops", (mimeType, value) => {
    const native = createBridge();
    const discovery = createClient();
    const { result } = renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );
    const preventDefault = vi.fn();

    act(() =>
      result.current.onDomDrop({
        dataTransfer: {
          files: { length: 0 },
          types: [mimeType],
          getData: (type) => (type === mimeType ? value : ""),
        },
        preventDefault,
      }),
    );

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(result.current.status).toBe("ignored");
    expect(result.current.message).toMatch(/ignored/i);
    expect(discovery.proposeLocalDrop).not.toHaveBeenCalled();
  });

  test("never uses raw webview drop paths as a discovery request", () => {
    const native = createBridge();
    const discovery = createClient();
    renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );

    act(() => native.emitDrag("drop", ["/secret/raw/path"]));

    expect(discovery.proposeLocalDrop).not.toHaveBeenCalled();
  });
});
