import type { DiscoveryProposal } from "../drop/types";
import { useI18n } from "../../i18n/I18nContext";

interface ProposalTopologyProps {
  readonly proposal: DiscoveryProposal;
}

const positions = [
  { x: 72, y: 24 },
  { x: 78, y: 67 },
  { x: 34, y: 76 },
  { x: 20, y: 35 },
  { x: 50, y: 13 },
] as const;

export function ProposalTopology({ proposal }: ProposalTopologyProps) {
  const { t } = useI18n();
  const visibleItems = proposal.items.slice(0, positions.length);

  return (
    <section aria-label={t("topology.label")} className="proposal-topology">
      <div className="proposal-topology__notice">
        <strong>{t("topology.label")}</strong>
        <span>{t("topology.notIngested")}</span>
      </div>
      <div aria-hidden="true" className="proposal-topology__canvas">
        <svg preserveAspectRatio="none" viewBox="0 0 100 100">
          {visibleItems.map((item, index) => (
            <line
              key={item.path}
              x1="50"
              x2={positions[index].x}
              y1="48"
              y2={positions[index].y}
            />
          ))}
        </svg>
        <span
          className="proposal-topology__node proposal-topology__node--root"
          style={{ left: "50%", top: "48%" }}
        >
          {t("topology.review")}
        </span>
        {visibleItems.map((item, index) => (
          <span
            className="proposal-topology__node"
            key={item.path}
            style={{
              left: `${positions[index].x}%`,
              top: `${positions[index].y}%`,
            }}
            title={item.path}
          >
            {item.name}
          </span>
        ))}
      </div>
      <div className="knowledge-timeline">
        <button
          aria-label={t("graph.playTimeline")}
          disabled
          title={t("topology.availableAfterIngestion")}
          type="button"
        >
          ▶
        </button>
        <input
          aria-label={t("graph.timelinePosition")}
          disabled
          max="100"
          min="0"
          type="range"
          value="100"
          readOnly
        />
        <select
          aria-label={t("graph.timelineSpeed")}
          defaultValue="1"
          disabled
        >
          <option value="1">1×</option>
        </select>
      </div>
      <p className="knowledge-timeline__status">
        {t("topology.timelineHelp")}
      </p>
    </section>
  );
}
