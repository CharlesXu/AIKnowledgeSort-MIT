export type DiscoveryDiagnosticCategory =
  | "excluded"
  | "unreadable"
  | "symlink"
  | "outOfScope"
  | "traversalLimit";

export interface DiscoveredItem {
  readonly path: string;
  readonly name: string;
  readonly byteSize: number;
}

export interface DiscoveryCounts {
  readonly included: number;
  readonly excluded: number;
  readonly unreadable: number;
  readonly symlink: number;
  readonly outOfScope: number;
}

export interface DiscoveryDiagnostic {
  readonly category: DiscoveryDiagnosticCategory;
  readonly path: string;
  readonly message: string;
}

export interface DiscoveryProposal {
  readonly items: readonly DiscoveredItem[];
  readonly counts: DiscoveryCounts;
  readonly diagnostics: readonly DiscoveryDiagnostic[];
}

export interface DiscoveryRequest {
  readonly grantId: string;
}

export interface DropGrantIssued {
  readonly grantId: string;
}
