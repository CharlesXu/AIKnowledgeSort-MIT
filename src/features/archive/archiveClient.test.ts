import { describe, expect, test, vi } from "vitest";
import {
  createBrowserArchiveClient,
  createTauriArchiveClient,
} from "./archiveClient";

describe("archive client", () => {
  test("invokes the explicit native archive and cleanup boundaries", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce({
        authorityId: "vault-1",
        displayPath: "/Vault",
        status: "authoritative",
      })
      .mockResolvedValueOnce({ planId: "plan-1" })
      .mockResolvedValueOnce({ planId: "plan-1", status: "committed", items: [] })
      .mockResolvedValueOnce({ planId: "cleanup-1", disposition: "trash" })
      .mockResolvedValueOnce({
        planId: "cleanup-2",
        disposition: "permanentDelete",
      })
      .mockResolvedValueOnce({ planId: "cleanup-1", status: "committed" });
    const client = createTauriArchiveClient(invoke);

    await client.chooseVault();
    await client.createPlan({
      proposalId: "proposal-1",
      itemIds: ["item-1"],
      namingBatchId: "naming-batch-1",
    });
    await client.confirmPlan({
      planId: "plan-1",
      confirmationNonce: "nonce-1",
    });
    await client.createCleanupPlan({
      authorityId: "vault-1",
      operationIds: ["operation-1"],
      cleanupEnabled: true,
    });
    await client.authorizePermanentCleanup({
      planId: "cleanup-1",
      confirmationNonce: "cleanup-nonce-1",
    });
    await client.confirmCleanupPlan({
      planId: "cleanup-2",
      confirmationNonce: "cleanup-nonce-2",
    });

    expect(invoke.mock.calls).toEqual([
      ["choose_authoritative_vault"],
      [
        "create_archive_plan",
        {
          request: {
            proposalId: "proposal-1",
            itemIds: ["item-1"],
            namingBatchId: "naming-batch-1",
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
      [
        "create_cleanup_plan",
        {
          request: {
            authorityId: "vault-1",
            operationIds: ["operation-1"],
            cleanupEnabled: true,
          },
        },
      ],
      [
        "authorize_permanent_cleanup",
        {
          request: {
            planId: "cleanup-1",
            confirmationNonce: "cleanup-nonce-1",
          },
        },
      ],
      [
        "confirm_cleanup_plan",
        {
          request: {
            planId: "cleanup-2",
            confirmationNonce: "cleanup-nonce-2",
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
      client.createPlan({
        proposalId: "proposal-1",
        itemIds: ["item-1"],
        namingBatchId: "naming-batch-1",
      }),
    ).rejects.toThrow(/desktop runtime is required/i);
    await expect(
      client.confirmPlan({
        planId: "plan-1",
        confirmationNonce: "nonce-1",
      }),
    ).rejects.toThrow(/desktop runtime is required/i);
    await expect(
      client.createCleanupPlan({
        authorityId: "vault-1",
        operationIds: ["operation-1"],
        cleanupEnabled: true,
      }),
    ).rejects.toThrow(/desktop runtime is required/i);
    await expect(
      client.authorizePermanentCleanup({
        planId: "cleanup-1",
        confirmationNonce: "cleanup-nonce-1",
      }),
    ).rejects.toThrow(/desktop runtime is required/i);
    await expect(
      client.confirmCleanupPlan({
        planId: "cleanup-1",
        confirmationNonce: "cleanup-nonce-1",
      }),
    ).rejects.toThrow(/desktop runtime is required/i);
  });
});
