export type SourceNodeKind = "file" | "directory";

export interface SourceNode {
  readonly id: string;
  readonly name: string;
  readonly kind: SourceNodeKind;
  readonly eligible: boolean;
  readonly children: readonly SourceNode[];
}

export type SelectionState = "unchecked" | "indeterminate" | "checked";

export interface ResolvedSelection {
  readonly explicitIds: readonly string[];
  readonly files: readonly SourceNode[];
}
