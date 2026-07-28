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
  version: "0.3.0-draft",
  title: "Ninebot document and electronic archive classification",
  status: "draft",
  ruleCount: 0,
  categoryCount: 466,
  taxonomyCounts: {
    level1: 14,
    level2: 94,
    level3: 179,
    level4: 179,
  },
  semanticEvidenceRequired: true,
  uniquePrimaryArchiveCategory: true,
  crossDomainKnowledgeLinks: true,
  provenanceTitle: "九号公司文档与电子档案管理规范（讨论稿 V0.9.0-rc.3）",
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function CandidateDiff({ candidate }: {
  readonly candidate: ProfileCandidateRecord;
}) {
  const ruleChanges = [
    ...candidate.diff.addedRuleIds.map((ruleId) => ["+", ruleId] as const),
    ...candidate.diff.changedRuleIds.map((ruleId) => ["~", ruleId] as const),
    ...candidate.diff.removedRuleIds.map((ruleId) => ["−", ruleId] as const),
  ];
  const categorySummary = `Taxonomy +${candidate.diff.addedCategoryIds.length}`
    + ` · ~${candidate.diff.changedCategoryIds.length}`
    + ` · −${candidate.diff.removedCategoryIds.length}`;

  return (
    <>
      <p className="profile-taxonomy-diff">{categorySummary}</p>
      <ul aria-label="Candidate rule changes" className="profile-diff">
      {ruleChanges.length === 0 ? (
        <li><span>·</span>No rule changes</li>
      ) : ruleChanges.map(([marker, ruleId]) => (
        <li key={`${marker}-${ruleId}`}>
          <span aria-hidden="true">{marker}</span>
          {ruleId}
        </li>
      ))}
      </ul>
    </>
  );
}

export function ProfileReview({ client }: {
  readonly client: ProfileClient;
}) {
  const [state, setState] = useState<ProfileStateSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [profileUrl, setProfileUrl] = useState("");
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

  async function importUrlCandidate(): Promise<void> {
    const url = profileUrl.trim();
    if (!url) return;
    setProfileUrl("");
    setBusy(true);
    setError(null);
    try {
      await client.importUrlCandidate(url);
      setReviewedCandidateId(null);
      setState(await client.inspect());
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
          {installed.version}
        </p>
        <p className="profile-meta">
          {installed.categoryCount} categories ·{" "}
          {installed.taxonomyCounts.level1} / {installed.taxonomyCounts.level2}
          {" / "}
          {installed.taxonomyCounts.level3} / {installed.taxonomyCounts.level4}
        </p>
        <p className="profile-meta">
          {installed.ruleCount === 0
            ? "0 executable rules — semantic review required"
            : `${installed.ruleCount} executable rules`}
        </p>
        <ul aria-label="Profile governance" className="profile-policy">
          {installed.uniquePrimaryArchiveCategory ? (
            <li>One primary archive category</li>
          ) : null}
          {installed.crossDomainKnowledgeLinks ? (
            <li>Cross-domain knowledge links</li>
          ) : null}
        </ul>
        <p className="profile-provenance">{installed.provenanceTitle}</p>
        {state?.active ? (
          <span className="profile-active">
            Approved and active · {state.active.version}
          </span>
        ) : (
          <span className="profile-inactive">
            Discussion draft — not approved or active
          </span>
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
        <p className="profile-help">
          HTTPS JSON only. Query and fragment values are never retained.
        </p>
        <div className="profile-url-import">
          <label htmlFor="profile-url">Profile URL</label>
          <div className="profile-url-import__controls">
            <input
              autoComplete="off"
              disabled={busy}
              id="profile-url"
              onChange={(event) => setProfileUrl(event.target.value)}
              placeholder="https://…/profile.json"
              spellCheck={false}
              type="url"
              value={profileUrl}
            />
            <button
              className="profile-import-button"
              disabled={busy || profileUrl.trim().length === 0}
              onClick={() => void importUrlCandidate()}
              type="button"
            >
              Import URL
            </button>
          </div>
        </div>
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
          <p className="profile-meta">
            {candidate.sourceKind === "remoteUrl" ? "Remote URL" : "Local file"}
            {" · "}
            {candidate.sourceByteSize > 0
              ? `${candidate.sourceByteSize.toLocaleString()} bytes`
              : "size unavailable"}
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
