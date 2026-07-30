import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { DropGrantIssued } from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export interface SourcePickerClient {
  chooseFiles(): Promise<DropGrantIssued | null>;
  chooseFolders(): Promise<DropGrantIssued | null>;
}

export function createTauriSourcePickerClient(
  invoke: Invoke = tauriInvoke,
): SourcePickerClient {
  return {
    chooseFiles() {
      return invoke<DropGrantIssued | null>("choose_local_files");
    },
    chooseFolders() {
      return invoke<DropGrantIssued | null>("choose_local_folders");
    },
  };
}

const DESKTOP_REQUIRED_MESSAGE =
  "Adding local sources requires the desktop app.";

export function createBrowserSourcePickerClient(): SourcePickerClient {
  return {
    async chooseFiles() {
      throw new Error(DESKTOP_REQUIRED_MESSAGE);
    },
    async chooseFolders() {
      throw new Error(DESKTOP_REQUIRED_MESSAGE);
    },
  };
}
