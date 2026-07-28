import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  DecideProfileCandidateRequest,
  ProfileCandidateRecord,
  ProfileClient,
  ProfileStateSummary,
} from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createTauriProfileClient(
  invoke: Invoke = tauriInvoke,
): ProfileClient {
  return {
    inspect() {
      return invoke<ProfileStateSummary>("inspect_profile_state");
    },
    importLocalCandidate() {
      return invoke<ProfileCandidateRecord | null>(
        "import_local_profile_candidate",
      );
    },
    decideCandidate(request: DecideProfileCandidateRequest) {
      return invoke<ProfileStateSummary>("decide_profile_candidate", {
        request,
      });
    },
  };
}

function desktopRuntimeRequired(): Promise<never> {
  return Promise.reject(
    new Error("Desktop runtime is required for profile operations."),
  );
}

export function createBrowserProfileClient(): ProfileClient {
  return {
    inspect: desktopRuntimeRequired,
    importLocalCandidate: desktopRuntimeRequired,
    decideCandidate: desktopRuntimeRequired,
  };
}
