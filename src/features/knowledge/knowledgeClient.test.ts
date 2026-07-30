import { describe, expect, test, vi } from "vitest";
import {
  createBrowserKnowledgeClient,
  createTauriKnowledgeClient,
} from "./knowledgeClient";

describe("knowledge client", () => {
  test("invokes only the three explicit native knowledge boundaries", async () => {
    const target = {
      authorityId: "vault-authority",
      operationId: "archive-operation",
      itemId: "reviewed-item",
      destinationPath: "Originals/abc/Reviewed.md",
      originalIdentity: {
        algorithm: "SHA-256",
        digest: "a".repeat(64),
      },
    };
    const invoke = vi
      .fn()
      .mockResolvedValueOnce([target])
      .mockResolvedValue({ revision: 0 });
    const client = createTauriKnowledgeClient(invoke);
    const openRequest = {
      authorityId: "vault-authority",
      operationId: "archive-operation",
    };
    const saveRequest = {
      ...openRequest,
      expectedRevision: 0,
      markdown: "# Note\n",
    };

    await expect(client.listTargets({
      authorityId: "vault-authority",
    })).resolves.toEqual([target]);
    await client.openDocument(openRequest);
    await client.saveDocument(saveRequest);

    expect(invoke).toHaveBeenNthCalledWith(1, "list_knowledge_targets", {
      request: { authorityId: "vault-authority" },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "open_knowledge_document", {
      request: openRequest,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "save_knowledge_document", {
      request: saveRequest,
    });
  });

  test("never simulates Vault knowledge persistence in a browser", async () => {
    const client = createBrowserKnowledgeClient();
    const request = {
      authorityId: "vault-authority",
      operationId: "archive-operation",
    };

    await expect(client.listTargets({
      authorityId: "vault-authority",
    })).rejects.toThrow("Desktop runtime is required for knowledge operations.");
    await expect(client.openDocument(request)).rejects.toThrow(
      "Desktop runtime is required for knowledge operations.",
    );
    await expect(client.saveDocument({
      ...request,
      expectedRevision: 0,
      markdown: "# Note\n",
    })).rejects.toThrow("Desktop runtime is required for knowledge operations.");
  });
});
