import { describe, expect, test, vi } from "vitest";
import {
  createMemoryDiscoveryClient,
  createTauriDiscoveryClient,
} from "./discoveryClient";
import type { DiscoveryProposal } from "./types";

const fixture: DiscoveryProposal = {
  items: [
    {
      path: "/grant/a.txt",
      name: "a.txt",
      byteSize: 3,
    },
  ],
  counts: {
    included: 1,
    excluded: 1,
    unreadable: 0,
    symlink: 0,
    outOfScope: 0,
  },
  diagnostics: [
    {
      category: "excluded",
      path: "/grant/missing.txt",
      message: "Path does not exist",
    },
  ],
};

describe("DiscoveryClient", () => {
  test("Tauri adapter invokes only the scoped proposal command", async () => {
    const invoke = vi.fn().mockResolvedValue(fixture);
    const client = createTauriDiscoveryClient(invoke);

    const proposal = await client.proposeLocalDrop({
      droppedPaths: ["/grant/a.txt"],
      grantedRoots: ["/grant"],
    });

    expect(proposal).toEqual(fixture);
    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("propose_local_drop", {
      droppedPaths: ["/grant/a.txt"],
      grantedRoots: ["/grant"],
    });
  });

  test("memory adapter returns deterministic isolated fixture copies", async () => {
    const client = createMemoryDiscoveryClient(fixture);
    const request = {
      droppedPaths: ["/grant/a.txt"],
      grantedRoots: ["/grant"],
    };

    const first = await client.proposeLocalDrop(request);
    const second = await client.proposeLocalDrop(request);

    expect(first).toEqual(fixture);
    expect(second).toEqual(fixture);
    expect(first).not.toBe(fixture);
    expect(first).not.toBe(second);
    expect(first.items).not.toBe(second.items);
    expect(request).toEqual({
      droppedPaths: ["/grant/a.txt"],
      grantedRoots: ["/grant"],
    });
  });
});
