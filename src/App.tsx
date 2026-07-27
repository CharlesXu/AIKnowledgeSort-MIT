import { AppShell } from "./app/AppShell";
import { demoDiscoveryProposal } from "./data/demoSources";
import {
  createBrowserDiscoveryClient,
  createTauriDiscoveryClient,
  type DiscoveryClient,
} from "./features/drop/discoveryClient";
import {
  createBrowserNativeDropBridge,
  tauriNativeDropBridge,
  type NativeDropBridge,
} from "./features/drop/useNativeDrop";
import {
  createBrowserArchiveClient,
  createTauriArchiveClient,
} from "./features/archive/archiveClient";
import type { ArchiveClient } from "./features/archive/types";

interface AppProps {
  readonly archiveClient?: ArchiveClient;
  readonly discoveryClient?: DiscoveryClient;
  readonly dropBridge?: NativeDropBridge;
}

const browserDropBridge = createBrowserNativeDropBridge();
const browserArchiveClient = createBrowserArchiveClient();
const browserDiscoveryClient = createBrowserDiscoveryClient(
  demoDiscoveryProposal,
);

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export default function App({
  archiveClient,
  discoveryClient,
  dropBridge,
}: AppProps) {
  const nativeRuntime = isTauriRuntime();

  return (
    <AppShell
      archiveClient={
        archiveClient ??
        (nativeRuntime ? createTauriArchiveClient() : browserArchiveClient)
      }
      discoveryClient={
        discoveryClient ??
        (nativeRuntime
          ? createTauriDiscoveryClient()
          : browserDiscoveryClient)
      }
      dropBridge={
        dropBridge ?? (nativeRuntime ? tauriNativeDropBridge : browserDropBridge)
      }
    />
  );
}
