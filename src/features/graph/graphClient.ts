import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  DecideRelationRequest,
  GraphClient,
  GraphRelation,
  GraphSnapshot,
  ImportComparisonRequest,
  InspectGraphRequest,
  ProposeRelationRequest,
} from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createTauriGraphClient(invoke: Invoke = tauriInvoke): GraphClient {
  return {
    inspect(request: InspectGraphRequest) {
      return invoke<GraphSnapshot>("inspect_knowledge_graph", { request });
    },
    propose(request: ProposeRelationRequest) {
      return invoke<GraphRelation>("propose_graph_relation", { request });
    },
    importComparison(request: ImportComparisonRequest) {
      return invoke<readonly GraphRelation[]>("import_comparison_relations", {
        request,
      });
    },
    decide(request: DecideRelationRequest) {
      return invoke<GraphRelation>("decide_graph_relation", { request });
    },
  };
}

function desktopRuntimeRequired(): Promise<never> {
  return Promise.reject(
    new Error("Desktop runtime is required for graph operations."),
  );
}

export function createBrowserGraphClient(): GraphClient {
  return {
    inspect: desktopRuntimeRequired,
    propose: desktopRuntimeRequired,
    importComparison: desktopRuntimeRequired,
    decide: desktopRuntimeRequired,
  };
}
