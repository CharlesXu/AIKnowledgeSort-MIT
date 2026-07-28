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
import {
  createBrowserNamingClient,
  createTauriNamingClient,
} from "./features/naming/namingClient";
import type { NamingClient } from "./features/naming/types";
import {
  createBrowserKnowledgeClient,
  createTauriKnowledgeClient,
} from "./features/knowledge/knowledgeClient";
import type { KnowledgeClient } from "./features/knowledge/types";
import { createBrowserGraphClient, createTauriGraphClient } from "./features/graph/graphClient";
import type { GraphClient } from "./features/graph/types";
import {
  createBrowserProfileClient,
  createTauriProfileClient,
} from "./features/profiles/profileClient";
import type { ProfileClient } from "./features/profiles/types";

interface AppProps {
  readonly archiveClient?: ArchiveClient;
  readonly discoveryClient?: DiscoveryClient;
  readonly dropBridge?: NativeDropBridge;
  readonly namingClient?: NamingClient;
  readonly knowledgeClient?: KnowledgeClient;
  readonly graphClient?: GraphClient;
  readonly profileClient?: ProfileClient;
}

const browserDropBridge = createBrowserNativeDropBridge();
const browserArchiveClient = createBrowserArchiveClient();
const browserDiscoveryClient = createBrowserDiscoveryClient(
  demoDiscoveryProposal,
);
const browserNamingClient = createBrowserNamingClient();
const browserKnowledgeClient = createBrowserKnowledgeClient();
const browserGraphClient = createBrowserGraphClient();
const browserProfileClient = createBrowserProfileClient();

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export default function App({
  archiveClient,
  discoveryClient,
  dropBridge,
  namingClient,
  knowledgeClient,
  graphClient,
  profileClient,
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
      namingClient={
        namingClient ??
        (nativeRuntime ? createTauriNamingClient() : browserNamingClient)
      }
      knowledgeClient={
        knowledgeClient ??
        (nativeRuntime ? createTauriKnowledgeClient() : browserKnowledgeClient)
      }
      graphClient={
        graphClient ?? (nativeRuntime ? createTauriGraphClient() : browserGraphClient)
      }
      profileClient={
        profileClient ??
        (nativeRuntime ? createTauriProfileClient() : browserProfileClient)
      }
    />
  );
}
