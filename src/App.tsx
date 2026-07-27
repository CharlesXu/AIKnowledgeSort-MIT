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

interface AppProps {
  readonly discoveryClient?: DiscoveryClient;
  readonly dropBridge?: NativeDropBridge;
}

const browserDropBridge = createBrowserNativeDropBridge();
const browserDiscoveryClient = createBrowserDiscoveryClient(
  demoDiscoveryProposal,
);

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export default function App({
  discoveryClient,
  dropBridge,
}: AppProps) {
  const nativeRuntime = isTauriRuntime();

  return (
    <AppShell
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
