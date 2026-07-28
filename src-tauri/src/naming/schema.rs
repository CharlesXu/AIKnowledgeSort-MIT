use crate::identity::ContentIdentity;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_FACTS: usize = 64;
pub const MAX_OCCUPIED_NAMES: usize = 10_000;
const MAX_ITEM_ID_BYTES: usize = 128;
const MAX_FILENAME_BYTES: usize = 255;
const MAX_FACT_VALUE_BYTES: usize = 512;
const MAX_EVIDENCE_LOCATION_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NamingFactKind {
    Project,
    Model,
    Regulation,
    Version,
    Subject,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamingFact {
    pub kind: NamingFactKind,
    pub value: String,
    pub evidence_location: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamingRequest {
    pub item_id: String,
    pub original_name: String,
    pub identity: ContentIdentity,
    pub facts: Vec<NamingFact>,
    pub occupied_names: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingPolicy {
    pub policy_id: &'static str,
    pub version: &'static str,
    pub required_facts: Vec<NamingFactKind>,
    pub separator: char,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NamingStatus {
    Proposed,
    NamingReview,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NamingReviewReason {
    MissingEvidence,
    ConflictingEvidence,
    UnsafeName,
    Collision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamingProposal {
    pub proposal_id: String,
    pub item_id: String,
    pub original_name: String,
    pub canonical_name: Option<String>,
    pub identity: ContentIdentity,
    pub policy_id: String,
    pub policy_version: String,
    pub applied_rule: String,
    pub status: NamingStatus,
    pub review_reason: Option<NamingReviewReason>,
    pub facts: Vec<NamingFact>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamingDecisionEvidence {
    pub naming_proposal_id: String,
    pub original_name: String,
    pub canonical_name: String,
    pub policy_id: String,
    pub policy_version: String,
    pub applied_rule: String,
    pub facts: Vec<NamingFact>,
}

pub fn canonical_policy() -> NamingPolicy {
    NamingPolicy {
        policy_id: "canonical-v1",
        version: "1.0.0",
        required_facts: vec![NamingFactKind::Subject],
        separator: '-',
    }
}

impl NamingRequest {
    pub fn validate(&self) -> Result<(), String> {
        validate_id(&self.item_id)?;
        validate_original_name(&self.original_name)?;
        self.identity.validate()?;

        if self.facts.is_empty() || self.facts.len() > MAX_FACTS {
            return Err("Naming fact count is invalid".to_owned());
        }
        let mut facts = HashSet::with_capacity(self.facts.len());
        for fact in &self.facts {
            validate_text(&fact.value, MAX_FACT_VALUE_BYTES, "Naming fact value")?;
            validate_text(
                &fact.evidence_location,
                MAX_EVIDENCE_LOCATION_BYTES,
                "Naming evidence location",
            )?;
            if !facts.insert(fact) {
                return Err("Naming request contains duplicate facts".to_owned());
            }
        }

        if self.occupied_names.len() > MAX_OCCUPIED_NAMES {
            return Err("Occupied name count exceeds the limit".to_owned());
        }
        for name in &self.occupied_names {
            validate_filename(name, "Occupied name")?;
        }
        Ok(())
    }
}

fn validate_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAX_ITEM_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Err("Naming item id is invalid".to_owned())
    } else {
        Ok(())
    }
}

fn validate_original_name(value: &str) -> Result<(), String> {
    validate_filename(value, "Original name")?;
    if value.contains(['/', '\\']) || matches!(value, "." | "..") {
        return Err("Original name must be one filename, not a path".to_owned());
    }
    Ok(())
}

fn validate_filename(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > MAX_FILENAME_BYTES
        || value.chars().any(char::is_control)
    {
        Err(format!("{label} is invalid"))
    } else {
        Ok(())
    }
}

fn validate_text(value: &str, max_bytes: usize, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        Err(format!("{label} is invalid"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_policy, NamingFact, NamingFactKind, NamingRequest, MAX_FACTS, MAX_OCCUPIED_NAMES,
    };
    use crate::identity::ContentIdentity;

    fn identity() -> ContentIdentity {
        ContentIdentity {
            algorithm: "SHA-256".to_owned(),
            digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
        }
    }

    fn fact(kind: NamingFactKind, value: &str, evidence_location: &str) -> NamingFact {
        NamingFact {
            kind,
            value: value.to_owned(),
            evidence_location: evidence_location.to_owned(),
        }
    }

    fn valid_request() -> NamingRequest {
        NamingRequest {
            item_id: "item-1".to_owned(),
            original_name: "000123.pdf".to_owned(),
            identity: identity(),
            facts: vec![
                fact(NamingFactKind::Project, "Atlas", "page:1"),
                fact(NamingFactKind::Model, "X100", "page:1"),
                fact(NamingFactKind::Version, "V2.1", "page:2"),
                fact(NamingFactKind::Subject, "Reset reliability", "page:1"),
            ],
            occupied_names: Vec::new(),
        }
    }

    #[test]
    fn accepts_a_bounded_evidence_grounded_request() {
        let request = valid_request();

        assert_eq!(request.validate(), Ok(()));
        assert_eq!(canonical_policy().policy_id, "canonical-v1");
        assert_eq!(
            canonical_policy().required_facts,
            vec![NamingFactKind::Subject]
        );
    }

    #[test]
    fn rejects_unknown_and_executable_shaped_fields() {
        let mut value = serde_json::to_value(valid_request()).expect("serialize request");
        value["command"] = serde_json::json!("rm -rf /");
        assert!(serde_json::from_value::<NamingRequest>(value).is_err());

        let mut value = serde_json::to_value(valid_request()).expect("serialize request");
        value["facts"][0]["template"] = serde_json::json!("{{ execute }}");
        assert!(serde_json::from_value::<NamingRequest>(value).is_err());
    }

    #[test]
    fn rejects_empty_oversized_or_control_character_text() {
        let mut request = valid_request();
        request.item_id.clear();
        assert!(request.validate().is_err());

        let mut request = valid_request();
        request.facts[0].value = "x".repeat(513);
        assert!(request.validate().is_err());

        let mut request = valid_request();
        request.facts[0].evidence_location = "page:\n1".to_owned();
        assert!(request.validate().is_err());
    }

    #[test]
    fn rejects_invalid_identity_duplicate_facts_and_excessive_collections() {
        let mut request = valid_request();
        request.identity.digest = "not-a-digest".to_owned();
        assert!(request.validate().is_err());

        let mut request = valid_request();
        request.facts.push(request.facts[0].clone());
        assert!(request.validate().is_err());

        let mut request = valid_request();
        request.facts = (0..=MAX_FACTS)
            .map(|index| fact(NamingFactKind::Project, &format!("P{index}"), "page:1"))
            .collect();
        assert!(request.validate().is_err());

        let mut request = valid_request();
        request.occupied_names = (0..=MAX_OCCUPIED_NAMES)
            .map(|index| format!("name-{index}.pdf"))
            .collect();
        assert!(request.validate().is_err());
    }

    #[test]
    fn rejects_path_like_original_names() {
        for original_name in [
            "../report.pdf",
            "folder/report.pdf",
            r"folder\report.pdf",
            ".",
            "..",
        ] {
            let mut request = valid_request();
            request.original_name = original_name.to_owned();
            assert!(request.validate().is_err(), "{original_name}");
        }
    }
}
