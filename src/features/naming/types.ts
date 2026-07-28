import type { ContentIdentity } from "../drop/types";

export type NamingFactKind =
  | "project"
  | "model"
  | "regulation"
  | "version"
  | "subject";

export interface NamingFact {
  readonly kind: NamingFactKind;
  readonly value: string;
  readonly evidenceLocation: string;
}

export interface NamingItemInput {
  readonly itemId: string;
  readonly facts: readonly NamingFact[];
}

export interface CreateNamingBatchRequest {
  readonly proposalId: string;
  readonly items: readonly NamingItemInput[];
}

export type NamingReviewReason =
  | "missingEvidence"
  | "conflictingEvidence"
  | "unsafeName"
  | "collision";

export interface NamingProposal {
  readonly proposalId: string;
  readonly itemId: string;
  readonly originalName: string;
  readonly canonicalName: string | null;
  readonly identity: ContentIdentity;
  readonly policyId: string;
  readonly policyVersion: string;
  readonly appliedRule: string;
  readonly status: "proposed" | "namingReview";
  readonly reviewReason: NamingReviewReason | null;
  readonly facts: readonly NamingFact[];
}

export interface NamingBatch {
  readonly batchId: string;
  readonly discoveryProposalId: string;
  readonly policyId: string;
  readonly policyVersion: string;
  readonly expiresAtUnixMs: number;
  readonly proposals: readonly NamingProposal[];
}

export interface NamingClient {
  createBatch(request: CreateNamingBatchRequest): Promise<NamingBatch>;
}
