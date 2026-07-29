use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const MAX_PROFILE_BYTES: usize = 1024 * 1024;
const MAX_RULES: usize = 1_000;
const MAX_CATEGORIES: usize = 2_000;
const MAX_DESTINATION_DEPTH: usize = 16;
const MAX_TAXONOMY_DEPTH: usize = 4;
const MAX_REQUIREMENTS_PER_RULE: usize = 32;
const MAX_PROVENANCE_EVIDENCE: usize = 64;
const MAX_ALIASES_PER_CATEGORY: usize = 16;
const MAX_METADATA_DIMENSIONS: usize = 32;
const MAX_NODE_TRIGGERS: usize = 16;
const MAX_ID_BYTES: usize = 128;
const MAX_TITLE_BYTES: usize = 256;
const MAX_TERM_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileStatus {
    Draft,
    Candidate,
    Approved,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileOwnership {
    Owned,
    FirstPartyAuthorized,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceKind {
    DocumentText,
    OcrText,
    Transcript,
    ReliableCompanion,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewDisposition {
    ImportantIndexed,
    ClassificationReview,
}

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IndependentNodeTrigger {
    HighValue,
    CrossDomain,
    UserRequested,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileProvenance {
    pub source_title: String,
    pub ownership: ProfileOwnership,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRequirement {
    pub kind: EvidenceKind,
    pub term: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClassificationRule {
    pub rule_id: String,
    pub destination: Vec<String>,
    pub all_of: Vec<EvidenceRequirement>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClassificationCategory {
    pub category_id: String,
    pub label: String,
    pub depth: u8,
    pub parent_id: Option<String>,
    pub path: Vec<String>,
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileGovernance {
    pub maximum_depth: u8,
    pub unique_primary_archive_category: bool,
    pub semantic_evidence_required: bool,
    pub metadata_only_dimensions: Vec<String>,
    pub insufficient_evidence_disposition: ReviewDisposition,
    pub conflicting_evidence_disposition: ReviewDisposition,
    pub archive_first: bool,
    pub cross_domain_knowledge_links: bool,
    pub independent_node_triggers: Vec<IndependentNodeTrigger>,
    pub generated_indexes_link_only: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclarativeProfile {
    pub schema_version: u32,
    pub profile_id: String,
    pub version: String,
    pub title: String,
    pub status: ProfileStatus,
    pub provenance: ProfileProvenance,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<ClassificationCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance: Option<ProfileGovernance>,
    pub rules: Vec<ClassificationRule>,
}

pub fn parse_candidate_profile(bytes: &[u8]) -> Result<DeclarativeProfile, String> {
    if bytes.is_empty() || bytes.len() > MAX_PROFILE_BYTES {
        return Err("Profile input is empty or exceeds 1 MiB".to_owned());
    }
    let profile: DeclarativeProfile =
        serde_json::from_slice(bytes).map_err(|_| "Profile JSON is invalid".to_owned())?;
    profile.validate()?;
    if profile.status != ProfileStatus::Candidate {
        return Err("Imported profile status must be candidate".to_owned());
    }
    Ok(profile)
}

impl DeclarativeProfile {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.schema_version, 1 | 2) {
            return Err("Profile schema version is unsupported".to_owned());
        }
        validate_id(&self.profile_id, "profile")?;
        validate_id(&self.version, "profile version")?;
        validate_text(&self.title, MAX_TITLE_BYTES, "profile title")?;
        validate_text(
            &self.provenance.source_title,
            MAX_TITLE_BYTES,
            "profile provenance title",
        )?;
        if self.provenance.evidence.is_empty()
            || self.provenance.evidence.len() > MAX_PROVENANCE_EVIDENCE
        {
            return Err("Profile provenance evidence count is invalid".to_owned());
        }
        for evidence in &self.provenance.evidence {
            validate_text(evidence, MAX_TERM_BYTES, "profile provenance evidence")?;
        }
        self.validate_taxonomy()?;
        if self.rules.len() > MAX_RULES {
            return Err("Profile rule count exceeds the limit".to_owned());
        }

        let taxonomy_paths: HashSet<&Vec<String>> = self
            .categories
            .iter()
            .map(|category| &category.path)
            .collect();
        let mut rule_ids = HashSet::with_capacity(self.rules.len());
        for rule in &self.rules {
            validate_id(&rule.rule_id, "rule")?;
            if !rule_ids.insert(rule.rule_id.as_str()) {
                return Err("Profile contains duplicate rule ids".to_owned());
            }
            if rule.destination.is_empty() || rule.destination.len() > MAX_DESTINATION_DEPTH {
                return Err("Profile rule destination depth is invalid".to_owned());
            }
            for segment in &rule.destination {
                validate_destination_segment(segment)?;
            }
            if self.schema_version == 2 && !taxonomy_paths.contains(&rule.destination) {
                return Err("Profile rule destination is absent from the taxonomy".to_owned());
            }
            if rule.all_of.is_empty() || rule.all_of.len() > MAX_REQUIREMENTS_PER_RULE {
                return Err("Profile rule evidence requirements are invalid".to_owned());
            }
            for requirement in &rule.all_of {
                validate_text(&requirement.term, MAX_TERM_BYTES, "profile evidence term")?;
            }
        }
        Ok(())
    }

    fn validate_taxonomy(&self) -> Result<(), String> {
        match self.schema_version {
            1 if !self.categories.is_empty() || self.governance.is_some() => {
                return Err("Profile schema version 1 cannot declare taxonomy".to_owned())
            }
            1 => return Ok(()),
            2 if self.categories.is_empty() || self.governance.is_none() => {
                return Err("Profile schema version 2 requires taxonomy governance".to_owned())
            }
            2 => {}
            _ => return Err("Profile schema version is unsupported".to_owned()),
        }

        if self.categories.len() > MAX_CATEGORIES {
            return Err("Profile category count exceeds the limit".to_owned());
        }
        let governance = self
            .governance
            .as_ref()
            .ok_or_else(|| "Profile schema version 2 requires taxonomy governance".to_owned())?;
        if governance.maximum_depth == 0
            || usize::from(governance.maximum_depth) > MAX_TAXONOMY_DEPTH
        {
            return Err("Profile governance maximum depth is invalid".to_owned());
        }
        validate_unique_ids(
            &governance.metadata_only_dimensions,
            MAX_METADATA_DIMENSIONS,
            "metadata dimension",
        )?;
        if governance.independent_node_triggers.is_empty()
            || governance.independent_node_triggers.len() > MAX_NODE_TRIGGERS
            || governance
                .independent_node_triggers
                .iter()
                .collect::<HashSet<_>>()
                .len()
                != governance.independent_node_triggers.len()
        {
            return Err("Profile independent node triggers are invalid".to_owned());
        }

        let mut categories = HashMap::with_capacity(self.categories.len());
        for category in &self.categories {
            validate_id(&category.category_id, "category")?;
            if categories
                .insert(category.category_id.as_str(), category)
                .is_some()
            {
                return Err("Profile contains duplicate category ids".to_owned());
            }
            validate_destination_segment(&category.label)?;
            let depth = usize::from(category.depth);
            if depth == 0
                || depth > usize::from(governance.maximum_depth)
                || depth > MAX_TAXONOMY_DEPTH
                || category.path.len() != depth
                || category.path.last() != Some(&category.label)
            {
                return Err("Profile category depth or path is invalid".to_owned());
            }
            for segment in &category.path {
                validate_destination_segment(segment)?;
            }
            if category.aliases.len() > MAX_ALIASES_PER_CATEGORY {
                return Err("Profile category alias count exceeds the limit".to_owned());
            }
            let mut aliases = HashSet::with_capacity(category.aliases.len());
            for alias in &category.aliases {
                validate_text(alias, MAX_TITLE_BYTES, "profile category alias")?;
                if alias == &category.label || !aliases.insert(alias.as_str()) {
                    return Err("Profile category aliases are invalid".to_owned());
                }
            }
        }

        for category in &self.categories {
            match (category.depth, category.parent_id.as_deref()) {
                (1, None) => {}
                (1, Some(_)) | (_, None) => {
                    return Err("Profile category parent is invalid".to_owned())
                }
                (_, Some(parent_id)) => {
                    let parent = categories
                        .get(parent_id)
                        .ok_or_else(|| "Profile category parent is missing".to_owned())?;
                    if parent.depth + 1 != category.depth
                        || parent.path.as_slice()
                            != &category.path[..category.path.len().saturating_sub(1)]
                    {
                        return Err("Profile category parent path is invalid".to_owned());
                    }
                }
            }
        }
        Ok(())
    }
}

fn validate_unique_ids(values: &[String], maximum: usize, label: &str) -> Result<(), String> {
    if values.is_empty() || values.len() > maximum {
        return Err(format!("Profile {label} count is invalid"));
    }
    let mut unique = HashSet::with_capacity(values.len());
    for value in values {
        validate_id(value, label)?;
        if !unique.insert(value.as_str()) {
            return Err(format!("Profile {label}s contain duplicates"));
        }
    }
    Ok(())
}

pub(crate) fn validate_id(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Err(format!("{label} id is invalid"))
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

fn validate_destination_segment(segment: &str) -> Result<(), String> {
    validate_text(segment, MAX_TITLE_BYTES, "profile destination segment")?;
    if matches!(segment, "." | "..")
        || segment.ends_with([' ', '.'])
        || segment.chars().any(|character| {
            matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
        })
    {
        return Err("Profile destination segment is not cross-platform safe".to_owned());
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .and_then(|value| value.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
        || stem
            .strip_prefix("LPT")
            .and_then(|value| value.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value));
    if reserved {
        return Err("Profile destination segment is reserved on Windows".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_candidate_profile;

    fn valid_profile() -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 1,
            "profileId": "fixture-profile",
            "version": "1.0.0",
            "title": "Fixture profile",
            "status": "candidate",
            "provenance": {
                "sourceTitle": "Owned fixture",
                "ownership": "owned",
                "evidence": ["authorization:test"]
            },
            "rules": [{
                "ruleId": "fixture.report",
                "destination": ["01-Research", "Reports"],
                "allOf": [{
                    "kind": "documentText",
                    "term": "quarterly report"
                }]
            }]
        })
    }

    fn bytes(value: &serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(value).expect("serialize profile fixture")
    }

    fn valid_v2_profile() -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 2,
            "profileId": "fixture-taxonomy",
            "version": "2.0.0",
            "title": "Fixture taxonomy",
            "status": "candidate",
            "provenance": {
                "sourceTitle": "Owned taxonomy fixture",
                "ownership": "owned",
                "evidence": ["sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
            },
            "categories": [
                {
                    "categoryId": "fixture.l1",
                    "label": "SN-01 Research",
                    "depth": 1,
                    "parentId": null,
                    "path": ["SN-01 Research"],
                    "aliases": []
                },
                {
                    "categoryId": "fixture.l2",
                    "label": "01 Reports",
                    "depth": 2,
                    "parentId": "fixture.l1",
                    "path": ["SN-01 Research", "01 Reports"],
                    "aliases": ["Research reports"]
                }
            ],
            "governance": {
                "maximumDepth": 4,
                "uniquePrimaryArchiveCategory": true,
                "semanticEvidenceRequired": true,
                "metadataOnlyDimensions": ["organization", "lifecycleState"],
                "insufficientEvidenceDisposition": "importantIndexed",
                "conflictingEvidenceDisposition": "classificationReview",
                "archiveFirst": true,
                "crossDomainKnowledgeLinks": true,
                "independentNodeTriggers": ["highValue", "crossDomain", "userRequested"],
                "generatedIndexesLinkOnly": true
            },
            "rules": [{
                "ruleId": "fixture.report",
                "destination": ["SN-01 Research", "01 Reports"],
                "allOf": [{
                    "kind": "documentText",
                    "term": "quarterly report"
                }]
            }]
        })
    }

    #[test]
    fn accepts_only_bounded_declarative_candidate_data() {
        let parsed = parse_candidate_profile(&bytes(&valid_profile()))
            .expect("parse strict candidate profile");

        assert_eq!(parsed.profile_id, "fixture-profile");
        assert_eq!(parsed.version, "1.0.0");
        assert_eq!(parsed.rules[0].rule_id, "fixture.report");
    }

    #[test]
    fn rejects_executable_unknown_and_approved_candidate_shapes() {
        let mut executable = valid_profile();
        executable["command"] = serde_json::json!("rm -rf");
        assert!(parse_candidate_profile(&bytes(&executable)).is_err());

        let mut nested_instruction = valid_profile();
        nested_instruction["rules"][0]["allOf"][0]["script"] = serde_json::json!("evaluate()");
        assert!(parse_candidate_profile(&bytes(&nested_instruction)).is_err());

        let mut approved = valid_profile();
        approved["status"] = serde_json::json!("approved");
        assert!(parse_candidate_profile(&bytes(&approved)).is_err());
    }

    #[test]
    fn rejects_invalid_ids_paths_evidence_and_duplicate_rules() {
        let mut empty_id = valid_profile();
        empty_id["profileId"] = serde_json::json!("");
        assert!(parse_candidate_profile(&bytes(&empty_id)).is_err());

        let mut unsafe_path = valid_profile();
        unsafe_path["rules"][0]["destination"] = serde_json::json!(["..", "Reports"]);
        assert!(parse_candidate_profile(&bytes(&unsafe_path)).is_err());

        let mut empty_evidence = valid_profile();
        empty_evidence["rules"][0]["allOf"] = serde_json::json!([]);
        assert!(parse_candidate_profile(&bytes(&empty_evidence)).is_err());

        let mut duplicate = valid_profile();
        let copied = duplicate["rules"][0].clone();
        duplicate["rules"]
            .as_array_mut()
            .expect("rules array")
            .push(copied);
        assert!(parse_candidate_profile(&bytes(&duplicate)).is_err());
    }

    #[test]
    fn rejects_payloads_above_the_documented_limit() {
        let oversized = vec![b' '; 1024 * 1024 + 1];

        assert!(parse_candidate_profile(&oversized).is_err());
    }

    #[test]
    fn accepts_schema_v2_taxonomy_and_preserves_schema_v1_compatibility() {
        let v2 = parse_candidate_profile(&bytes(&valid_v2_profile()))
            .expect("parse governed taxonomy profile");
        assert_eq!(v2.schema_version, 2);
        assert_eq!(v2.categories.len(), 2);
        assert_eq!(v2.categories[1].parent_id.as_deref(), Some("fixture.l1"));
        assert!(v2.governance.is_some());

        let v1 = parse_candidate_profile(&bytes(&valid_profile()))
            .expect("retain schema v1 candidate compatibility");
        assert_eq!(v1.schema_version, 1);
        assert!(v1.categories.is_empty());
        assert!(v1.governance.is_none());
    }

    #[test]
    fn rejects_invalid_taxonomy_identity_parent_depth_path_and_aliases() {
        let mut duplicate_id = valid_v2_profile();
        duplicate_id["categories"][1]["categoryId"] = serde_json::json!("fixture.l1");
        assert!(parse_candidate_profile(&bytes(&duplicate_id)).is_err());

        let mut missing_parent = valid_v2_profile();
        missing_parent["categories"][1]["parentId"] = serde_json::json!("fixture.missing");
        assert!(parse_candidate_profile(&bytes(&missing_parent)).is_err());

        let mut wrong_depth = valid_v2_profile();
        wrong_depth["categories"][1]["depth"] = serde_json::json!(5);
        assert!(parse_candidate_profile(&bytes(&wrong_depth)).is_err());

        let mut wrong_path = valid_v2_profile();
        wrong_path["categories"][1]["path"] = serde_json::json!(["SN-01 Research", "Different"]);
        assert!(parse_candidate_profile(&bytes(&wrong_path)).is_err());

        let mut unsafe_path = valid_v2_profile();
        unsafe_path["categories"][1]["path"] = serde_json::json!(["SN-01 Research", "../Reports"]);
        unsafe_path["categories"][1]["label"] = serde_json::json!("../Reports");
        assert!(parse_candidate_profile(&bytes(&unsafe_path)).is_err());

        let mut invalid_alias = valid_v2_profile();
        invalid_alias["categories"][1]["aliases"] = serde_json::json!([""]);
        assert!(parse_candidate_profile(&bytes(&invalid_alias)).is_err());
    }

    #[test]
    fn rejects_incomplete_or_executable_schema_v2_and_draft_ingress() {
        let mut missing_policy = valid_v2_profile();
        missing_policy
            .as_object_mut()
            .expect("profile object")
            .remove("governance");
        assert!(parse_candidate_profile(&bytes(&missing_policy)).is_err());

        let mut unknown_policy_field = valid_v2_profile();
        unknown_policy_field["governance"]["instruction"] = serde_json::json!("run this prompt");
        assert!(parse_candidate_profile(&bytes(&unknown_policy_field)).is_err());

        let mut draft = valid_v2_profile();
        draft["status"] = serde_json::json!("draft");
        assert!(parse_candidate_profile(&bytes(&draft)).is_err());
    }
}
