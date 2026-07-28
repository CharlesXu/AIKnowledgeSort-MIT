import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  ModelConfigInput,
  ModelRuntimeClient,
  ModelRuntimeState,
  RunModelComparisonRequest,
  ComparisonRecord,
} from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createTauriModelRuntimeClient(
  invoke: Invoke = tauriInvoke,
): ModelRuntimeClient {
  return {
    inspect() {
      return invoke<ModelRuntimeState>("inspect_model_runtime");
    },
    upsert(request: ModelConfigInput) {
      return invoke<ModelRuntimeState>("upsert_model_config", { request });
    },
    remove(request: { readonly configId: string }) {
      return invoke<ModelRuntimeState>("remove_model_config", { request });
    },
    runComparison(request: RunModelComparisonRequest) {
      return invoke<ComparisonRecord>("run_model_comparison", { request });
    },
  };
}

function desktopRuntimeRequired(): Promise<never> {
  return Promise.reject(
    new Error("Desktop runtime is required for model runtime operations."),
  );
}

export function createBrowserModelRuntimeClient(): ModelRuntimeClient {
  return {
    inspect: desktopRuntimeRequired,
    upsert: desktopRuntimeRequired,
    remove: desktopRuntimeRequired,
    runComparison: desktopRuntimeRequired,
  };
}
