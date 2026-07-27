import type { NativeDropStatus } from "./useNativeDrop";
import type { DiscoveryProposal } from "./types";

interface ScanReportProps {
  readonly isDemo?: boolean;
  readonly proposal: DiscoveryProposal;
  readonly status: NativeDropStatus;
  readonly statusMessage: string;
}

const countDefinitions = [
  ["Included", "included"],
  ["Excluded", "excluded"],
  ["Unreadable", "unreadable"],
  ["Symlinks", "symlink"],
  ["Out of scope", "outOfScope"],
] as const;

export function ScanReport({
  isDemo = false,
  proposal,
  status,
  statusMessage,
}: ScanReportProps) {
  const showStatus =
    status === "loading" ||
    status === "error" ||
    status === "ignored" ||
    status === "ready";

  return (
    <section aria-label="Scan report" className="scan-report" role="region">
      <header className="scan-report__header">
        <div>
          <strong>Scan report</strong>
          <span>{isDemo ? "Demo scan" : "Live scan"}</span>
        </div>
        <span>100%</span>
      </header>
      <div aria-hidden="true" className="scan-report__progress">
        <span />
      </div>
      <p className="scan-report__summary">
        {isDemo ? "Browser fixture" : "Trusted local result"} ·{" "}
        {proposal.items.length} previewed
      </p>
      <div aria-label="Discovery counts" className="scan-report__counts">
        {countDefinitions.map(([label, key]) => (
          <div
            aria-label={label}
            className="scan-report__count"
            key={key}
            role="status"
          >
            <strong>{proposal.counts[key]}</strong>
            <span>{label}</span>
          </div>
        ))}
      </div>
      {showStatus ? (
        <p
          aria-label="Drop status"
          className={`scan-report__status scan-report__status--${status}`}
          role="status"
        >
          {statusMessage}
        </p>
      ) : null}
      <footer className="scan-report__notice">
        <i aria-hidden="true" />
        <span>
          <strong>No files have been changed</strong>
          <small>Drop files or folders anywhere to scan.</small>
        </span>
      </footer>
    </section>
  );
}
