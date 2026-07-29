import { describe, expect, test, vi } from "vitest";
import {
  createBrowserProfileClient,
  createTauriProfileClient,
} from "./profileClient";

describe("profile client", () => {
  test("invokes only the six explicit native profile boundaries", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce({ installed: [], active: null, candidates: [] })
      .mockResolvedValueOnce({ candidateId: "candidate-1" })
      .mockResolvedValueOnce({ candidateId: "candidate-2" })
      .mockResolvedValueOnce({ candidateId: "candidate-3" })
      .mockResolvedValueOnce({ installed: [], active: null, candidates: [] })
      .mockResolvedValueOnce({ batchId: "classification-batch-1" });
    const client = createTauriProfileClient(invoke);

    await client.inspect();
    await client.importLocalCandidate();
    await client.importUrlCandidate(
      "https://profiles.example.com/ninebot.json",
    );
    const compileRequest = {
      configId: "local-compiler",
      profileId: "ninebot-electronic-archive",
      version: "0.4.0-candidate",
      title: "Ninebot classification",
      sourceTitle: "Formal notice",
      ownership: "owned" as const,
      baseProfileId: "ninebot-electronic-archive",
      baseProfileVersion: "0.3.0-draft",
    };
    await client.compileLocalCandidate(compileRequest);
    await client.decideCandidate({
      candidateId: "candidate-1",
      reviewedDigest: "a".repeat(64),
      decision: "approve",
    });
    await client.createClassificationBatch({
      proposalId: "proposal-1",
      items: [
        {
          itemId: "item-1",
          references: [
            {
              kind: "documentText",
              location: "page:1",
              text: "MCU reset reliability",
            },
          ],
        },
      ],
    });

    expect(invoke.mock.calls).toEqual([
      ["inspect_profile_state"],
      ["import_local_profile_candidate"],
      [
        "import_url_profile_candidate",
        {
          request: {
            url: "https://profiles.example.com/ninebot.json",
          },
        },
      ],
      [
        "compile_local_profile_candidate",
        { request: compileRequest },
      ],
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
      [
        "create_classification_batch",
        {
          request: {
            proposalId: "proposal-1",
            items: [
              {
                itemId: "item-1",
                references: [
                  {
                    kind: "documentText",
                    location: "page:1",
                    text: "MCU reset reliability",
                  },
                ],
              },
            ],
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
      client.importUrlCandidate(
        "https://profiles.example.com/ninebot.json",
      ),
    ).rejects.toThrow(
      "Desktop runtime is required for profile operations.",
    );
    await expect(client.compileLocalCandidate({
      configId: "local-compiler",
      profileId: "ninebot-electronic-archive",
      version: "0.4.0-candidate",
      title: "Ninebot classification",
      sourceTitle: "Formal notice",
      ownership: "owned",
      baseProfileId: "ninebot-electronic-archive",
      baseProfileVersion: "0.3.0-draft",
    })).rejects.toThrow(
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
    await expect(
      client.createClassificationBatch({
        proposalId: "proposal-1",
        items: [
          {
            itemId: "item-1",
            references: [
              {
                kind: "documentText",
                location: "page:1",
                text: "MCU reset reliability",
              },
            ],
          },
        ],
      }),
    ).rejects.toThrow(
      "Desktop runtime is required for profile operations.",
    );
  });
});
