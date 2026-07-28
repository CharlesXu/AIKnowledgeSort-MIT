import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
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
    openDocument: vi.fn().mockResolvedValue(document()),
    saveDocument: vi.fn().mockResolvedValue(document(1, "# Changed\n")),
  };
}

describe("DocumentPane", () => {
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
    render(<DocumentPane client={knowledgeClient} targets={[target]} />);

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
    expect(editor).toHaveValue("# Reset reliability\n");
    expect(screen.getByText("New Vault note · not saved")).toBeInTheDocument();

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
