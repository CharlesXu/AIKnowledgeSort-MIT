import { describe, expect, test, vi } from "vitest";
import {
  createMemoryDiscoveryClient,
  createTauriDiscoveryClient,
} from "./discoveryClient";
import type { DiscoveryProposal } from "./types";

const fixture: DiscoveryProposal = {
  proposalId: "fixture-proposal",
  items: [
    {
      itemId: "fixture-a",
      path: "/grant/a.txt",
      name: "a.txt",
      byteSize: 3,
      identity: {
        algorithm: "SHA-256",
        digest: "7692c3ad3540bb803c020b3aee66cd8887123234ea0c6e7143c0add73ff431ed",
      },
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
      grantId: "opaque-grant-id",
    });

    expect(proposal).toEqual(fixture);
    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("propose_local_drop", {
      grantId: "opaque-grant-id",
    });
  });

  test("memory adapter returns deterministic isolated fixture copies", async () => {
    const client = createMemoryDiscoveryClient(fixture);
    const request = {
      grantId: "fixture-grant",
    };

    const first = await client.proposeLocalDrop(request);
    const second = await client.proposeLocalDrop(request);

    expect(first).toEqual(fixture);
    expect(second).toEqual(fixture);
    expect(first).not.toBe(fixture);
    expect(first).not.toBe(second);
    expect(first.items).not.toBe(second.items);
    expect(request).toEqual({ grantId: "fixture-grant" });
  });
});
