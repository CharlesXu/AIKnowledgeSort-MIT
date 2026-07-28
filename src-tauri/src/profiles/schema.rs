use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_PROFILE_BYTES: usize = 1024 * 1024;
const MAX_RULES: usize = 1_000;
const MAX_DESTINATION_DEPTH: usize = 16;
const MAX_REQUIREMENTS_PER_RULE: usize = 32;
const MAX_PROVENANCE_EVIDENCE: usize = 64;
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
pub struct DeclarativeProfile {
    pub schema_version: u32,
    pub profile_id: String,
    pub version: String,
    pub title: String,
    pub status: ProfileStatus,
    pub provenance: ProfileProvenance,
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
        if self.schema_version != 1 {
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
        if self.rules.len() > MAX_RULES {
            return Err("Profile rule count exceeds the limit".to_owned());
        }

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
            if rule.all_of.is_empty() || rule.all_of.len() > MAX_REQUIREMENTS_PER_RULE {
                return Err("Profile rule evidence requirements are invalid".to_owned());
            }
            for requirement in &rule.all_of {
                validate_text(&requirement.term, MAX_TERM_BYTES, "profile evidence term")?;
            }
        }
        Ok(())
    }
}

fn validate_id(value: &str, label: &str) -> Result<(), String> {
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
}
