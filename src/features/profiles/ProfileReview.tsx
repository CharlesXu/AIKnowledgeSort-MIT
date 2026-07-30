import { useEffect, useState } from "react";
import {
  statusTranslationKey,
  useI18n,
} from "../../i18n/I18nContext";
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
  const { t } = useI18n();
  const ruleChanges = [
    ...candidate.diff.addedRuleIds.map((ruleId) => ["+", ruleId] as const),
    ...candidate.diff.changedRuleIds.map((ruleId) => ["~", ruleId] as const),
    ...candidate.diff.removedRuleIds.map((ruleId) => ["−", ruleId] as const),
  ];
  const categorySummary = t("profile.taxonomyDiff", {
    added: candidate.diff.addedCategoryIds.length,
    changed: candidate.diff.changedCategoryIds.length,
    removed: candidate.diff.removedCategoryIds.length,
  });

  return (
    <>
      <p className="profile-taxonomy-diff">{categorySummary}</p>
      <ul aria-label={t("profile.ruleChanges")} className="profile-diff">
      {ruleChanges.length === 0 ? (
        <li><span>·</span>{t("profile.noRuleChanges")}</li>
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
  const { t } = useI18n();
  const [state, setState] = useState<ProfileStateSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [profileUrl, setProfileUrl] = useState("");
  const [compilerConfigId, setCompilerConfigId] = useState("");
  const [compilerVersion, setCompilerVersion] = useState("");
  const [compilerSourceTitle, setCompilerSourceTitle] = useState("");
  const [compilerBaseKey, setCompilerBaseKey] = useState(
    `${bundledDraft.profileId}@${bundledDraft.version}`,
  );
  const [compilerOwnership, setCompilerOwnership] = useState<
    "owned" | "firstPartyAuthorized"
  >("owned");
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
  const installedProfiles = state?.installed.length
    ? state.installed
    : [bundledDraft];
  const compilerBase = installedProfiles.find(
    (profile) => `${profile.profileId}@${profile.version}` === compilerBaseKey,
  ) ?? installed;
  const candidate = state?.candidates[0] ?? null;
  const installedStatusKey = statusTranslationKey(installed.status);

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

  async function compileCandidate(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const compiled = await client.compileLocalCandidate({
        configId: compilerConfigId.trim(),
        profileId: compilerBase.profileId,
        version: compilerVersion.trim(),
        title: compilerBase.title,
        sourceTitle: compilerSourceTitle.trim(),
        ownership: compilerOwnership,
        baseProfileId: compilerBase.profileId,
        baseProfileVersion: compilerBase.version,
      });
      if (compiled) {
        setReviewedCandidateId(null);
        setState(await client.inspect());
      }
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
          <h3 id="profile-mode">{t("profile.classificationMode")}</h3>
          <span className={`profile-status profile-status--${installed.status}`}>
            {installedStatusKey
              ? t(installedStatusKey)
              : installed.status.toUpperCase()}
          </span>
        </div>
        <strong className="profile-title">{installed.title}</strong>
        <p className="profile-meta">
          {installed.version}
        </p>
        <p className="profile-meta">
          {t("profile.categories", { count: installed.categoryCount })} ·{" "}
          {installed.taxonomyCounts.level1} / {installed.taxonomyCounts.level2}
          {" / "}
          {installed.taxonomyCounts.level3} / {installed.taxonomyCounts.level4}
        </p>
        <p className="profile-meta">
          {installed.ruleCount === 0
            ? t("profile.noExecutableRules")
            : t("profile.executableRules", { count: installed.ruleCount })}
        </p>
        <ul aria-label={t("profile.governance")} className="profile-policy">
          {installed.uniquePrimaryArchiveCategory ? (
            <li>{t("profile.onePrimary")}</li>
          ) : null}
          {installed.crossDomainKnowledgeLinks ? (
            <li>{t("profile.crossDomain")}</li>
          ) : null}
        </ul>
        <p className="profile-provenance">{installed.provenanceTitle}</p>
        {state?.active ? (
          <span className="profile-active">
            {t("profile.approvedActive", { version: state.active.version })}
          </span>
        ) : (
          <span className="profile-inactive">
            {t("profile.draftInactive")}
          </span>
        )}
      </section>

      <section className="context-section" aria-labelledby="profile-import">
        <div className="profile-section-heading">
          <h3 id="profile-import">{t("profile.candidateImport")}</h3>
          <button
            className="profile-import-button"
            disabled={busy}
            onClick={() => void importCandidate()}
            type="button"
          >
            {t("profile.importLocal")}
          </button>
        </div>
        <p className="profile-help">
          {t("profile.declarativeHelp")}
        </p>
        <p className="profile-help">
          {t("profile.httpsHelp")}
        </p>
        <div className="profile-url-import">
          <label htmlFor="profile-url">{t("profile.url")}</label>
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
              {t("profile.importUrl")}
            </button>
          </div>
        </div>
      </section>

      <section className="context-section" aria-labelledby="profile-compiler">
        <div className="profile-section-heading">
          <h3 id="profile-compiler">{t("profile.compiler")}</h3>
          <span className="profile-status profile-status--candidate">
            {t("profile.reviewOnly")}
          </span>
        </div>
        <p className="profile-help">
          {t("profile.compilerHelp")}
        </p>
        <div className="profile-url-import">
          <label htmlFor="compiler-base">{t("profile.baseProfile")}</label>
          <select
            disabled={busy}
            id="compiler-base"
            onChange={(event) => setCompilerBaseKey(event.target.value)}
            value={`${compilerBase.profileId}@${compilerBase.version}`}
          >
            {installedProfiles.map((profile) => (
              <option
                key={`${profile.profileId}@${profile.version}`}
                value={`${profile.profileId}@${profile.version}`}
              >
                {profile.title} · {profile.version}
              </option>
            ))}
          </select>
          <label htmlFor="compiler-config">{t("profile.modelConfigId")}</label>
          <input
            autoComplete="off"
            disabled={busy}
            id="compiler-config"
            onChange={(event) => setCompilerConfigId(event.target.value)}
            placeholder="local-compiler"
            spellCheck={false}
            value={compilerConfigId}
          />
          <label htmlFor="compiler-version">{t("profile.candidateVersion")}</label>
          <input
            autoComplete="off"
            disabled={busy}
            id="compiler-version"
            onChange={(event) => setCompilerVersion(event.target.value)}
            placeholder="0.4.0-candidate"
            spellCheck={false}
            value={compilerVersion}
          />
          <label htmlFor="compiler-source-title">{t("profile.sourceTitle")}</label>
          <input
            autoComplete="off"
            disabled={busy}
            id="compiler-source-title"
            onChange={(event) => setCompilerSourceTitle(event.target.value)}
            placeholder={t("profile.sourceTitlePlaceholder")}
            value={compilerSourceTitle}
          />
          <label htmlFor="compiler-ownership">{t("profile.sourceAuthority")}</label>
          <select
            disabled={busy}
            id="compiler-ownership"
            onChange={(event) => setCompilerOwnership(
              event.target.value as "owned" | "firstPartyAuthorized",
            )}
            value={compilerOwnership}
          >
            <option value="owned">{t("profile.owned")}</option>
            <option value="firstPartyAuthorized">{t("profile.firstPartyAuthorized")}</option>
          </select>
          <button
            className="profile-import-button"
            disabled={
              busy
              || compilerConfigId.trim().length === 0
              || compilerVersion.trim().length === 0
              || compilerSourceTitle.trim().length === 0
            }
            onClick={() => void compileCandidate()}
            type="button"
          >
            {t("profile.compileLocal")}
          </button>
        </div>
      </section>

      {candidate ? (
        <section className="context-section" aria-labelledby="profile-candidate">
          <div className="profile-section-heading">
            <h3 id="profile-candidate">{candidate.sourceBasename}</h3>
            <span className={`profile-status profile-status--${candidate.status}`}>
              {statusTranslationKey(candidate.status)
                ? t(statusTranslationKey(candidate.status)!)
                : candidate.status.toUpperCase()}
            </span>
          </div>
          <p className="profile-meta">
            {candidate.profileId} · {candidate.profileVersion}
          </p>
          <p className="profile-meta">
            {candidate.sourceKind === "remoteUrl"
              ? t("profile.remoteUrl")
              : candidate.sourceKind === "modelGenerated"
                ? t("profile.modelGenerated")
                : t("profile.localFile")}
            {" · "}
            {candidate.sourceByteSize > 0
              ? t("profile.bytes", {
                  count: candidate.sourceByteSize.toLocaleString(),
                })
              : t("profile.sizeUnavailable")}
          </p>
          <code className="profile-digest" title={candidate.sourceIdentity.digest}>
            SHA-256 {candidate.sourceIdentity.digest.slice(0, 12)}…
          </code>
          {candidate.generation ? (
            <div className="profile-generation">
              <p className="profile-meta">
                {t("profile.source")} · {candidate.generation.originalSourceBasename}
                {" · "}
                {t("profile.bytes", {
                  count: candidate.generation.originalSourceByteSize.toLocaleString(),
                })}
              </p>
              <code
                className="profile-digest"
                title={candidate.generation.originalSourceIdentity.digest}
              >
                Source SHA-256{" "}
                {candidate.generation.originalSourceIdentity.digest.slice(0, 12)}…
              </code>
              <p className="profile-meta">
                {t("profile.modelSource")} · {candidate.generation.modelConfigId}
                {" · "}{t("profile.base")}{" "}
                {candidate.generation.base.profileId}@
                {candidate.generation.base.version}
              </p>
            </div>
          ) : null}
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
                {t("profile.reviewedDigest", {
                  digest: candidate.sourceIdentity.digest.slice(0, 12),
                })}
              </label>
              <div className="profile-actions">
                <button
                  aria-label={t("profile.rejectLabel")}
                  disabled={busy || reviewedCandidateId !== candidate.candidateId}
                  onClick={() => void decide("reject")}
                  type="button"
                >
                  {t("profile.reject")}
                </button>
                <button
                  aria-label={t("profile.approveLabel")}
                  className="profile-actions__approve"
                  disabled={busy || reviewedCandidateId !== candidate.candidateId}
                  onClick={() => void decide("approve")}
                  type="button"
                >
                  {t("profile.approveExact")}
                </button>
              </div>
            </>
          ) : null}
        </section>
      ) : (
        <section className="context-section context-section--deferred">
          <h3>{t("profile.noCandidate")}</h3>
          <p>{t("profile.noCandidateHelp")}</p>
        </section>
      )}

      {error ? (
        <p className="profile-error" role="alert">{error}</p>
      ) : null}
    </div>
  );
}
