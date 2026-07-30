import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { I18nProvider } from "../../i18n/I18nContext";
import type { KnowledgeDocument } from "../knowledge/types";
import type { ContentIdentity } from "../drop/types";
import { AgentReviewPane } from "./AgentReviewPane";
import type { ComparisonRecord, ModelRuntimeClient, ModelRuntimeState } from "./types";

const identity: ContentIdentity = {
  algorithm: "SHA-256",
  digest: "a".repeat(64),
};

const state: ModelRuntimeState = {
  schemaVersion: 1,
  configs: [
    {
      configId: "desktop-model",
      label: "Desktop Model",
      location: "local",
      endpointUrl: "http://127.0.0.1:11434/v1/chat/completions",
      model: "qwen3:8b",
      timeoutMs: 30_000,
      authenticated: false,
      providerProtocol: "openAi",
      credentialSource: "environment",
      credentialEnvironment: null,
      credentialStored: false,
    },
    {
      configId: "agent-model",
      label: "Agent Model",
      location: "remote",
      endpointUrl: "https://models.example.com/v1/chat/completions",
      model: "reasoner-v1",
      timeoutMs: 60_000,
      authenticated: true,
      providerProtocol: "openAi",
      credentialSource: "environment",
      credentialEnvironment: "AIKS_MODEL_API_KEY_AGENT_MODEL",
      credentialStored: false,
    },
  ],
};

const savedDocument: KnowledgeDocument = {
  documentId: "operation-1",
  authorityId: "vault-1",
  operationId: "operation-1",
  revision: 2,
  markdownPath: "Knowledge/operation-1/00000002.md",
  markdown: "# Note\nEvidence line.\n",
  savedAtUnixMs: 1_785_246_000_000,
  markdownIdentity: identity,
  originalIdentity: { ...identity, digest: "b".repeat(64) },
};

const record: ComparisonRecord = {
  schemaVersion: 1,
  comparisonId: "c".repeat(32),
  envelope: {
    schemaVersion: 1,
    task: "knowledgeRelations",
    originalIdentity: savedDocument.originalIdentity,
    markdownIdentity: identity,
    knowledgeRevision: 2,
    ruleSnapshot: {
      policyId: "knowledge-relations-v1",
      version: "1.0.0",
      identity: { ...identity, digest: "d".repeat(64) },
      json: "{}",
    },
    evidence: [{
      evidenceId: "line-2-2",
      startLine: 2,
      endLine: 2,
      text: "Evidence line.\n",
    }],
  },
  envelopeIdentity: { ...identity, digest: "e".repeat(64) },
  desktopConfigId: "desktop-model",
  agentConfigId: "agent-model",
  desktopOutcome: {
    status: "succeeded",
    model: "qwen3:8b",
    proposal: {
      summary: "Desktop proposal",
      relations: [{
        source: "MCU",
        relationType: "dependsOn",
        target: "Reset controller",
        evidenceIds: ["line-2-2"],
      }],
    },
    failureReason: null,
  },
  agentOutcome: {
    status: "succeeded",
    model: "reasoner-v1",
    proposal: {
      summary: "Agent proposal",
      relations: [{
        source: "MCU",
        relationType: "relatedTo",
        target: "Reset controller",
        evidenceIds: ["line-2-2"],
      }],
    },
    failureReason: null,
  },
  adjudication: {
    decision: "review",
    reason: "Relation types conflict",
    evidenceIds: ["line-2-2"],
    selectedSide: null,
    revisedRelations: [],
  },
  adjudicationFailure: null,
  status: "review",
  actor: "desktop-orchestrator",
  recordedAtUnixMs: 1_785_246_100_000,
};

function client(): ModelRuntimeClient {
  return {
    inspect: vi.fn().mockResolvedValue(state),
    discoverModels: vi.fn(),
    upsert: vi.fn(),
    remove: vi.fn(),
    runComparison: vi.fn().mockResolvedValue(record),
    runFileSemanticComparison: vi.fn(),
  };
}

describe("AgentReviewPane", () => {
  test("renders model comparison controls in Simplified Chinese", async () => {
    render(
      <I18nProvider initialLanguage="zh-CN">
        <AgentReviewPane client={client()} document={savedDocument} />
      </I18nProvider>,
    );

    expect(await screen.findByText("证据比对")).toBeInTheDocument();
    expect(screen.getByLabelText("桌面模型")).toBeInTheDocument();
    expect(screen.getByLabelText("Agent 模型")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "运行比对" }))
      .toBeInTheDocument();
    expect(screen.queryByText("Evidence comparison")).toBeNull();
  });

  test("requires one saved authoritative Vault revision", async () => {
    const modelClient = client();
    render(<AgentReviewPane client={modelClient} document={null} />);

    expect(await screen.findByText(/saved Vault revision is required/i))
      .toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Run comparison" })).toBeNull();
    expect(modelClient.runComparison).not.toHaveBeenCalled();
  });

  test("submits only exact authority, revision, ranges, and distinct config IDs", async () => {
    const modelClient = client();
    render(<AgentReviewPane client={modelClient} document={savedDocument} />);
    await screen.findByRole("button", { name: "Run comparison" });
    fireEvent.change(screen.getByLabelText("Start line"), { target: { value: "2" } });
    fireEvent.change(screen.getByLabelText("End line"), { target: { value: "2" } });
    fireEvent.click(screen.getByRole("button", { name: "Run comparison" }));

    await waitFor(() => expect(modelClient.runComparison).toHaveBeenCalledWith({
      authorityId: "vault-1",
      operationId: "operation-1",
      knowledgeRevision: 2,
      evidenceRanges: [{ startLine: 2, endLine: 2 }],
      desktopConfigId: "desktop-model",
      agentConfigId: "agent-model",
    }));
  });

  test("renders both proposals, evidence identity, and read-only Agent decision", async () => {
    render(<AgentReviewPane client={client()} document={savedDocument} />);
    fireEvent.click(await screen.findByRole("button", { name: "Run comparison" }));

    const results = await screen.findByRole("region", { name: "Model comparison result" });
    expect(results).toHaveTextContent("Desktop Model");
    expect(results).toHaveTextContent("qwen3:8b");
    expect(results).toHaveTextContent("Agent Model");
    expect(results).toHaveTextContent("reasoner-v1");
    expect(results).toHaveTextContent("Desktop proposal");
    expect(results).toHaveTextContent("Agent proposal");
    expect(results).toHaveTextContent("Relation types conflict");
    expect(results).toHaveTextContent("Evidence line.");
    expect(results).toHaveTextContent(record.envelopeIdentity.digest);
    expect(results).toHaveTextContent("Semantic advice · no operation authorized");
    for (const name of [/apply/i, /move/i, /rename/i, /delete/i, /cleanup/i, /write graph/i]) {
      expect(within(results).queryByRole("button", { name })).toBeNull();
    }
  });

  test("keeps the last audit result and line range after a later execution failure", async () => {
    const modelClient = client();
    const runComparison = vi.mocked(modelClient.runComparison);
    runComparison
      .mockResolvedValueOnce(record)
      .mockRejectedValueOnce(new Error("Provider unavailable"));
    render(<AgentReviewPane client={modelClient} document={savedDocument} />);
    fireEvent.change(await screen.findByLabelText("Start line"), {
      target: { value: "2" },
    });
    fireEvent.change(screen.getByLabelText("End line"), {
      target: { value: "2" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run comparison" }));
    expect(await screen.findByRole("region", { name: "Model comparison result" }))
      .toHaveTextContent("Relation types conflict");

    fireEvent.click(screen.getByRole("button", { name: "Run comparison" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Provider unavailable");
    expect(screen.getByRole("region", { name: "Model comparison result" }))
      .toHaveTextContent("Relation types conflict");
    expect(screen.getByLabelText("Start line")).toHaveValue(2);
    expect(screen.getByLabelText("End line")).toHaveValue(2);
  });
});
