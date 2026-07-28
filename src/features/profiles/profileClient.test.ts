import { describe, expect, test, vi } from "vitest";
import {
  createBrowserProfileClient,
  createTauriProfileClient,
} from "./profileClient";

describe("profile client", () => {
  test("invokes only the three explicit native profile boundaries", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce({ installed: [], active: null, candidates: [] })
      .mockResolvedValueOnce({ candidateId: "candidate-1" })
      .mockResolvedValueOnce({ installed: [], active: null, candidates: [] });
    const client = createTauriProfileClient(invoke);

    await client.inspect();
    await client.importLocalCandidate();
    await client.decideCandidate({
      candidateId: "candidate-1",
      reviewedDigest: "a".repeat(64),
      decision: "approve",
    });

    expect(invoke.mock.calls).toEqual([
      ["inspect_profile_state"],
      ["import_local_profile_candidate"],
      [
        "decide_profile_candidate",
        {
          request: {
            candidateId: "candidate-1",
            reviewedDigest: "a".repeat(64),
            decision: "approve",
          },
        },
      ],
    ]);
  });

  test("never simulates profile operations in a browser", async () => {
    const client = createBrowserProfileClient();

    await expect(client.inspect()).rejects.toThrow(
      "Desktop runtime is required for profile operations.",
    );
    await expect(client.importLocalCandidate()).rejects.toThrow(
      "Desktop runtime is required for profile operations.",
    );
    await expect(
      client.decideCandidate({
        candidateId: "candidate-1",
        reviewedDigest: "a".repeat(64),
        decision: "reject",
      }),
    ).rejects.toThrow(
      "Desktop runtime is required for profile operations.",
    );
  });
});
