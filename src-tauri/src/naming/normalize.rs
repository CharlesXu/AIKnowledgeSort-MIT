use crate::naming::schema::{
    NamingFactKind, NamingPolicy, NamingProposal, NamingRequest, NamingReviewReason, NamingStatus,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;

const APPLIED_RULE: &str = "ordered-cited-facts-v1";
const MAX_CANONICAL_NAME_BYTES: usize = 255;

pub fn propose_name(
    policy: &NamingPolicy,
    request: &NamingRequest,
) -> Result<NamingProposal, String> {
    request.validate()?;

    let mut values = HashMap::<NamingFactKind, String>::new();
    for fact in &request.facts {
        let normalized = normalize_token(&fact.value, policy.separator);
        if let Some(existing) = values.get(&fact.kind) {
            if existing != &normalized {
                return Ok(review_proposal(
                    policy,
                    request,
                    NamingReviewReason::ConflictingEvidence,
                ));
            }
        } else {
            values.insert(fact.kind, normalized);
        }
    }

    if policy
        .required_facts
        .iter()
        .any(|kind| !values.contains_key(kind))
    {
        return Ok(review_proposal(
            policy,
            request,
            NamingReviewReason::MissingEvidence,
        ));
    }
    if policy
        .required_facts
        .iter()
        .any(|kind| values.get(kind).map_or(true, String::is_empty))
    {
        return Ok(review_proposal(
            policy,
            request,
            NamingReviewReason::UnsafeName,
        ));
    }

    let stem = [
        NamingFactKind::Project,
        NamingFactKind::Model,
        NamingFactKind::Regulation,
        NamingFactKind::Version,
        NamingFactKind::Subject,
    ]
    .iter()
    .filter_map(|kind| values.get(kind))
    .filter(|value| !value.is_empty())
    .cloned()
    .collect::<Vec<_>>()
    .join(&policy.separator.to_string());

    let extension = match original_extension(&request.original_name) {
        Ok(extension) => extension,
        Err(()) => {
            return Ok(review_proposal(
                policy,
                request,
                NamingReviewReason::UnsafeName,
            ))
        }
    };
    if !is_safe_stem(&stem) || !is_safe_extension(extension) {
        return Ok(review_proposal(
            policy,
            request,
            NamingReviewReason::UnsafeName,
        ));
    }

    let proposed = format!("{stem}{extension}");
    if proposed.len() > MAX_CANONICAL_NAME_BYTES {
        return Ok(review_proposal(
            policy,
            request,
            NamingReviewReason::UnsafeName,
        ));
    }

    let occupied = request
        .occupied_names
        .iter()
        .map(|name| comparison_key(name))
        .collect::<HashSet<_>>();
    let canonical_name = if occupied.contains(&comparison_key(&proposed)) {
        let suffix = &request.identity.digest[..8];
        let collision_name = format!("{stem}--{suffix}{extension}");
        if collision_name.len() > MAX_CANONICAL_NAME_BYTES
            || occupied.contains(&comparison_key(&collision_name))
        {
            return Ok(review_proposal(
                policy,
                request,
                NamingReviewReason::Collision,
            ));
        }
        collision_name
    } else {
        proposed
    };

    Ok(NamingProposal {
        proposal_id: proposal_id(policy, request, Some(&canonical_name), None),
        item_id: request.item_id.clone(),
        original_name: request.original_name.clone(),
        canonical_name: Some(canonical_name),
        identity: request.identity.clone(),
        policy_id: policy.policy_id.to_owned(),
        policy_version: policy.version.to_owned(),
        applied_rule: APPLIED_RULE.to_owned(),
        status: NamingStatus::Proposed,
        review_reason: None,
        facts: request.facts.clone(),
    })
}

fn review_proposal(
    policy: &NamingPolicy,
    request: &NamingRequest,
    reason: NamingReviewReason,
) -> NamingProposal {
    NamingProposal {
        proposal_id: proposal_id(policy, request, None, Some(reason)),
        item_id: request.item_id.clone(),
        original_name: request.original_name.clone(),
        canonical_name: None,
        identity: request.identity.clone(),
        policy_id: policy.policy_id.to_owned(),
        policy_version: policy.version.to_owned(),
        applied_rule: APPLIED_RULE.to_owned(),
        status: NamingStatus::NamingReview,
        review_reason: Some(reason),
        facts: request.facts.clone(),
    }
}

fn proposal_id(
    policy: &NamingPolicy,
    request: &NamingRequest,
    canonical_name: Option<&str>,
    reason: Option<NamingReviewReason>,
) -> String {
    let mut hasher = Sha256::new();
    for value in [
        policy.policy_id,
        policy.version,
        request.item_id.as_str(),
        request.original_name.as_str(),
        request.identity.algorithm.as_str(),
        request.identity.digest.as_str(),
        canonical_name.unwrap_or_default(),
        review_reason_code(reason),
    ] {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    for fact in &request.facts {
        hasher.update(fact_kind_code(fact.kind).as_bytes());
        hasher.update([0]);
        hasher.update(fact.value.as_bytes());
        hasher.update([0]);
        hasher.update(fact.evidence_location.as_bytes());
        hasher.update([0]);
    }
    format!("naming-{:x}", hasher.finalize())
}

fn fact_kind_code(kind: NamingFactKind) -> &'static str {
    match kind {
        NamingFactKind::Project => "project",
        NamingFactKind::Model => "model",
        NamingFactKind::Regulation => "regulation",
        NamingFactKind::Version => "version",
        NamingFactKind::Subject => "subject",
    }
}

fn review_reason_code(reason: Option<NamingReviewReason>) -> &'static str {
    match reason {
        None => "proposed",
        Some(NamingReviewReason::MissingEvidence) => "missing-evidence",
        Some(NamingReviewReason::ConflictingEvidence) => "conflicting-evidence",
        Some(NamingReviewReason::UnsafeName) => "unsafe-name",
        Some(NamingReviewReason::Collision) => "collision",
    }
}

fn normalize_token(value: &str, separator: char) -> String {
    let mut normalized = String::new();
    let mut pending_separator = false;
    for character in value.nfc() {
        if character.is_alphanumeric() || matches!(character, '.' | '_' | '-') {
            if pending_separator && !normalized.is_empty() {
                normalized.push(separator);
            }
            normalized.push(character);
            pending_separator = false;
        } else {
            pending_separator = true;
        }
    }
    normalized
        .trim_matches(|character| matches!(character, '.' | '_' | '-') || character == separator)
        .to_owned()
}

fn original_extension(original_name: &str) -> Result<&str, ()> {
    match original_name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            Ok(&original_name[stem.len()..])
        }
        Some((_, "")) => Err(()),
        _ => Ok(""),
    }
}

fn is_safe_stem(stem: &str) -> bool {
    if stem.is_empty() || stem.ends_with([' ', '.']) {
        return false;
    }
    let device_stem = stem.split('.').next().unwrap_or_default().to_uppercase();
    !is_windows_reserved(&device_stem)
}

fn is_windows_reserved(stem: &str) -> bool {
    matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .and_then(|value| value.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
        || stem
            .strip_prefix("LPT")
            .and_then(|value| value.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
}

fn is_safe_extension(extension: &str) -> bool {
    extension
        .chars()
        .all(|character| character == '.' || character.is_alphanumeric())
}

fn comparison_key(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfc()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::propose_name;
    use crate::identity::ContentIdentity;
    use crate::naming::schema::{
        canonical_policy, NamingFact, NamingFactKind, NamingRequest, NamingReviewReason,
        NamingStatus,
    };

    fn identity() -> ContentIdentity {
        ContentIdentity {
            algorithm: "SHA-256".to_owned(),
            digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
        }
    }

    fn fact(kind: NamingFactKind, value: &str) -> NamingFact {
        NamingFact {
            kind,
            value: value.to_owned(),
            evidence_location: "page:1".to_owned(),
        }
    }

    fn request(original_name: &str, facts: Vec<NamingFact>) -> NamingRequest {
        NamingRequest {
            item_id: "item-1".to_owned(),
            original_name: original_name.to_owned(),
            identity: identity(),
            facts,
            occupied_names: Vec::new(),
        }
    }

    fn complete_facts(subject: &str) -> Vec<NamingFact> {
        vec![
            fact(NamingFactKind::Project, "Atlas"),
            fact(NamingFactKind::Model, "X100"),
            fact(NamingFactKind::Version, "V2.1"),
            fact(NamingFactKind::Subject, subject),
        ]
    }

    #[test]
    fn proposes_tokens_in_policy_order_and_preserves_extension_case() {
        let proposal = propose_name(
            &canonical_policy(),
            &request("000123.PDF", complete_facts("Reset reliability")),
        )
        .expect("proposal");

        assert_eq!(
            proposal.canonical_name.as_deref(),
            Some("Atlas-X100-V2.1-Reset-reliability.PDF")
        );
        assert_eq!(proposal.status, NamingStatus::Proposed);
        assert_eq!(proposal.review_reason, None);
    }

    #[test]
    fn routes_conflicting_or_missing_evidence_to_review() {
        let mut conflicting = complete_facts("Reset reliability");
        conflicting.push(fact(NamingFactKind::Subject, "Clock reliability"));
        let conflict = propose_name(&canonical_policy(), &request("000123.pdf", conflicting))
            .expect("conflict proposal");
        assert_eq!(conflict.canonical_name, None);
        assert_eq!(conflict.status, NamingStatus::NamingReview);
        assert_eq!(
            conflict.review_reason,
            Some(NamingReviewReason::ConflictingEvidence)
        );

        let missing = propose_name(
            &canonical_policy(),
            &request("000123.pdf", vec![fact(NamingFactKind::Project, "Atlas")]),
        )
        .expect("missing proposal");
        assert_eq!(missing.canonical_name, None);
        assert_eq!(
            missing.review_reason,
            Some(NamingReviewReason::MissingEvidence)
        );
    }

    #[test]
    fn canonically_equivalent_unicode_produces_the_same_nfc_name() {
        let composed = propose_name(
            &canonical_policy(),
            &request("a.md", vec![fact(NamingFactKind::Subject, "Café")]),
        )
        .expect("composed");
        let decomposed = propose_name(
            &canonical_policy(),
            &request("a.md", vec![fact(NamingFactKind::Subject, "Cafe\u{301}")]),
        )
        .expect("decomposed");

        assert_eq!(composed.canonical_name, decomposed.canonical_name);
        assert_eq!(composed.canonical_name.as_deref(), Some("Café.md"));
    }

    #[test]
    fn collapses_unsafe_runs_without_dropping_meaningful_tokens() {
        let proposal = propose_name(
            &canonical_policy(),
            &request(
                "a.md",
                vec![fact(
                    NamingFactKind::Subject,
                    "Motor: reset / reliability???",
                )],
            ),
        )
        .expect("proposal");

        assert_eq!(
            proposal.canonical_name.as_deref(),
            Some("Motor-reset-reliability.md")
        );
    }

    #[test]
    fn routes_empty_or_windows_reserved_stems_to_review() {
        for subject in [":://??", "CON"] {
            let proposal = propose_name(
                &canonical_policy(),
                &request("a.txt", vec![fact(NamingFactKind::Subject, subject)]),
            )
            .expect("review proposal");

            assert_eq!(proposal.canonical_name, None);
            assert_eq!(proposal.review_reason, Some(NamingReviewReason::UnsafeName));
        }
    }

    #[test]
    fn resolves_one_case_insensitive_collision_from_the_content_digest() {
        let mut request = request("a.pdf", vec![fact(NamingFactKind::Subject, "Report")]);
        request.occupied_names = vec!["report.PDF".to_owned()];

        let proposal = propose_name(&canonical_policy(), &request).expect("proposal");

        assert_eq!(
            proposal.canonical_name.as_deref(),
            Some("Report--01234567.pdf")
        );
    }

    #[test]
    fn routes_a_digest_suffix_collision_to_review() {
        let mut request = request("a.pdf", vec![fact(NamingFactKind::Subject, "Report")]);
        request.occupied_names = vec!["Report.pdf".to_owned(), "REPORT--01234567.PDF".to_owned()];

        let proposal = propose_name(&canonical_policy(), &request).expect("proposal");

        assert_eq!(proposal.canonical_name, None);
        assert_eq!(proposal.review_reason, Some(NamingReviewReason::Collision));
    }

    #[test]
    fn binds_the_proposal_id_to_exact_evidence() {
        let first = request("a.md", vec![fact(NamingFactKind::Subject, "Report")]);
        let mut second = first.clone();
        second.facts[0].evidence_location = "page:2".to_owned();

        let first = propose_name(&canonical_policy(), &first).expect("first proposal");
        let second = propose_name(&canonical_policy(), &second).expect("second proposal");

        assert_eq!(first.canonical_name, second.canonical_name);
        assert_ne!(first.proposal_id, second.proposal_id);
    }
}
