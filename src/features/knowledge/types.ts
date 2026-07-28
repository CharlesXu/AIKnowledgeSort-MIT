import type { ContentIdentity } from "../drop/types";

export interface KnowledgeTarget {
  readonly authorityId: string;
  readonly operationId: string;
  readonly itemId: string;
  readonly destinationPath: string;
  readonly originalIdentity: ContentIdentity;
}

export interface KnowledgeDocument {
  readonly documentId: string;
  readonly authorityId: string;
  readonly operationId: string;
  readonly revision: number;
  readonly markdownPath: string | null;
  readonly markdown: string;
  readonly savedAtUnixMs: number | null;
  readonly markdownIdentity: ContentIdentity | null;
  readonly originalIdentity: ContentIdentity;
}

export interface OpenKnowledgeDocumentRequest {
  readonly authorityId: string;
  readonly operationId: string;
}

export interface SaveKnowledgeDocumentRequest
  extends OpenKnowledgeDocumentRequest {
  readonly expectedRevision: number;
  readonly markdown: string;
}

export interface KnowledgeClient {
  openDocument(request: OpenKnowledgeDocumentRequest): Promise<KnowledgeDocument>;
  saveDocument(request: SaveKnowledgeDocumentRequest): Promise<KnowledgeDocument>;
}
