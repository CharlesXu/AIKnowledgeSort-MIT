import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  KnowledgeClient,
  KnowledgeDocument,
  KnowledgeTarget,
  ListKnowledgeTargetsRequest,
  OpenKnowledgeDocumentRequest,
  SaveKnowledgeDocumentRequest,
} from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createTauriKnowledgeClient(
  invoke: Invoke = tauriInvoke,
): KnowledgeClient {
  return {
    listTargets(request: ListKnowledgeTargetsRequest) {
      return invoke<readonly KnowledgeTarget[]>("list_knowledge_targets", {
        request,
      });
    },
    openDocument(request: OpenKnowledgeDocumentRequest) {
      return invoke<KnowledgeDocument>("open_knowledge_document", { request });
    },
    saveDocument(request: SaveKnowledgeDocumentRequest) {
      return invoke<KnowledgeDocument>("save_knowledge_document", { request });
    },
  };
}

function desktopRuntimeRequired(): Promise<never> {
  return Promise.reject(
    new Error("Desktop runtime is required for knowledge operations."),
  );
}

export function createBrowserKnowledgeClient(): KnowledgeClient {
  return {
    listTargets: desktopRuntimeRequired,
    openDocument: desktopRuntimeRequired,
    saveDocument: desktopRuntimeRequired,
  };
}
