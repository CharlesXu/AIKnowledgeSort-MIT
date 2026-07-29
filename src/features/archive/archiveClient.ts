import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  ArchiveClient,
  ArchiveCommitResult,
  ArchivePlan,
  ArchiveUndoPlan,
  ArchiveUndoResult,
  CleanupPlan,
  CleanupResult,
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
    createCleanupPlan(request) {
      return invoke<CleanupPlan>("create_cleanup_plan", { request });
    },
    authorizePermanentCleanup(request) {
      return invoke<CleanupPlan>("authorize_permanent_cleanup", { request });
    },
    confirmCleanupPlan(request) {
      return invoke<CleanupResult>("confirm_cleanup_plan", { request });
    },
    createArchiveUndoPlan(request) {
      return invoke<ArchiveUndoPlan>("create_archive_undo_plan", { request });
    },
    confirmArchiveUndoPlan(request) {
      return invoke<ArchiveUndoResult>("confirm_archive_undo_plan", { request });
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
    createCleanupPlan: desktopRuntimeRequired,
    authorizePermanentCleanup: desktopRuntimeRequired,
    confirmCleanupPlan: desktopRuntimeRequired,
    createArchiveUndoPlan: desktopRuntimeRequired,
    confirmArchiveUndoPlan: desktopRuntimeRequired,
  };
}
