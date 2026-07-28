import { describe, expect, test, vi } from "vitest";
import {
  createBrowserModelRuntimeClient,
  createTauriModelRuntimeClient,
} from "./modelRuntimeClient";

describe("model runtime client", () => {
  test("invokes only the four explicit native model boundaries", async () => {
    const invoke = vi.fn().mockResolvedValue({ schemaVersion: 1, configs: [] });
    const client = createTauriModelRuntimeClient(invoke);
    const config = {
      configId: "local-ollama",
      label: "Local Ollama",
      location: "local" as const,
      endpointUrl: "http://127.0.0.1:11434/v1/chat/completions",
      model: "qwen3:8b",
      timeoutMs: 30_000,
      authenticated: false,
    };
    const comparison = {
      authorityId: "vault-1",
      operationId: "operation-1",
      knowledgeRevision: 2,
      evidenceRanges: [{ startLine: 3, endLine: 5 }],
      desktopConfigId: "local-ollama",
      agentConfigId: "remote-reasoner",
    };

    await client.inspect();
    await client.upsert(config);
    await client.remove({ configId: config.configId });
    await client.runComparison(comparison);

    expect(invoke.mock.calls).toEqual([
      ["inspect_model_runtime"],
      ["upsert_model_config", { request: config }],
      ["remove_model_config", { request: { configId: "local-ollama" } }],
      ["run_model_comparison", { request: comparison }],
    ]);
  });

  test("never simulates model runtime state or decisions in a browser", async () => {
    const client = createBrowserModelRuntimeClient();
    const expected = "Desktop runtime is required for model runtime operations.";
    await expect(client.inspect()).rejects.toThrow(expected);
    await expect(client.upsert({
      configId: "local",
      label: "Local",
      location: "local",
      endpointUrl: "http://127.0.0.1/v1/chat/completions",
      model: "model",
      timeoutMs: 1_000,
      authenticated: false,
    })).rejects.toThrow(expected);
    await expect(client.remove({ configId: "local" })).rejects.toThrow(expected);
    await expect(client.runComparison({
      authorityId: "vault",
      operationId: "operation",
      knowledgeRevision: 1,
      evidenceRanges: [{ startLine: 1, endLine: 1 }],
      desktopConfigId: "desktop",
      agentConfigId: "agent",
    })).rejects.toThrow(expected);
  });
});
