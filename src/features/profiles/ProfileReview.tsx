import { useEffect, useState } from "react";
import type {
  ProfileCandidateRecord,
  ProfileClient,
  ProfileDecision,
  ProfileStateSummary,
  ProfileSummary,
} from "./types";

const bundledDraft: ProfileSummary = {
  profileId: "ninebot-electronic-archive",
  version: "0.1.0-draft",
  title: "Ninebot electronic archive",
  status: "draft",
  ruleCount: 0,
  provenanceTitle: "AI Knowledge Sort clean implementation handoff",
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function CandidateDiff({ candidate }: {
  readonly candidate: ProfileCandidateRecord;
}) {
  const changes = [
    ...candidate.diff.addedRuleIds.map((ruleId) => ["+", ruleId] as const),
    ...candidate.diff.changedRuleIds.map((ruleId) => ["~", ruleId] as const),
    ...candidate.diff.removedRuleIds.map((ruleId) => ["−", ruleId] as const),
  ];

  return (
    <ul aria-label="Candidate rule changes" className="profile-diff">
      {changes.length === 0 ? (
        <li><span>·</span>No rule changes</li>
      ) : changes.map(([marker, ruleId]) => (
        <li key={`${marker}-${ruleId}`}>
          <span aria-hidden="true">{marker}</span>
          {ruleId}
        </li>
      ))}
    </ul>
  );
}

export function ProfileReview({ client }: {
  readonly client: ProfileClient;
}) {
  const [state, setState] = useState<ProfileStateSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reviewedCandidateId, setReviewedCandidateId] = useState<string | null>(
    null,
  );

  useEffect(() => {
    let active = true;
    client.inspect()
      .then((nextState) => {
        if (active) setState(nextState);
      })
      .catch((reason: unknown) => {
        if (active) setError(errorMessage(reason));
      });
    return () => {
      active = false;
    };
  }, [client]);

  const installed = state?.installed.find(
    (profile) => profile.profileId === bundledDraft.profileId
      && profile.version === bundledDraft.version,
  ) ?? bundledDraft;
  const candidate = state?.candidates[0] ?? null;

  async function importCandidate(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const imported = await client.importLocalCandidate();
      if (imported) setState(await client.inspect());
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function decide(decision: ProfileDecision): Promise<void> {
    if (!candidate) return;
    setBusy(true);
    setError(null);
    try {
      setState(await client.decideCandidate({
        candidateId: candidate.candidateId,
        reviewedDigest: candidate.sourceIdentity.digest,
        decision,
      }));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="profile-review">
      <section className="context-section" aria-labelledby="profile-mode">
        <div className="profile-section-heading">
          <h3 id="profile-mode">Classification mode</h3>
          <span className={`profile-status profile-status--${installed.status}`}>
            {installed.status.toUpperCase()}
          </span>
        </div>
        <strong className="profile-title">{installed.title}</strong>
        <p className="profile-meta">
          {installed.version} · {installed.ruleCount === 0
            ? "0 rules — classification disabled"
            : `${installed.ruleCount} rules`}
        </p>
        <p className="profile-provenance">{installed.provenanceTitle}</p>
        {state?.active ? (
          <span className="profile-active">
            Approved and active · {state.active.version}
          </span>
        ) : (
          <span className="profile-inactive">No approved profile active</span>
        )}
      </section>

      <section className="context-section" aria-labelledby="profile-import">
        <div className="profile-section-heading">
          <h3 id="profile-import">Candidate import</h3>
          <button
            className="profile-import-button"
            disabled={busy}
            onClick={() => void importCandidate()}
            type="button"
          >
            Import local profile
          </button>
        </div>
        <p className="profile-help">
          Declarative data only. Source bytes and digest remain in the Vault.
        </p>
      </section>

      {candidate ? (
        <section className="context-section" aria-labelledby="profile-candidate">
          <div className="profile-section-heading">
            <h3 id="profile-candidate">{candidate.sourceBasename}</h3>
            <span className={`profile-status profile-status--${candidate.status}`}>
              {candidate.status.toUpperCase()}
            </span>
          </div>
          <p className="profile-meta">
            {candidate.profileId} · {candidate.profileVersion}
          </p>
          <code className="profile-digest" title={candidate.sourceIdentity.digest}>
            SHA-256 {candidate.sourceIdentity.digest.slice(0, 12)}…
          </code>
          <CandidateDiff candidate={candidate} />
          {candidate.status === "unapproved" ? (
            <>
              <label className="profile-digest-confirmation">
                <input
                  checked={reviewedCandidateId === candidate.candidateId}
                  onChange={(event) => setReviewedCandidateId(
                    event.target.checked ? candidate.candidateId : null,
                  )}
                  type="checkbox"
                />
                I reviewed SHA-256 digest {candidate.sourceIdentity.digest.slice(0, 12)}…
              </label>
              <div className="profile-actions">
                <button
                  aria-label="Reject profile"
                  disabled={busy || reviewedCandidateId !== candidate.candidateId}
                  onClick={() => void decide("reject")}
                  type="button"
                >
                  Reject
                </button>
                <button
                  aria-label="Approve profile"
                  className="profile-actions__approve"
                  disabled={busy || reviewedCandidateId !== candidate.candidateId}
                  onClick={() => void decide("approve")}
                  type="button"
                >
                  Approve exact digest
                </button>
              </div>
            </>
          ) : null}
        </section>
      ) : (
        <section className="context-section context-section--deferred">
          <h3>No candidate awaiting review</h3>
          <p>Import a formal notice or draft exported as a profile JSON file.</p>
        </section>
      )}

      {error ? (
        <p className="profile-error" role="alert">{error}</p>
      ) : null}
    </div>
  );
}
