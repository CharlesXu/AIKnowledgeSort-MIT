export type AgentToolEffect = "read" | "semanticAdvice";
export type AgentGrantStatus = "active" | "inactive" | "revoked" | "expired";

export interface AgentToolDescriptor {
  readonly toolId: string;
  readonly title: string;
  readonly effect: AgentToolEffect;
}

export interface AgentResourceLimits {
  readonly maxRequestsPerSession: number;
  readonly maxRequestBytes: number;
  readonly maxResponseBytes: number;
}

export interface AgentScopeSummary {
  readonly scopeId: string;
  readonly displayPath: string;
}

export interface NativeScopeSelection {
  readonly selectionId: string;
  readonly scopes: readonly AgentScopeSummary[];
}

export interface CreateAgentGrantRequest {
  readonly selectionId: string;
  readonly agentId: string;
  readonly label: string;
  readonly toolIds: readonly string[];
  readonly allowedHttpOrigins: readonly string[];
  readonly expiresInSeconds: number;
  readonly limits: AgentResourceLimits;
}

export interface AgentGrantSummary {
  readonly grantId: string;
  readonly agentId: string;
  readonly label: string;
  readonly toolIds: readonly string[];
  readonly allowedHttpOrigins: readonly string[];
  readonly scopes: readonly AgentScopeSummary[];
  readonly createdAtUnixMs: number;
  readonly expiresAtUnixMs: number;
  readonly revokedAtUnixMs: number | null;
  readonly status: AgentGrantStatus;
  readonly limits: AgentResourceLimits;
}

export interface IssuedAgentGrant {
  readonly grant: AgentGrantSummary;
  readonly grantToken: string;
}

export interface AgentAccessState {
  readonly schemaVersion: number;
  readonly toolCatalogVersion: string;
  readonly tools: readonly AgentToolDescriptor[];
  readonly grants: readonly AgentGrantSummary[];
}

export interface McpTransportState {
  readonly running: boolean;
  readonly url: string | null;
  readonly executablePath: string | null;
}

export interface AgentAccessClient {
  selectDirectories(): Promise<NativeScopeSelection | null>;
  inspect(): Promise<AgentAccessState>;
  createGrant(request: CreateAgentGrantRequest): Promise<IssuedAgentGrant>;
  revokeGrant(request: { readonly grantId: string }): Promise<AgentAccessState>;
  inspectTransport(): Promise<McpTransportState>;
  startTransport(request: { readonly port: number }): Promise<McpTransportState>;
  stopTransport(): Promise<McpTransportState>;
}
