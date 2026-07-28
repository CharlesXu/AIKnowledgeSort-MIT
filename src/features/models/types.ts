import type { ContentIdentity } from "../drop/types";

export type ModelLocation = "local" | "remote";

export interface ModelConfigInput {
  readonly configId: string;
  readonly label: string;
  readonly location: ModelLocation;
  readonly endpointUrl: string;
  readonly model: string;
  readonly timeoutMs: number;
  readonly authenticated: boolean;
}

export interface ModelConfigSummary extends ModelConfigInput {
  readonly credentialEnvironment: string | null;
}

export interface ModelRuntimeState {
  readonly schemaVersion: number;
  readonly configs: readonly ModelConfigSummary[];
}

export interface EvidenceRange {
  readonly startLine: number;
  readonly endLine: number;
}

export interface EvidenceExcerpt extends EvidenceRange {
  readonly evidenceId: string;
  readonly text: string;
}

export interface RuleSnapshot {
  readonly policyId: string;
  readonly version: string;
  readonly identity: ContentIdentity;
  readonly json: string;
}

export interface ComparisonEnvelope {
  readonly schemaVersion: number;
  readonly task: "knowledgeRelations";
  readonly originalIdentity: ContentIdentity;
  readonly markdownIdentity: ContentIdentity;
  readonly knowledgeRevision: number;
  readonly ruleSnapshot: RuleSnapshot;
  readonly evidence: readonly EvidenceExcerpt[];
}

export interface RelationSuggestion {
  readonly source: string;
  readonly relationType: string;
  readonly target: string;
  readonly evidenceIds: readonly string[];
}

export interface ModelProposal {
  readonly summary: string;
  readonly relations: readonly RelationSuggestion[];
}

export interface ProviderOutcome {
  readonly status: "succeeded" | "failed";
  readonly model: string | null;
  readonly proposal: ModelProposal | null;
  readonly failureReason: string | null;
}

export interface AgentAdjudication {
  readonly decision: "accept" | "revise" | "reject" | "review";
  readonly reason: string;
  readonly evidenceIds: readonly string[];
  readonly selectedSide: "desktop" | "agent" | null;
  readonly revisedRelations: readonly RelationSuggestion[];
}

export interface ComparisonRecord {
  readonly schemaVersion: number;
  readonly comparisonId: string;
  readonly envelope: ComparisonEnvelope;
  readonly envelopeIdentity: ContentIdentity;
  readonly desktopConfigId: string;
  readonly agentConfigId: string;
  readonly desktopOutcome: ProviderOutcome;
  readonly agentOutcome: ProviderOutcome;
  readonly adjudication: AgentAdjudication | null;
  readonly adjudicationFailure: string | null;
  readonly status: "completed" | "review" | "failed";
  readonly actor: "desktop-orchestrator";
  readonly recordedAtUnixMs: number;
}

export interface RunModelComparisonRequest {
  readonly authorityId: string;
  readonly operationId: string;
  readonly knowledgeRevision: number;
  readonly evidenceRanges: readonly EvidenceRange[];
  readonly desktopConfigId: string;
  readonly agentConfigId: string;
}

export interface ModelRuntimeClient {
  inspect(): Promise<ModelRuntimeState>;
  upsert(request: ModelConfigInput): Promise<ModelRuntimeState>;
  remove(request: { readonly configId: string }): Promise<ModelRuntimeState>;
  runComparison(request: RunModelComparisonRequest): Promise<ComparisonRecord>;
}
