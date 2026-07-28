import { useEffect, useMemo, useState } from "react";
import type { GraphClient, GraphEvent, GraphRelation, GraphSnapshot } from "../graph/types";
import type { KnowledgeDocument } from "../knowledge/types";

interface KnowledgeGraphPaneProps {
  readonly client: GraphClient;
  readonly document: KnowledgeDocument;
}

interface RelationDraft {
  readonly sourceNode: string;
  readonly relationType: string;
  readonly targetNode: string;
  readonly startLine: string;
  readonly endLine: string;
}

const emptyDraft: RelationDraft = {
  sourceNode: "",
  relationType: "",
  targetNode: "",
  startLine: "1",
  endLine: "1",
};

const MAX_VISIBLE_NODES = 24;

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function eventProjection(events: readonly GraphEvent[]): readonly GraphEvent[] {
  const latest = new Map<string, GraphEvent>();
  for (const event of events) {
    latest.set(event.relationId, event);
  }
  return [...latest.values()].filter((event) => event.status !== "rejected");
}

function graphLayout(events: readonly GraphEvent[]): ReadonlyMap<string, { x: number; y: number }> {
  const names = [...new Set(events.flatMap((event) => [event.sourceNode, event.targetNode]))]
    .slice(0, MAX_VISIBLE_NODES);
  if (names.length === 1) return new Map([[names[0], { x: 50, y: 50 }]]);
  return new Map(names.map((name, index) => {
    const angle = (2 * Math.PI * index) / names.length - Math.PI / 2;
    return [name, { x: 50 + 34 * Math.cos(angle), y: 50 + 34 * Math.sin(angle) }];
  }));
}

export function KnowledgeGraphPane({ client, document }: KnowledgeGraphPaneProps) {
  const [snapshot, setSnapshot] = useState<GraphSnapshot | null>(null);
  const [draft, setDraft] = useState<RelationDraft>(emptyDraft);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [reason, setReason] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [position, setPosition] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);

  async function refresh(): Promise<void> {
    const next = await client.inspect({
      authorityId: document.authorityId,
      operationId: document.operationId,
    });
    setSnapshot(next);
    setPosition(next.events.length);
  }

  useEffect(() => {
    let active = true;
    setSnapshot(null);
    setSelectedId(null);
    setError(null);
    client.inspect({
      authorityId: document.authorityId,
      operationId: document.operationId,
    }).then((next) => {
      if (active) {
        setSnapshot(next);
        setPosition(next.events.length);
      }
    }).catch((nextError: unknown) => {
      if (active) setError(errorText(nextError));
    });
    return () => { active = false; };
  }, [client, document.authorityId, document.operationId]);

  useEffect(() => {
    if (!playing || snapshot === null || position >= snapshot.events.length) {
      if (snapshot !== null && position >= snapshot.events.length) setPlaying(false);
      return;
    }
    const timer = window.setTimeout(
      () => setPosition((current) => Math.min(current + 1, snapshot.events.length)),
      800 / speed,
    );
    return () => window.clearTimeout(timer);
  }, [playing, position, snapshot, speed]);

  const selected = snapshot?.relations.find((relation) => relation.relationId === selectedId);
  const visibleEvents = useMemo(
    () => eventProjection(snapshot?.events.slice(0, position) ?? []),
    [position, snapshot],
  );
  const nodeLayout = useMemo(() => graphLayout(visibleEvents), [visibleEvents]);

  function updateDraft(field: keyof RelationDraft, value: string): void {
    setDraft((current) => ({ ...current, [field]: value }));
  }

  async function propose(): Promise<void> {
    setPending(true);
    setError(null);
    try {
      await client.propose({
        authorityId: document.authorityId,
        operationId: document.operationId,
        knowledgeRevision: document.revision,
        sourceNode: draft.sourceNode,
        relationType: draft.relationType,
        targetNode: draft.targetNode,
        evidenceRanges: [{
          startLine: Number(draft.startLine),
          endLine: Number(draft.endLine),
        }],
      });
      await refresh();
      setDraft(emptyDraft);
    } catch (nextError) {
      setError(errorText(nextError));
    } finally {
      setPending(false);
    }
  }

  async function decide(decision: "accept" | "reject" | "revise"): Promise<void> {
    if (selected === undefined) return;
    setPending(true);
    setError(null);
    try {
      await client.decide({
        authorityId: document.authorityId,
        relationId: selected.relationId,
        expectedVersion: selected.version,
        decision,
        reason,
        revision: decision === "revise" ? {
          knowledgeRevision: document.revision,
          sourceNode: draft.sourceNode,
          relationType: draft.relationType,
          targetNode: draft.targetNode,
          evidenceRanges: [{
            startLine: Number(draft.startLine),
            endLine: Number(draft.endLine),
          }],
        } : null,
      });
      await refresh();
      setReason("");
    } catch (nextError) {
      setError(errorText(nextError));
    } finally {
      setPending(false);
    }
  }

  function selectRelation(relation: GraphRelation): void {
    setSelectedId(relation.relationId);
    const evidence = relation.evidence[0];
    setDraft({
      sourceNode: relation.sourceNode,
      relationType: relation.relationType,
      targetNode: relation.targetNode,
      startLine: String(evidence?.startLine ?? 1),
      endLine: String(evidence?.endLine ?? 1),
    });
    setReason("");
  }

  const activeEvent = position > 0 ? snapshot?.events[position - 1] : undefined;
  return (
    <section aria-label="Knowledge graph" className="knowledge-graph">
      <div className="knowledge-graph__notice">
        <div><strong>Evidence graph</strong><span>Vault revision {document.revision}</span></div>
        <span>{snapshot?.relations.length ?? 0} relations</span>
      </div>
      {error !== null ? <p className="knowledge-graph__error" role="alert">{error}</p> : null}
      <div aria-label="Knowledge network" className="knowledge-graph__canvas" role="img">
        <svg aria-hidden="true" preserveAspectRatio="none" viewBox="0 0 100 100">
          {visibleEvents.map((event) => {
            const source = nodeLayout.get(event.sourceNode);
            const target = nodeLayout.get(event.targetNode);
            return source && target ? (
              <g key={event.relationId}>
                <line x1={source.x} x2={target.x} y1={source.y} y2={target.y} />
                <text x={(source.x + target.x) / 2} y={(source.y + target.y) / 2}>
                  {event.relationType}
                </text>
              </g>
            ) : null;
          })}
        </svg>
        {[...nodeLayout.entries()].map(([name, point]) => (
          <span className="knowledge-graph__node" key={name} style={{ left: `${point.x}%`, top: `${point.y}%` }}>
            {name}
          </span>
        ))}
        {visibleEvents.length === 0 ? <span className="knowledge-graph__empty">No persisted relations</span> : null}
      </div>
      <div className="knowledge-timeline" data-height="34">
        <button
          aria-label={playing ? "Pause knowledge timeline" : "Play knowledge timeline"}
          disabled={(snapshot?.events.length ?? 0) === 0}
          onClick={() => {
            if (!playing && snapshot !== null && position >= snapshot.events.length) setPosition(0);
            setPlaying((current) => !current);
          }}
          type="button"
        >{playing ? "Ⅱ" : "▶"}</button>
        <input
          aria-label="Knowledge timeline position"
          disabled={(snapshot?.events.length ?? 0) === 0}
          max={snapshot?.events.length ?? 0}
          min="0"
          onChange={(event) => setPosition(Number(event.target.value))}
          type="range"
          value={position}
        />
        <select aria-label="Knowledge timeline speed" onChange={(event) => setSpeed(Number(event.target.value))} value={speed}>
          <option value="1">1×</option><option value="2">2×</option>
        </select>
      </div>
      <p className="knowledge-timeline__status">
        {activeEvent ? `${activeEvent.status} · ${new Date(activeEvent.recordedAtUnixMs).toLocaleString()}` : "Start of graph history"}
      </p>
      <form className="knowledge-graph__form" onSubmit={(event) => { event.preventDefault(); void propose(); }}>
        <input aria-label="Relation source node" onChange={(event) => updateDraft("sourceNode", event.target.value)} placeholder="Source node" value={draft.sourceNode} />
        <input aria-label="Relation type" onChange={(event) => updateDraft("relationType", event.target.value)} placeholder="Relation" value={draft.relationType} />
        <input aria-label="Relation target node" onChange={(event) => updateDraft("targetNode", event.target.value)} placeholder="Target node" value={draft.targetNode} />
        <div className="knowledge-graph__range">
          <input aria-label="Evidence start line" min="1" onChange={(event) => updateDraft("startLine", event.target.value)} type="number" value={draft.startLine} />
          <span>–</span>
          <input aria-label="Evidence end line" min="1" onChange={(event) => updateDraft("endLine", event.target.value)} type="number" value={draft.endLine} />
        </div>
        <button disabled={pending || document.revision === 0} type="submit">Add relation</button>
      </form>
      {document.revision === 0 ? <p className="knowledge-graph__hint">Save a Vault revision before adding relations.</p> : null}
      <div aria-label="Graph relations" className="knowledge-graph__relations" role="list">
        {snapshot?.relations.map((relation) => (
          <div key={relation.relationId} role="listitem">
            <button
              aria-pressed={selectedId === relation.relationId}
              onClick={() => selectRelation(relation)}
              type="button"
            >
              <strong>{relation.sourceNode} {relation.relationType} {relation.targetNode}</strong>
              <span>{relation.status} · v{relation.version}</span>
            </button>
          </div>
        ))}
      </div>
      {selected !== undefined ? (
        <section aria-label="Relation evidence" className="knowledge-graph__evidence">
          {selected.evidence.map((evidence) => (
            <blockquote key={`${evidence.startLine}-${evidence.endLine}`}>
              <span>Lines {evidence.startLine}–{evidence.endLine}</span>{evidence.text}
            </blockquote>
          ))}
          {selected.status === "review" ? (
            <div className="knowledge-graph__decisions">
              <input aria-label="Relation decision reason" onChange={(event) => setReason(event.target.value)} placeholder="Decision reason" value={reason} />
              <button disabled={pending} onClick={() => void decide("accept")} type="button">Accept</button>
              <button disabled={pending} onClick={() => void decide("revise")} type="button">Revise</button>
              <button disabled={pending} onClick={() => void decide("reject")} type="button">Reject</button>
            </div>
          ) : null}
        </section>
      ) : null}
    </section>
  );
}
