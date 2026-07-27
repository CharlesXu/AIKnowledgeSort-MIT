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
  proposalId: "trusted-proposal",
  items: [{
    itemId: "trusted-result",
    path: "/trusted/result.md",
    name: "result.md",
    byteSize: 12,
    identity: {
      algorithm: "SHA-256",
      digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0",
    },
  }],
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
    emitDrag(type: "over" | "drop" | "cancel") {
      callbacks?.onDragState({ type });
    },
  };
}

function createClient() {
  const proposeLocalDrop = vi.fn().mockResolvedValue(proposal);
  const client: DiscoveryClient = { proposeLocalDrop };
  return { client, proposeLocalDrop };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function proposalNamed(name: string): DiscoveryProposal {
  return {
    ...proposal,
    proposalId: `proposal-${name}`,
    items: [{
      itemId: `item-${name}`,
      path: `/trusted/${name}`,
      name,
      byteSize: 12,
      identity: proposal.items[0].identity,
    }],
  };
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

  test("keeps the newest grant result when requests resolve out of order", async () => {
    const native = createBridge();
    const grantA = deferred<DiscoveryProposal>();
    const grantB = deferred<DiscoveryProposal>();
    const discoveryClient: DiscoveryClient = {
      proposeLocalDrop: vi.fn(({ grantId }) =>
        grantId === "grant-a" ? grantA.promise : grantB.promise,
      ),
    };
    const { result } = renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient,
      }),
    );

    act(() => {
      native.emitGrant("grant-a");
      native.emitGrant("grant-b");
    });
    await act(async () => grantB.resolve(proposalNamed("newest.md")));
    await waitFor(() =>
      expect(result.current.proposal?.items[0]?.name).toBe("newest.md"),
    );

    await act(async () => grantA.resolve(proposalNamed("stale.md")));

    expect(result.current.status).toBe("ready");
    expect(result.current.proposal?.items[0]?.name).toBe("newest.md");
  });

  test("ignores an old client promise after bridge and client dependencies change", async () => {
    const oldNative = createBridge();
    const newNative = createBridge();
    const oldRequest = deferred<DiscoveryProposal>();
    const oldClient: DiscoveryClient = {
      proposeLocalDrop: vi.fn(() => oldRequest.promise),
    };
    const newClient: DiscoveryClient = {
      proposeLocalDrop: vi.fn().mockResolvedValue(proposalNamed("new.md")),
    };
    const { rerender, result } = renderHook(
      ({ bridge, discoveryClient }) =>
        useNativeDrop({ bridge, discoveryClient }),
      {
        initialProps: {
          bridge: oldNative.bridge,
          discoveryClient: oldClient,
        },
      },
    );

    act(() => oldNative.emitGrant("old-grant"));
    rerender({
      bridge: newNative.bridge,
      discoveryClient: newClient,
    });
    act(() => newNative.emitGrant("new-grant"));
    await waitFor(() =>
      expect(result.current.proposal?.items[0]?.name).toBe("new.md"),
    );

    await act(async () => oldRequest.resolve(proposalNamed("old.md")));

    expect(result.current.status).toBe("ready");
    expect(result.current.proposal?.items[0]?.name).toBe("new.md");
  });

  test("ignores late error and drag callbacks from a disposed subscription", () => {
    const oldNative = createBridge();
    const newNative = createBridge();
    const discovery = createClient();
    const { rerender, result } = renderHook(
      ({ bridge }) =>
        useNativeDrop({
          bridge,
          discoveryClient: discovery.client,
        }),
      { initialProps: { bridge: oldNative.bridge } },
    );

    rerender({ bridge: newNative.bridge });
    act(() => {
      oldNative.emitGrantError("stale native error");
      oldNative.emitDrag("over");
    });

    expect(result.current.status).toBe("idle");
    expect(result.current.message).toBe("");
  });

  test("ignores late error and drag callbacks after unmount", () => {
    const native = createBridge();
    const discovery = createClient();
    const rendered = renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );
    const snapshot = rendered.result.current;

    rendered.unmount();
    act(() => {
      native.emitGrantError("late native error");
      native.emitDrag("over");
    });

    expect(rendered.result.current).toBe(snapshot);
  });

  test("uses native drag callbacks only for hover and cancel visual state", async () => {
    const native = createBridge();
    const discovery = createClient();
    const { result } = renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );

    act(() => native.emitDrag("over"));
    expect(result.current.status).toBe("hovering");

    act(() => native.emitDrag("cancel"));
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

  test("never discovers from an untrusted native drop callback", () => {
    const native = createBridge();
    const discovery = createClient();
    renderHook(() =>
      useNativeDrop({
        bridge: native.bridge,
        discoveryClient: discovery.client,
      }),
    );

    act(() => native.emitDrag("drop"));

    expect(discovery.proposeLocalDrop).not.toHaveBeenCalled();
  });
});
