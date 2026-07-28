use crate::identity::ContentIdentity;
use crate::profiles::schema::{DeclarativeProfile, EvidenceKind, ProfileStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

const MAX_EVIDENCE_REFERENCES: usize = 256;
const MAX_EVIDENCE_LOCATION_BYTES: usize = 256;
const MAX_EVIDENCE_TEXT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceReference {
    pub kind: EvidenceKind,
    pub location: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidencePacket {
    pub source_identity: ContentIdentity,
    pub references: Vec<EvidenceReference>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceCitation {
    pub kind: EvidenceKind,
    pub location: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalStatus {
    Proposed,
    ClassificationReview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewReason {
    MissingEvidence,
    ConflictingRules,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationProposal {
    pub proposal_id: String,
    pub source_identity: ContentIdentity,
    pub profile_id: String,
    pub profile_version: String,
    pub status: ProposalStatus,
    pub rule_ids: Vec<String>,
    pub evidence: Vec<EvidenceCitation>,
    pub destination: Option<Vec<String>>,
    pub review_reason: Option<ReviewReason>,
    pub committable: bool,
}

struct RuleMatch {
    rule_id: String,
    destination: Vec<String>,
    citations: Vec<EvidenceCitation>,
}

pub fn classify(
    profile: &DeclarativeProfile,
    evidence: EvidencePacket,
) -> Result<ClassificationProposal, String> {
    profile.validate()?;
    validate_evidence(&evidence)?;

    let mut matches = Vec::new();
    for rule in &profile.rules {
        let mut citations = Vec::with_capacity(rule.all_of.len());
        let mut matched = true;
        for requirement in &rule.all_of {
            let term = requirement.term.to_lowercase();
            let reference = evidence.references.iter().find(|reference| {
                reference.kind == requirement.kind && reference.text.to_lowercase().contains(&term)
            });
            match reference {
                Some(reference) => citations.push(EvidenceCitation {
                    kind: reference.kind,
                    location: reference.location.clone(),
                }),
                None => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            matches.push(RuleMatch {
                rule_id: rule.rule_id.clone(),
                destination: rule.destination.clone(),
                citations,
            });
        }
    }

    let mut destinations: BTreeMap<Vec<String>, Vec<RuleMatch>> = BTreeMap::new();
    for matched_rule in matches {
        destinations
            .entry(matched_rule.destination.clone())
            .or_default()
            .push(matched_rule);
    }

    let proposal_id = Uuid::new_v4().simple().to_string();
    if destinations.len() == 1 {
        let (destination, matched_rules) = destinations
            .into_iter()
            .next()
            .expect("one destination exists");
        let (rule_ids, citations) = flatten_matches(matched_rules);
        return Ok(ClassificationProposal {
            proposal_id,
            source_identity: evidence.source_identity,
            profile_id: profile.profile_id.clone(),
            profile_version: profile.version.clone(),
            status: ProposalStatus::Proposed,
            rule_ids,
            evidence: citations,
            destination: Some(destination),
            review_reason: None,
            committable: profile.status == ProfileStatus::Approved,
        });
    }

    let reason = if destinations.is_empty() {
        ReviewReason::MissingEvidence
    } else {
        ReviewReason::ConflictingRules
    };
    let (rule_ids, citations) = flatten_matches(destinations.into_values().flatten().collect());
    Ok(ClassificationProposal {
        proposal_id,
        source_identity: evidence.source_identity,
        profile_id: profile.profile_id.clone(),
        profile_version: profile.version.clone(),
        status: ProposalStatus::ClassificationReview,
        rule_ids,
        evidence: citations,
        destination: None,
        review_reason: Some(reason),
        committable: false,
    })
}

fn validate_evidence(evidence: &EvidencePacket) -> Result<(), String> {
    evidence.source_identity.validate()?;
    if evidence.references.is_empty() || evidence.references.len() > MAX_EVIDENCE_REFERENCES {
        return Err("Classification evidence count is invalid".to_owned());
    }
    for reference in &evidence.references {
        if reference.location.trim().is_empty()
            || reference.location.len() > MAX_EVIDENCE_LOCATION_BYTES
            || reference.location.chars().any(char::is_control)
        {
            return Err("Classification evidence location is invalid".to_owned());
        }
        if reference.text.trim().is_empty() || reference.text.len() > MAX_EVIDENCE_TEXT_BYTES {
            return Err("Classification evidence text is invalid".to_owned());
        }
    }
    Ok(())
}

fn flatten_matches(matches: Vec<RuleMatch>) -> (Vec<String>, Vec<EvidenceCitation>) {
    let mut rule_ids = matches
        .iter()
        .map(|matched_rule| matched_rule.rule_id.clone())
        .collect::<Vec<_>>();
    rule_ids.sort();
    rule_ids.dedup();

    let mut citations = matches
        .into_iter()
        .flat_map(|matched_rule| matched_rule.citations)
        .collect::<Vec<_>>();
    citations.sort_by(|left, right| left.location.cmp(&right.location));
    citations.dedup();
    (rule_ids, citations)
}

#[cfg(test)]
mod tests {
    use super::{classify, EvidencePacket, EvidenceReference, ProposalStatus, ReviewReason};
    use crate::identity::ContentIdentity;
    use crate::profiles::schema::{
        ClassificationRule, DeclarativeProfile, EvidenceKind, EvidenceRequirement,
        ProfileOwnership, ProfileProvenance, ProfileStatus,
    };

    fn identity() -> ContentIdentity {
        ContentIdentity {
            algorithm: "SHA-256".to_owned(),
            digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0".to_owned(),
        }
    }

    fn rule(rule_id: &str, term: &str, destination: &[&str]) -> ClassificationRule {
        ClassificationRule {
            rule_id: rule_id.to_owned(),
            destination: destination
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            all_of: vec![EvidenceRequirement {
                kind: EvidenceKind::DocumentText,
                term: term.to_owned(),
            }],
        }
    }

    fn profile(status: ProfileStatus, rules: Vec<ClassificationRule>) -> DeclarativeProfile {
        DeclarativeProfile {
            schema_version: 1,
            profile_id: "fixture-profile".to_owned(),
            version: "1.0.0".to_owned(),
            title: "Fixture profile".to_owned(),
            status,
            provenance: ProfileProvenance {
                source_title: "Owned fixture".to_owned(),
                ownership: ProfileOwnership::Owned,
                evidence: vec!["authorization:test".to_owned()],
            },
            rules,
        }
    }

    fn evidence(text: &str) -> EvidencePacket {
        EvidencePacket {
            source_identity: identity(),
            references: vec![EvidenceReference {
                kind: EvidenceKind::DocumentText,
                location: "page:3".to_owned(),
                text: text.to_owned(),
            }],
        }
    }

    #[test]
    fn records_exact_draft_profile_rule_destination_and_evidence() {
        let proposal = classify(
            &profile(
                ProfileStatus::Draft,
                vec![rule(
                    "fixture.report",
                    "quarterly report",
                    &["01-Research", "Reports"],
                )],
            ),
            evidence("Quarterly report for Project Atlas"),
        )
        .expect("classify with draft profile");

        assert_eq!(proposal.profile_id, "fixture-profile");
        assert_eq!(proposal.profile_version, "1.0.0");
        assert_eq!(proposal.status, ProposalStatus::Proposed);
        assert_eq!(proposal.rule_ids, ["fixture.report"]);
        assert_eq!(
            proposal.destination,
            Some(vec!["01-Research".to_owned(), "Reports".to_owned()])
        );
        assert_eq!(proposal.evidence[0].location, "page:3");
        assert!(!proposal.committable);
    }

    #[test]
    fn routes_conflicting_destinations_to_dedicated_review_without_a_path() {
        let proposal = classify(
            &profile(
                ProfileStatus::Approved,
                vec![
                    rule("fixture.report", "quarterly", &["01-Research"]),
                    rule("fixture.finance", "atlas", &["02-Finance"]),
                ],
            ),
            evidence("Quarterly report for Project Atlas"),
        )
        .expect("classify conflicting evidence");

        assert_eq!(proposal.status, ProposalStatus::ClassificationReview);
        assert_eq!(proposal.review_reason, Some(ReviewReason::ConflictingRules));
        assert_eq!(proposal.rule_ids, ["fixture.finance", "fixture.report"]);
        assert_eq!(proposal.destination, None);
        assert!(!proposal.committable);
    }

    #[test]
    fn routes_missing_evidence_to_review_without_fabrication() {
        let proposal = classify(
            &profile(
                ProfileStatus::Approved,
                vec![rule("fixture.report", "quarterly report", &["01-Research"])],
            ),
            evidence("Unrelated content"),
        )
        .expect("classify insufficient evidence");

        assert_eq!(proposal.status, ProposalStatus::ClassificationReview);
        assert_eq!(proposal.review_reason, Some(ReviewReason::MissingEvidence));
        assert!(proposal.rule_ids.is_empty());
        assert!(proposal.destination.is_none());
        assert!(!proposal.committable);
    }

    #[test]
    fn only_one_approved_structurally_valid_match_is_committable() {
        let proposal = classify(
            &profile(
                ProfileStatus::Approved,
                vec![rule("fixture.report", "quarterly report", &["01-Research"])],
            ),
            evidence("Quarterly report"),
        )
        .expect("classify approved profile");

        assert_eq!(proposal.status, ProposalStatus::Proposed);
        assert!(proposal.committable);
    }
}
