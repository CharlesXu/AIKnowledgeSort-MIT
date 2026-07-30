import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { I18nProvider } from "../../i18n/I18nContext";
import type { KnowledgeClient, KnowledgeDocument, KnowledgeTarget } from "../knowledge/types";
import { DocumentPane } from "./DocumentPane";

const identity = {
  algorithm: "SHA-256" as const,
  digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
};

const target: KnowledgeTarget = {
  authorityId: "vault-1",
  operationId: "operation-1",
  itemId: "item-1",
  destinationPath: `Originals/${identity.digest}/Reset-reliability.md`,
  originalIdentity: identity,
};

function document(revision = 0, markdown = "# Reset reliability\n"): KnowledgeDocument {
  return {
    documentId: "operation-1",
    authorityId: "vault-1",
    operationId: "operation-1",
    revision,
    markdownPath: revision === 0 ? null : `Knowledge/operation-1/${String(revision).padStart(8, "0")}.md`,
    markdown,
    savedAtUnixMs: revision === 0 ? null : 1234,
    markdownIdentity: revision === 0 ? null : identity,
    originalIdentity: identity,
  };
}

function client(): KnowledgeClient {
  return {
    listTargets: vi.fn().mockResolvedValue([target]),
    openDocument: vi.fn().mockResolvedValue(document()),
    saveDocument: vi.fn().mockResolvedValue(document(1, "# Changed\n")),
  };
}

describe("DocumentPane", () => {
  test("renders the Markdown workspace controls in Simplified Chinese", () => {
    render(
      <I18nProvider initialLanguage="zh-CN">
        <DocumentPane client={client()} targets={[]} />
      </I18nProvider>,
    );

    expect(screen.getByRole("region", { name: "文档工作区" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "源码" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "实时预览" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "阅读" })).toBeInTheDocument();
    expect(screen.getByText("本地草稿 · 尚未保存")).toBeInTheDocument();
  });

  test("keeps an unarchived local draft outside Vault persistence", () => {
    const knowledgeClient = client();
    render(<DocumentPane client={knowledgeClient} targets={[]} />);

    expect(screen.getByText("Local draft · not saved")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Create knowledge note" })).toBeNull();
    expect(knowledgeClient.openDocument).not.toHaveBeenCalled();
    expect(knowledgeClient.saveDocument).not.toHaveBeenCalled();
  });

  test("opens only an eligible archive and saves one exact revision", async () => {
    const knowledgeClient = client();
    const onDocumentChange = vi.fn();
    const { rerender } = render(
      <DocumentPane
        client={knowledgeClient}
        onDocumentChange={onDocumentChange}
        targets={[target]}
      />,
    );

    expect(await screen.findByRole("combobox", {
      name: "Eligible archived original",
    })).toHaveValue("operation-1");
    fireEvent.click(screen.getByRole("button", { name: "Create knowledge note" }));

    await waitFor(() => expect(knowledgeClient.openDocument).toHaveBeenCalledWith({
      authorityId: "vault-1",
      operationId: "operation-1",
    }));
    const editor = screen.getByRole("textbox", {
      name: "Markdown, Mermaid, and code editor",
    });
    await waitFor(() => expect(editor).toHaveValue("# Reset reliability\n"));
    expect(await screen.findByText("New Vault note · not saved")).toBeInTheDocument();

    fireEvent.change(editor, { target: { value: "# Changed\n" } });
    fireEvent.click(screen.getByRole("tab", { name: "Live preview" }));
    expect(screen.getByRole("region", { name: "Document preview" }))
      .toHaveTextContent("Changed");
    fireEvent.click(screen.getByRole("button", { name: "Save Vault revision" }));

    await waitFor(() => expect(knowledgeClient.saveDocument).toHaveBeenCalledWith({
      authorityId: "vault-1",
      operationId: "operation-1",
      expectedRevision: 0,
      markdown: "# Changed\n",
    }));
    expect(await screen.findByText("Saved revision 1")).toBeInTheDocument();
    expect(onDocumentChange).toHaveBeenLastCalledWith(document(1, "# Changed\n"));

    rerender(
      <DocumentPane
        client={knowledgeClient}
        onDocumentChange={onDocumentChange}
        targets={[]}
      />,
    );
    await waitFor(() => expect(onDocumentChange).toHaveBeenLastCalledWith(null));
    expect(screen.getByText("Local draft · not saved")).toBeInTheDocument();
  });

  test("preserves unsaved Markdown when persistence rejects a stale revision", async () => {
    const knowledgeClient = client();
    vi.mocked(knowledgeClient.saveDocument).mockRejectedValue(
      new Error("Knowledge document revision changed; reopen before saving"),
    );
    render(<DocumentPane client={knowledgeClient} targets={[target]} />);
    fireEvent.click(await screen.findByRole("button", { name: "Create knowledge note" }));
    const editor = await screen.findByRole("textbox", {
      name: "Markdown, Mermaid, and code editor",
    });
    fireEvent.change(editor, { target: { value: "# Keep this edit\n" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Vault revision" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/revision changed/i);
    expect(editor).toHaveValue("# Keep this edit\n");
  });
});
