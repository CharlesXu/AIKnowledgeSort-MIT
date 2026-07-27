import type { DiscoveryProposal } from "../drop/types";

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
  const visibleItems = proposal.items.slice(0, positions.length);

  return (
    <section aria-label="Proposal topology" className="proposal-topology">
      <div className="proposal-topology__notice">
        <strong>Proposal topology</strong>
        <span>Not yet ingested</span>
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
          Review
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
          aria-label="Play knowledge timeline"
          disabled
          title="Available after confirmed ingestion"
          type="button"
        >
          ▶
        </button>
        <input
          aria-label="Knowledge timeline position"
          disabled
          max="100"
          min="0"
          type="range"
          value="100"
          readOnly
        />
        <select
          aria-label="Knowledge timeline speed"
          defaultValue="1"
          disabled
        >
          <option value="1">1×</option>
        </select>
      </div>
      <p className="knowledge-timeline__status">
        Timeline playback is available after confirmed ingestion.
      </p>
    </section>
  );
}
