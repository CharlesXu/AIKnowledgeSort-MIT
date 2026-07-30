import type { ContentIdentity } from "../drop/types";

export type ModelLocation = "local" | "remote";
export type ModelCredentialSource = "environment" | "keychain";
export type ModelProviderProtocol = "openAi" | "anthropic";

export interface ModelConfigInput {
  readonly configId: string;
  readonly label: string;
  readonly location: ModelLocation;
  readonly endpointUrl: string;
  readonly model: string;
  readonly timeoutMs: number;
  readonly authenticated: boolean;
  readonly providerProtocol: ModelProviderProtocol;
  readonly credentialSource: ModelCredentialSource;
  readonly apiKey?: string | null;
}

export interface ModelConfigSummary extends ModelConfigInput {
  readonly credentialEnvironment: string | null;
  readonly credentialStored: boolean;
}

export interface ModelRuntimeState {
  readonly schemaVersion: number;
  readonly configs: readonly ModelConfigSummary[];
}

export interface DiscoverModelsRequest {
  readonly configId: string | null;
  readonly location: ModelLocation;
  readonly endpointUrl: string;
  readonly timeoutMs: number;
  readonly authenticated: boolean;
  readonly credentialSource: ModelCredentialSource;
  readonly apiKey: string | null;
}

export interface DiscoveredModels {
  readonly providerProtocol: ModelProviderProtocol;
  readonly modelsEndpointUrl: string;
  readonly completionEndpointUrl: string;
  readonly models: readonly string[];
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

export interface FileEvidenceExcerpt {
  readonly evidenceId: string;
  readonly location: string;
  readonly text: string;
}

export interface FileSemanticSuggestion {
  readonly summary: string;
  readonly categoryId: string | null;
  readonly categoryEvidenceIds: readonly string[];
  readonly namingFacts: readonly {
    readonly kind: "project" | "model" | "regulation" | "version" | "subject";
    readonly value: string;
    readonly evidenceIds: readonly string[];
  }[];
  readonly uncertaintyReason: string | null;
}

export interface FileSemanticProviderOutcome {
  readonly status: "succeeded" | "failed";
  readonly model: string | null;
  readonly suggestion: FileSemanticSuggestion | null;
  readonly failureReason: string | null;
}

export interface FileSemanticComparison {
  readonly schemaVersion: number;
  readonly comparisonId: string;
  readonly envelope: {
    readonly schemaVersion: number;
    readonly task: "fileClassificationAndNaming";
    readonly itemId: string;
    readonly originalName: string;
    readonly byteSize: number;
    readonly sourceIdentity: ContentIdentity;
    readonly profile: {
      readonly profileId: string;
      readonly version: string;
      readonly identity: ContentIdentity;
      readonly categories: readonly {
        readonly categoryId: string;
        readonly label: string;
        readonly depth: number;
        readonly parentId: string | null;
        readonly path: readonly string[];
        readonly aliases: readonly string[];
      }[];
    };
    readonly evidence: {
      readonly sourceIdentity: ContentIdentity;
      readonly format: "text" | "docx" | "pdf";
      readonly excerpts: readonly FileEvidenceExcerpt[];
      readonly truncated: boolean;
    };
  };
  readonly envelopeIdentity: ContentIdentity;
  readonly desktopConfigId: string;
  readonly agentConfigId: string;
  readonly desktopOutcome: FileSemanticProviderOutcome;
  readonly agentOutcome: FileSemanticProviderOutcome;
  readonly adjudication: {
    readonly decision: "accept" | "revise" | "reject" | "review";
    readonly reason: string;
    readonly evidenceIds: readonly string[];
    readonly selectedSide: "desktop" | "agent" | null;
    readonly revisedSuggestion: FileSemanticSuggestion | null;
  } | null;
  readonly adjudicationFailure: string | null;
  readonly resolvedSuggestion: FileSemanticSuggestion | null;
  readonly status: "completed" | "review" | "failed";
}

export interface RunFileSemanticComparisonRequest {
  readonly proposalId: string;
  readonly itemId: string;
  readonly desktopConfigId: string;
  readonly agentConfigId: string;
}

export interface ModelRuntimeClient {
  inspect(): Promise<ModelRuntimeState>;
  discoverModels(request: DiscoverModelsRequest): Promise<DiscoveredModels>;
  upsert(request: ModelConfigInput): Promise<ModelRuntimeState>;
  remove(request: { readonly configId: string }): Promise<ModelRuntimeState>;
  runComparison(request: RunModelComparisonRequest): Promise<ComparisonRecord>;
  runFileSemanticComparison(
    request: RunFileSemanticComparisonRequest,
  ): Promise<FileSemanticComparison>;
}
