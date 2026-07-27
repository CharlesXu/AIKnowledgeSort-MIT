import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  ArchiveClient,
  ArchiveCommitResult,
  ArchivePlan,
  ConfirmArchivePlanRequest,
  CreateArchivePlanRequest,
  VaultSummary,
} from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createTauriArchiveClient(
  invoke: Invoke = tauriInvoke,
): ArchiveClient {
  return {
    chooseVault() {
      return invoke<VaultSummary | null>("choose_authoritative_vault");
    },
    createPlan(request: CreateArchivePlanRequest) {
      return invoke<ArchivePlan>("create_archive_plan", { request });
    },
    confirmPlan(request: ConfirmArchivePlanRequest) {
      return invoke<ArchiveCommitResult>("confirm_archive_plan", { request });
    },
  };
}

function desktopRuntimeRequired(): Promise<never> {
  return Promise.reject(
    new Error("Desktop runtime is required for archive operations."),
  );
}

export function createBrowserArchiveClient(): ArchiveClient {
  return {
    chooseVault: desktopRuntimeRequired,
    createPlan: desktopRuntimeRequired,
    confirmPlan: desktopRuntimeRequired,
  };
}
