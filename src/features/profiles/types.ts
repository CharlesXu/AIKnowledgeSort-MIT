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
  readonly provenanceTitle: string;
}

export interface ProfileDiff {
  readonly addedRuleIds: readonly string[];
  readonly removedRuleIds: readonly string[];
  readonly changedRuleIds: readonly string[];
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

export interface ProfileClient {
  inspect(): Promise<ProfileStateSummary>;
  importLocalCandidate(): Promise<ProfileCandidateRecord | null>;
  importUrlCandidate(url: string): Promise<ProfileCandidateRecord>;
  decideCandidate(
    request: DecideProfileCandidateRequest,
  ): Promise<ProfileStateSummary>;
}
