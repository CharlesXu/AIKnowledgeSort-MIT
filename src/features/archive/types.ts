import type { ContentIdentity } from "../drop/types";

export interface VaultSummary {
  readonly authorityId: string;
  readonly displayPath: string;
  readonly status: "authoritative";
}

export interface ArchivePlanItem {
  readonly itemId: string;
  readonly sourcePath: string;
  readonly destinationPath: string;
  readonly byteSize: number;
  readonly identity: ContentIdentity;
}

export interface ArchivePlan {
  readonly planId: string;
  readonly planVersion: number;
  readonly proposalId: string;
  readonly authorityId: string;
  readonly vaultPath: string;
  readonly expiresAtUnixMs: number;
  readonly confirmationNonce: string;
  readonly sourcePreserved: boolean;
  readonly items: readonly ArchivePlanItem[];
}

export interface ArchiveItemResult {
  readonly operationId: string;
  readonly itemId: string;
  readonly destinationPath: string;
  readonly identity: ContentIdentity;
  readonly status: "committed" | "failed";
  readonly failureReason: string | null;
}

export interface ArchiveCommitResult {
  readonly planId: string;
  readonly status: "committed" | "partial" | "failed";
  readonly items: readonly ArchiveItemResult[];
}

export interface CreateArchivePlanRequest {
  readonly proposalId: string;
  readonly itemIds: readonly string[];
}

export interface ConfirmArchivePlanRequest {
  readonly planId: string;
  readonly confirmationNonce: string;
}

export interface ArchiveClient {
  chooseVault(): Promise<VaultSummary | null>;
  createPlan(request: CreateArchivePlanRequest): Promise<ArchivePlan>;
  confirmPlan(request: ConfirmArchivePlanRequest): Promise<ArchiveCommitResult>;
}
