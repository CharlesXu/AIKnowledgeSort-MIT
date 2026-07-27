import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { DiscoveryProposal, DiscoveryRequest } from "./types";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export interface DiscoveryClient {
  proposeLocalDrop(request: DiscoveryRequest): Promise<DiscoveryProposal>;
}

export function createTauriDiscoveryClient(
  invoke: Invoke = tauriInvoke,
): DiscoveryClient {
  return {
    proposeLocalDrop(request) {
      return invoke<DiscoveryProposal>("propose_local_drop", {
        droppedPaths: [...request.droppedPaths],
        grantedRoots: [...request.grantedRoots],
      });
    },
  };
}

export function createMemoryDiscoveryClient(
  fixture: DiscoveryProposal,
): DiscoveryClient {
  const snapshot = structuredClone(fixture);

  return {
    async proposeLocalDrop() {
      return structuredClone(snapshot);
    },
  };
}

export const createBrowserDiscoveryClient = createMemoryDiscoveryClient;
