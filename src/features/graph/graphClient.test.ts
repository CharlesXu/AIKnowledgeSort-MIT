import { describe, expect, test, vi } from "vitest";
import { createBrowserGraphClient, createTauriGraphClient } from "./graphClient";

describe("graph client", () => {
  test("invokes only the four explicit native graph boundaries", async () => {
    const invoke = vi.fn().mockResolvedValue({ relations: [], events: [] });
    const client = createTauriGraphClient(invoke);
    const inspect = { authorityId: "vault", operationId: "operation" };
    const proposal = {
      ...inspect,
      knowledgeRevision: 1,
      sourceNode: "MCU reset",
      relationType: "requires",
      targetNode: "Clock stabilization",
      evidenceRanges: [{ startLine: 2, endLine: 3 }],
    };
    const decision = {
      authorityId: "vault",
      relationId: "a".repeat(32),
      expectedVersion: 1,
      decision: "accept" as const,
      reason: "Evidence verified",
      revision: null,
    };
    const comparison = {
      authorityId: "vault",
      comparisonId: "b".repeat(32),
    };

    await client.inspect(inspect);
    await client.propose(proposal);
    await client.importComparison(comparison);
    await client.decide(decision);

    expect(invoke).toHaveBeenNthCalledWith(1, "inspect_knowledge_graph", { request: inspect });
    expect(invoke).toHaveBeenNthCalledWith(2, "propose_graph_relation", { request: proposal });
    expect(invoke).toHaveBeenNthCalledWith(3, "import_comparison_relations", { request: comparison });
    expect(invoke).toHaveBeenNthCalledWith(4, "decide_graph_relation", { request: decision });
  });

  test("never simulates graph persistence in a browser", async () => {
    const client = createBrowserGraphClient();
    const request = { authorityId: "vault", operationId: "operation" };
    await expect(client.inspect(request)).rejects.toThrow(
      "Desktop runtime is required for graph operations.",
    );
    await expect(client.propose({
      ...request,
      knowledgeRevision: 1,
      sourceNode: "Source",
      relationType: "relates to",
      targetNode: "Target",
      evidenceRanges: [{ startLine: 1, endLine: 1 }],
    })).rejects.toThrow("Desktop runtime is required for graph operations.");
    await expect(client.importComparison({
      authorityId: "vault",
      comparisonId: "b".repeat(32),
    })).rejects.toThrow("Desktop runtime is required for graph operations.");
    await expect(client.decide({
      authorityId: "vault",
      relationId: "a".repeat(32),
      expectedVersion: 1,
      decision: "reject",
      reason: "Not supported",
      revision: null,
    })).rejects.toThrow("Desktop runtime is required for graph operations.");
  });
});
