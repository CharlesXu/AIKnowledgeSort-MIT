import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  AgentAccessClient,
  AgentAccessState,
  CreateAgentGrantRequest,
  IssuedAgentGrant,
  NativeScopeSelection,
} from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createTauriAgentAccessClient(
  invoke: Invoke = tauriInvoke,
): AgentAccessClient {
  return {
    selectDirectories() {
      return invoke<NativeScopeSelection | null>("select_agent_grant_directories");
    },
    inspect() {
      return invoke<AgentAccessState>("inspect_agent_access");
    },
    createGrant(request: CreateAgentGrantRequest) {
      return invoke<IssuedAgentGrant>("create_agent_grant", { request });
    },
    revokeGrant(request: { readonly grantId: string }) {
      return invoke<AgentAccessState>("revoke_agent_grant", { request });
    },
  };
}

function desktopRuntimeRequired(): Promise<never> {
  return Promise.reject(
    new Error("Desktop runtime is required for Agent access operations."),
  );
}

export function createBrowserAgentAccessClient(): AgentAccessClient {
  return {
    selectDirectories: desktopRuntimeRequired,
    inspect: desktopRuntimeRequired,
    createGrant: desktopRuntimeRequired,
    revokeGrant: desktopRuntimeRequired,
  };
}
