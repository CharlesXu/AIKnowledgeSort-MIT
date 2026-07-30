import type { NativeDropStatus } from "./useNativeDrop";
import type { DiscoveryProposal } from "./types";
import {
  useI18n,
  type TranslationKey,
} from "../../i18n/I18nContext";

interface ScanReportProps {
  readonly isDemo?: boolean;
  readonly proposal: DiscoveryProposal;
  readonly status: NativeDropStatus;
  readonly statusMessage: string;
}

const countDefinitions: readonly [TranslationKey, keyof DiscoveryProposal["counts"]][] = [
  ["scan.included", "included"],
  ["scan.excluded", "excluded"],
  ["scan.unreadable", "unreadable"],
  ["scan.symlinks", "symlink"],
  ["scan.outOfScope", "outOfScope"],
] as const;

export function ScanReport({
  isDemo = false,
  proposal,
  status,
  statusMessage,
}: ScanReportProps) {
  const { t } = useI18n();
  const showStatus =
    status === "loading" ||
    status === "error" ||
    status === "ignored" ||
    status === "ready";

  return (
    <section aria-label={t("scan.report")} className="scan-report" role="region">
      <header className="scan-report__header">
        <div>
          <strong>{t("scan.report")}</strong>
          <span>{isDemo ? t("scan.demo") : t("scan.live")}</span>
        </div>
        <span>100%</span>
      </header>
      <div aria-hidden="true" className="scan-report__progress">
        <span />
      </div>
      <p className="scan-report__summary">
        {isDemo ? t("scan.browserFixture") : t("scan.trustedResult")} ·{" "}
        {t("scan.previewed", { count: proposal.items.length })}
      </p>
      <div aria-label={t("scan.counts")} className="scan-report__counts">
        {countDefinitions.map(([labelKey, key]) => (
          <div
            aria-label={t(labelKey)}
            className="scan-report__count"
            key={key}
            role="status"
          >
            <strong>{proposal.counts[key]}</strong>
            <span>{t(labelKey)}</span>
          </div>
        ))}
      </div>
      {showStatus ? (
        <p
          aria-label={t("scan.dropStatus")}
          className={`scan-report__status scan-report__status--${status}`}
          role="status"
        >
          {statusMessage}
        </p>
      ) : null}
      <footer className="scan-report__notice">
        <i aria-hidden="true" />
        <span>
          <strong>{t("scan.unchanged")}</strong>
          <small>{t("scan.dropHint")}</small>
        </span>
      </footer>
    </section>
  );
}
