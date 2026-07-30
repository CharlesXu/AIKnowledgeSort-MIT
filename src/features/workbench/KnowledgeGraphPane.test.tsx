import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { I18nProvider } from "../../i18n/I18nContext";
import type { GraphClient, GraphSnapshot } from "../graph/types";
import type { KnowledgeDocument } from "../knowledge/types";
import { KnowledgeGraphPane } from "./KnowledgeGraphPane";

const identity = {
  algorithm: "SHA-256" as const,
  digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0",
};

const document: KnowledgeDocument = {
  documentId: "knowledge-document-1",
  authorityId: "vault-1",
  operationId: "operation-1",
  revision: 2,
  markdownPath: "Knowledge/MCU reset reliability.md",
  markdown: "# MCU reset reliability\nClock stabilization is required.",
  savedAtUnixMs: 2_000,
  markdownIdentity: identity,
  originalIdentity: identity,
};

const snapshot: GraphSnapshot = {
  authorityId: "vault-1",
  operationId: "operation-1",
  relations: [{
    relationId: "a".repeat(32),
    version: 1,
    authorityId: "vault-1",
    operationId: "operation-1",
    knowledgeRevision: 2,
    sourceNode: "MCU reset",
    relationType: "requires",
    targetNode: "Clock stabilization",
    status: "review",
    evidence: [{
      operationId: "operation-1",
      knowledgeRevision: 2,
      startLine: 2,
      endLine: 2,
      text: "Clock stabilization is required.",
      markdownIdentity: identity,
      originalIdentity: identity,
    }],
    actor: "desktop-user",
    reason: "Extracted from committed Markdown",
    recordedAtUnixMs: 2_000,
  }],
  events: [{
    relationId: "a".repeat(32),
    version: 1,
    status: "review",
    sourceNode: "MCU reset",
    relationType: "requires",
    targetNode: "Clock stabilization",
    recordedAtUnixMs: 2_000,
  }],
};

function client(): GraphClient {
  return {
    inspect: vi.fn().mockResolvedValue(snapshot),
    propose: vi.fn().mockResolvedValue(snapshot.relations[0]),
    decide: vi.fn().mockResolvedValue(snapshot.relations[0]),
  };
}

describe("KnowledgeGraphPane", () => {
  test("renders graph controls and evidence review in Simplified Chinese", async () => {
    render(
      <I18nProvider initialLanguage="zh-CN">
        <KnowledgeGraphPane client={client()} document={document} />
      </I18nProvider>,
    );

    expect(await screen.findByText("证据图谱")).toBeInTheDocument();
    expect(screen.getByText("Vault 修订版本 2")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "添加关系" }))
      .toBeInTheDocument();
    expect(screen.getByLabelText("知识图谱时间轴位置")).toBeInTheDocument();
    expect(screen.queryByText("Evidence graph")).toBeNull();
  });

  test("shows persisted relation evidence and sends an exact review decision", async () => {
    const graphClient = client();
    render(<KnowledgeGraphPane client={graphClient} document={document} />);

    const relation = await screen.findByRole("button", {
      name: /MCU reset requires Clock stabilization/,
    });
    fireEvent.click(relation);

    expect(screen.getByText("Clock stabilization is required.")).toBeInTheDocument();
    expect(screen.getByText("Lines 2–2")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Relation decision reason"), {
      target: { value: "Evidence verified" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Accept" }));

    await waitFor(() => expect(graphClient.decide).toHaveBeenCalledWith({
      authorityId: "vault-1",
      relationId: "a".repeat(32),
      expectedVersion: 1,
      decision: "accept",
      reason: "Evidence verified",
      revision: null,
    }));
  });

  test("proposes only node fields and committed Markdown line ranges", async () => {
    const graphClient = client();
    render(<KnowledgeGraphPane client={graphClient} document={document} />);
    await screen.findByText("1 relations");

    fireEvent.change(screen.getByLabelText("Relation source node"), {
      target: { value: "Brown-out threshold" },
    });
    fireEvent.change(screen.getByLabelText("Relation type"), {
      target: { value: "protects" },
    });
    fireEvent.change(screen.getByLabelText("Relation target node"), {
      target: { value: "Boot state" },
    });
    fireEvent.change(screen.getByLabelText("Evidence start line"), {
      target: { value: "2" },
    });
    fireEvent.change(screen.getByLabelText("Evidence end line"), {
      target: { value: "2" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add relation" }));

    await waitFor(() => expect(graphClient.propose).toHaveBeenCalledWith({
      authorityId: "vault-1",
      operationId: "operation-1",
      knowledgeRevision: 2,
      sourceNode: "Brown-out threshold",
      relationType: "protects",
      targetNode: "Boot state",
      evidenceRanges: [{ startLine: 2, endLine: 2 }],
    }));
  });

  test("keeps verified evidence visible when a revision decision fails", async () => {
    const graphClient = client();
    vi.mocked(graphClient.decide).mockRejectedValue(
      new Error("Graph relation version changed; inspect before deciding"),
    );
    render(<KnowledgeGraphPane client={graphClient} document={document} />);

    fireEvent.click(await screen.findByRole("button", {
      name: /MCU reset requires Clock stabilization/,
    }));
    fireEvent.change(screen.getByLabelText("Relation type"), {
      target: { value: "depends on" },
    });
    fireEvent.change(screen.getByLabelText("Relation decision reason"), {
      target: { value: "Clarify the claim" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Revise" }));

    await waitFor(() => expect(graphClient.decide).toHaveBeenCalledWith({
      authorityId: "vault-1",
      relationId: "a".repeat(32),
      expectedVersion: 1,
      decision: "revise",
      reason: "Clarify the claim",
      revision: {
        knowledgeRevision: 2,
        sourceNode: "MCU reset",
        relationType: "depends on",
        targetNode: "Clock stabilization",
        evidenceRanges: [{ startLine: 2, endLine: 2 }],
      },
    }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/version changed/i);
    expect(screen.getByText("Clock stabilization is required.")).toBeInTheDocument();
  });

  test("keeps graph history playback in the compact timeline", async () => {
    render(<KnowledgeGraphPane client={client()} document={document} />);
    await screen.findByText("1 relations");

    expect(screen.getByLabelText("Knowledge timeline position")).toHaveValue("1");
    expect(screen.getByLabelText("Play knowledge timeline")).toBeEnabled();
    expect(screen.getByLabelText("Knowledge timeline position").parentElement)
      .toHaveAttribute("data-height", "34");
  });
});
