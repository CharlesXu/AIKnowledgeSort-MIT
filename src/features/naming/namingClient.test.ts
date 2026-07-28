import { describe, expect, test, vi } from "vitest";
import {
  createBrowserNamingClient,
  createTauriNamingClient,
} from "./namingClient";

describe("naming client", () => {
  test("sends only proposal ids, item ids, and cited facts", async () => {
    const invoke = vi.fn().mockResolvedValue({ batchId: "batch-1" });
    const client = createTauriNamingClient(invoke);

    await client.createBatch({
      proposalId: "proposal-1",
      items: [
        {
          itemId: "item-1",
          facts: [
            {
              kind: "subject",
              value: "Reset reliability",
              evidenceLocation: "page:1",
            },
          ],
        },
      ],
    });

    expect(invoke).toHaveBeenCalledWith("create_naming_batch", {
      request: {
        proposalId: "proposal-1",
        items: [
          {
            itemId: "item-1",
            facts: [
              {
                kind: "subject",
                value: "Reset reliability",
                evidenceLocation: "page:1",
              },
            ],
          },
        ],
      },
    });
  });

  test("never simulates naming proposals in a browser", async () => {
    await expect(
      createBrowserNamingClient().createBatch({
        proposalId: "proposal-1",
        items: [],
      }),
    ).rejects.toThrow(
      "Desktop runtime is required for naming operations.",
    );
  });
});
