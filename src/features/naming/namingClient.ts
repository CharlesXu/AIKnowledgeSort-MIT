import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  CreateNamingBatchRequest,
  NamingBatch,
  NamingClient,
} from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createTauriNamingClient(
  invoke: Invoke = tauriInvoke,
): NamingClient {
  return {
    createBatch(request: CreateNamingBatchRequest) {
      return invoke<NamingBatch>("create_naming_batch", { request });
    },
  };
}

export function createBrowserNamingClient(): NamingClient {
  return {
    createBatch() {
      return Promise.reject(
        new Error("Desktop runtime is required for naming operations."),
      );
    },
  };
}
