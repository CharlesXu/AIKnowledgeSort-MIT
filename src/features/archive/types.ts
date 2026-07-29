import type { ContentIdentity } from "../drop/types";
import type { NamingFact } from "../naming/types";
import type { ClassificationProposal } from "../profiles/types";

export interface NamingDecisionEvidence {
  readonly namingProposalId: string;
  readonly originalName: string;
  readonly canonicalName: string;
  readonly policyId: string;
  readonly policyVersion: string;
  readonly appliedRule: string;
  readonly facts: readonly NamingFact[];
}

export interface VaultSummary {
  readonly authorityId: string;
  readonly displayPath: string;
  readonly status: "authoritative";
}

export interface ArchivePlanItem {
  readonly itemId: string;
  readonly sourcePath: string;
  readonly destinationPath: string;
  readonly originalName: string;
  readonly canonicalName: string;
  readonly naming: NamingDecisionEvidence;
  readonly classification?: ClassificationProposal;
  readonly byteSize: number;
  readonly identity: ContentIdentity;
}

export interface ArchivePlan {
  readonly planId: string;
  readonly planVersion: number;
  readonly proposalId: string;
  readonly namingBatchId: string;
  readonly classificationBatchId?: string;
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
  readonly namingBatchId: string;
  readonly classificationBatchId: string;
}

export interface ConfirmArchivePlanRequest {
  readonly planId: string;
  readonly confirmationNonce: string;
}

export interface CleanupPlanItem {
  readonly operationId: string;
  readonly sourcePath: string;
  readonly retainedPath: string;
  readonly identity: ContentIdentity;
}

export interface CleanupPlan {
  readonly planId: string;
  readonly planVersion: number;
  readonly authorityId: string;
  readonly disposition: "trash" | "permanentDelete";
  readonly items: readonly CleanupPlanItem[];
  readonly expiresAtUnixMs: number;
  readonly confirmationNonce: string;
  readonly confirmationBindingSha256: string;
}

export interface CleanupResult {
  readonly planId: string;
  readonly status: "committed" | "failed";
  readonly disposition: "trash" | "permanentDelete";
  readonly removedPaths: readonly string[];
  readonly failureReason: string | null;
}

export interface ArchiveUndoPlan {
  readonly undoId: string;
  readonly planVersion: number;
  readonly operationId: string;
  readonly authorityId: string;
  readonly sourcePath: string;
  readonly archivedPath: string;
  readonly archivedRelativePath: string;
  readonly byteSize: number;
  readonly identity: ContentIdentity;
  readonly expiresAtUnixMs: number;
  readonly confirmationNonce: string;
  readonly confirmationBindingSha256: string;
}

export interface ArchiveUndoResult {
  readonly undoId: string;
  readonly operationId: string;
  readonly status: "committed" | "failed";
  readonly failureReason: string | null;
}

export interface ArchiveClient {
  chooseVault(): Promise<VaultSummary | null>;
  createPlan(request: CreateArchivePlanRequest): Promise<ArchivePlan>;
  confirmPlan(request: ConfirmArchivePlanRequest): Promise<ArchiveCommitResult>;
  createCleanupPlan(request: {
    readonly authorityId: string;
    readonly operationIds: readonly string[];
    readonly cleanupEnabled: boolean;
  }): Promise<CleanupPlan>;
  authorizePermanentCleanup(request: {
    readonly planId: string;
    readonly confirmationNonce: string;
  }): Promise<CleanupPlan>;
  confirmCleanupPlan(request: {
    readonly planId: string;
    readonly confirmationNonce: string;
  }): Promise<CleanupResult>;
  createArchiveUndoPlan(request: {
    readonly operationId: string;
  }): Promise<ArchiveUndoPlan>;
  confirmArchiveUndoPlan(request: {
    readonly undoId: string;
    readonly confirmationNonce: string;
  }): Promise<ArchiveUndoResult>;
}
