import type { ContentIdentity } from "../drop/types";

export type ProfileStatus = "draft" | "candidate" | "approved" | "rejected";
export type ProfileDecision = "approve" | "reject";
export type ProfileSourceKind = "localFile" | "remoteUrl";
export type CandidateStatus = "unapproved" | "approved" | "rejected";

export interface ProfileVersionRef {
  readonly profileId: string;
  readonly version: string;
}

export interface ProfileSummary extends ProfileVersionRef {
  readonly title: string;
  readonly status: ProfileStatus;
  readonly ruleCount: number;
  readonly categoryCount: number;
  readonly taxonomyCounts: {
    readonly level1: number;
    readonly level2: number;
    readonly level3: number;
    readonly level4: number;
  };
  readonly semanticEvidenceRequired: boolean;
  readonly uniquePrimaryArchiveCategory: boolean;
  readonly crossDomainKnowledgeLinks: boolean;
  readonly provenanceTitle: string;
}

export interface ProfileDiff {
  readonly addedRuleIds: readonly string[];
  readonly removedRuleIds: readonly string[];
  readonly changedRuleIds: readonly string[];
  readonly addedCategoryIds: readonly string[];
  readonly removedCategoryIds: readonly string[];
  readonly changedCategoryIds: readonly string[];
}

export interface ProfileDecisionSummary {
  readonly actor: string;
  readonly decidedAtUnixMs: number;
  readonly decision: ProfileDecision;
  readonly reviewedDigest: string;
}

export interface ProfileCandidateRecord {
  readonly schemaVersion: number;
  readonly candidateId: string;
  readonly importedAtUnixMs: number;
  readonly sourceKind: ProfileSourceKind;
  readonly sourceBasename: string;
  readonly sourceByteSize: number;
  readonly locatorIdentity: ContentIdentity;
  readonly sourceIdentity: ContentIdentity;
  readonly profileId: string;
  readonly profileVersion: string;
  readonly status: CandidateStatus;
  readonly base: ProfileVersionRef | null;
  readonly diff: ProfileDiff;
  readonly approval: ProfileDecisionSummary | null;
}

export interface ProfileStateSummary {
  readonly installed: readonly ProfileSummary[];
  readonly active: ProfileVersionRef | null;
  readonly candidates: readonly ProfileCandidateRecord[];
}

export interface DecideProfileCandidateRequest {
  readonly candidateId: string;
  readonly reviewedDigest: string;
  readonly decision: ProfileDecision;
}

export type EvidenceKind =
  | "documentText"
  | "ocrText"
  | "transcript"
  | "reliableCompanion";

export interface ClassificationEvidenceReference {
  readonly kind: EvidenceKind;
  readonly location: string;
  readonly text: string;
}

export interface ClassificationProposal {
  readonly proposalId: string;
  readonly sourceIdentity: ContentIdentity;
  readonly profileId: string;
  readonly profileVersion: string;
  readonly status: "proposed" | "classificationReview";
  readonly ruleIds: readonly string[];
  readonly evidence: readonly {
    readonly kind: EvidenceKind;
    readonly location: string;
  }[];
  readonly destination: readonly string[] | null;
  readonly reviewReason: "missingEvidence" | "conflictingRules" | null;
  readonly committable: boolean;
}

export interface ClassificationBatch {
  readonly batchId: string;
  readonly discoveryProposalId: string;
  readonly profileId: string;
  readonly profileVersion: string;
  readonly expiresAtUnixMs: number;
  readonly items: readonly {
    readonly itemId: string;
    readonly proposal: ClassificationProposal;
  }[];
}

export interface CreateClassificationBatchRequest {
  readonly proposalId: string;
  readonly items: readonly {
    readonly itemId: string;
    readonly references: readonly ClassificationEvidenceReference[];
  }[];
}

export interface ProfileClient {
  inspect(): Promise<ProfileStateSummary>;
  importLocalCandidate(): Promise<ProfileCandidateRecord | null>;
  importUrlCandidate(url: string): Promise<ProfileCandidateRecord>;
  decideCandidate(
    request: DecideProfileCandidateRequest,
  ): Promise<ProfileStateSummary>;
  createClassificationBatch(
    request: CreateClassificationBatchRequest,
  ): Promise<ClassificationBatch>;
}
