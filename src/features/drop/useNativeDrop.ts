import { listen } from "@tauri-apps/api/event";
import {
  getCurrentWebview,
  type DragDropEvent,
} from "@tauri-apps/api/webview";
import { useCallback, useEffect, useRef, useState } from "react";
import type { DiscoveryClient } from "./discoveryClient";
import type { DiscoveryProposal, DropGrantIssued } from "./types";

export type NativeDropStatus =
  | "idle"
  | "hovering"
  | "loading"
  | "ready"
  | "error"
  | "ignored";

export interface NativeDragState {
  readonly type: "over" | "drop" | "cancel";
}

export interface NativeDropCallbacks {
  readonly onGrant: (grant: DropGrantIssued) => void;
  readonly onGrantError: (message: string) => void;
  readonly onDragState: (event: NativeDragState) => void;
}

export interface NativeDropBridge {
  subscribe(callbacks: NativeDropCallbacks): Promise<() => void>;
}

interface DomDropEvent {
  readonly dataTransfer: {
    readonly files: { readonly length: number };
    readonly types: readonly string[];
    getData(type: string): string;
  };
  preventDefault(): void;
}

interface DomDragOverEvent {
  readonly dataTransfer: {
    readonly files: { readonly length: number };
    readonly types: readonly string[];
  };
  preventDefault(): void;
}

interface UseNativeDropOptions {
  readonly bridge: NativeDropBridge;
  readonly discoveryClient: DiscoveryClient;
  readonly initialProposal?: DiscoveryProposal;
}

export interface NativeDropState {
  readonly status: NativeDropStatus;
  readonly message: string;
  readonly proposal?: DiscoveryProposal;
  readonly isDemo: boolean;
  readonly reviewGrant: (grant: DropGrantIssued) => void;
  readonly reportGrantError: (message: string) => void;
  readonly onDomDragOver: (event: DomDragOverEvent) => void;
  readonly onDomDrop: (event: DomDropEvent) => void;
}

const MAX_VISIBLE_ERROR_LENGTH = 240;
const EXTERNAL_DROP_MESSAGE =
  "External text and URL drops are ignored. Drop local files from the native desktop window.";

function boundedMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.slice(0, MAX_VISIBLE_ERROR_LENGTH);
}

function isExternalTextDrop(types: readonly string[], fileCount: number): boolean {
  return (
    fileCount === 0 &&
    (types.includes("text/plain") || types.includes("text/uri-list"))
  );
}

export const tauriNativeDropBridge: NativeDropBridge = {
  async subscribe(callbacks) {
    const cleanups: Array<() => void> = [];

    try {
      cleanups.push(
        await listen<DropGrantIssued>("local-drop-grant", ({ payload }) => {
          callbacks.onGrant(payload);
        }),
      );
      cleanups.push(
        await listen<string>("local-drop-grant-error", ({ payload }) => {
          callbacks.onGrantError(payload);
        }),
      );
      cleanups.push(
        await getCurrentWebview().onDragDropEvent(({ payload }) => {
          const type = mapDragState(payload);
          callbacks.onDragState({ type });
        }),
      );
    } catch (error) {
      cleanups.forEach((cleanup) => cleanup());
      throw error;
    }

    return () => {
      cleanups.forEach((cleanup) => cleanup());
    };
  },
};

function mapDragState(
  event: DragDropEvent,
): NativeDragState["type"] {
  if (event.type === "enter" || event.type === "over") {
    return "over";
  }
  return event.type === "drop" ? "drop" : "cancel";
}

export function createBrowserNativeDropBridge(): NativeDropBridge {
  return {
    async subscribe() {
      return () => {};
    },
  };
}

export function useNativeDrop({
  bridge,
  discoveryClient,
  initialProposal,
}: UseNativeDropOptions): NativeDropState {
  const [status, setStatus] = useState<NativeDropStatus>("idle");
  const [message, setMessage] = useState("");
  const [proposal, setProposal] = useState(initialProposal);
  const [isDemo, setIsDemo] = useState(initialProposal !== undefined);
  const seenGrantIds = useRef(new Set<string>());
  const statusBeforeHover = useRef<NativeDropStatus>("idle");
  const requestSequence = useRef(0);

  const reviewGrant = useCallback(
    (grant: DropGrantIssued) => {
      if (seenGrantIds.current.has(grant.grantId)) {
        return;
      }
      seenGrantIds.current.add(grant.grantId);
      const requestId = ++requestSequence.current;
      setStatus("loading");
      setMessage("Reviewing trusted local sources…");
      void discoveryClient
        .proposeLocalDrop({ grantId: grant.grantId })
        .then((nextProposal) => {
          if (requestId !== requestSequence.current) {
            return;
          }
          setProposal(nextProposal);
          setIsDemo(false);
          setStatus("ready");
          setMessage("Trusted local discovery proposal is ready.");
        })
        .catch((error: unknown) => {
          if (requestId !== requestSequence.current) {
            return;
          }
          setStatus("error");
          setMessage(boundedMessage(error));
        });
    },
    [discoveryClient],
  );

  const reportGrantError = useCallback((errorMessage: string) => {
    requestSequence.current += 1;
    setStatus("error");
    setMessage(boundedMessage(errorMessage));
  }, []);

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;

    const callbacks: NativeDropCallbacks = {
      onGrant(grant) {
        if (!disposed) {
          reviewGrant(grant);
        }
      },
      onGrantError(errorMessage) {
        if (!disposed) {
          reportGrantError(errorMessage);
        }
      },
      onDragState(event) {
        if (disposed) {
          return;
        }
        if (event.type === "over") {
          setStatus((current) => {
            if (current !== "hovering") {
              statusBeforeHover.current = current;
            }
            return "hovering";
          });
          setMessage("Release to review local files.");
          return;
        }

        setStatus((current) =>
          current === "hovering" ? statusBeforeHover.current : current,
        );
        setMessage("");
      },
    };

    void bridge
      .subscribe(callbacks)
      .then((unsubscribe) => {
        if (disposed) {
          unsubscribe();
        } else {
          cleanup = unsubscribe;
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setStatus("error");
          setMessage(boundedMessage(error));
        }
      });

    return () => {
      disposed = true;
      requestSequence.current += 1;
      cleanup?.();
    };
  }, [bridge, reportGrantError, reviewGrant]);

  const onDomDragOver = useCallback((event: DomDragOverEvent) => {
    if (isExternalTextDrop(event.dataTransfer.types, event.dataTransfer.files.length)) {
      event.preventDefault();
    }
  }, []);

  const onDomDrop = useCallback((event: DomDropEvent) => {
    if (!isExternalTextDrop(event.dataTransfer.types, event.dataTransfer.files.length)) {
      return;
    }
    event.preventDefault();
    setStatus("ignored");
    setMessage(EXTERNAL_DROP_MESSAGE);
  }, []);

  return {
    status,
    message,
    proposal,
    isDemo,
    reviewGrant,
    reportGrantError,
    onDomDragOver,
    onDomDrop,
  };
}
