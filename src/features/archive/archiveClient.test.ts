import { describe, expect, test, vi } from "vitest";
import {
  createBrowserArchiveClient,
  createTauriArchiveClient,
} from "./archiveClient";

describe("archive client", () => {
  test("invokes only the three explicit native archive boundaries", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce({
        authorityId: "vault-1",
        displayPath: "/Vault",
        status: "authoritative",
      })
      .mockResolvedValueOnce({ planId: "plan-1" })
      .mockResolvedValueOnce({ planId: "plan-1", status: "committed", items: [] });
    const client = createTauriArchiveClient(invoke);

    await client.chooseVault();
    await client.createPlan({
      proposalId: "proposal-1",
      itemIds: ["item-1"],
    });
    await client.confirmPlan({
      planId: "plan-1",
      confirmationNonce: "nonce-1",
    });

    expect(invoke.mock.calls).toEqual([
      ["choose_authoritative_vault"],
      [
        "create_archive_plan",
        {
          request: {
            proposalId: "proposal-1",
            itemIds: ["item-1"],
          },
        },
      ],
      [
        "confirm_archive_plan",
        {
          request: {
            planId: "plan-1",
            confirmationNonce: "nonce-1",
          },
        },
      ],
    ]);
  });

  test("never simulates archive mutation in a browser", async () => {
    const client = createBrowserArchiveClient();

    await expect(client.chooseVault()).rejects.toThrow(
      /desktop runtime is required/i,
    );
    await expect(
      client.createPlan({ proposalId: "proposal-1", itemIds: ["item-1"] }),
    ).rejects.toThrow(/desktop runtime is required/i);
    await expect(
      client.confirmPlan({
        planId: "plan-1",
        confirmationNonce: "nonce-1",
      }),
    ).rejects.toThrow(/desktop runtime is required/i);
  });
});
