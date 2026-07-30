import type { ContentIdentity } from "../drop/types";

export interface EvidenceRange {
  readonly startLine: number;
  readonly endLine: number;
}

export interface EvidenceReference extends EvidenceRange {
  readonly operationId: string;
  readonly knowledgeRevision: number;
  readonly text: string;
  readonly markdownIdentity: ContentIdentity;
  readonly originalIdentity: ContentIdentity;
}

export type RelationStatus = "review" | "accepted" | "rejected";
export type GraphDecision = "accept" | "revise" | "reject";

export interface GraphRelation {
  readonly relationId: string;
  readonly version: number;
  readonly authorityId: string;
  readonly operationId: string;
  readonly knowledgeRevision: number;
  readonly sourceNode: string;
  readonly relationType: string;
  readonly targetNode: string;
  readonly status: RelationStatus;
  readonly evidence: readonly EvidenceReference[];
  readonly comparisonId?: string | null;
  readonly actor: string;
  readonly reason: string;
  readonly recordedAtUnixMs: number;
}

export interface GraphEvent {
  readonly relationId: string;
  readonly version: number;
  readonly status: RelationStatus;
  readonly sourceNode: string;
  readonly relationType: string;
  readonly targetNode: string;
  readonly recordedAtUnixMs: number;
}

export interface GraphSnapshot {
  readonly authorityId: string;
  readonly operationId: string;
  readonly relations: readonly GraphRelation[];
  readonly events: readonly GraphEvent[];
}

export interface RelationRevisionInput {
  readonly knowledgeRevision: number;
  readonly sourceNode: string;
  readonly relationType: string;
  readonly targetNode: string;
  readonly evidenceRanges: readonly EvidenceRange[];
}

export interface InspectGraphRequest {
  readonly authorityId: string;
  readonly operationId: string;
}

export interface ProposeRelationRequest extends RelationRevisionInput {
  readonly authorityId: string;
  readonly operationId: string;
}

export interface ImportComparisonRequest {
  readonly authorityId: string;
  readonly comparisonId: string;
}

export interface DecideRelationRequest {
  readonly authorityId: string;
  readonly relationId: string;
  readonly expectedVersion: number;
  readonly decision: GraphDecision;
  readonly reason: string;
  readonly revision: RelationRevisionInput | null;
}

export interface GraphClient {
  inspect(request: InspectGraphRequest): Promise<GraphSnapshot>;
  propose(request: ProposeRelationRequest): Promise<GraphRelation>;
  importComparison(request: ImportComparisonRequest): Promise<readonly GraphRelation[]>;
  decide(request: DecideRelationRequest): Promise<GraphRelation>;
}
