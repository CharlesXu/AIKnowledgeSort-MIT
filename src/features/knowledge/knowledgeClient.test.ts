import { describe, expect, test, vi } from "vitest";
import {
  createBrowserKnowledgeClient,
  createTauriKnowledgeClient,
} from "./knowledgeClient";

describe("knowledge client", () => {
  test("invokes only the two explicit native knowledge boundaries", async () => {
    const invoke = vi.fn().mockResolvedValue({ revision: 0 });
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

    await client.openDocument(openRequest);
    await client.saveDocument(saveRequest);

    expect(invoke).toHaveBeenNthCalledWith(1, "open_knowledge_document", {
      request: openRequest,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "save_knowledge_document", {
      request: saveRequest,
    });
  });

  test("never simulates Vault knowledge persistence in a browser", async () => {
    const client = createBrowserKnowledgeClient();
    const request = {
      authorityId: "vault-authority",
      operationId: "archive-operation",
    };

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
