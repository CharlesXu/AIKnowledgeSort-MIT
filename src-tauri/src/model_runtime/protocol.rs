use crate::identity::ContentIdentity;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const KNOWLEDGE_RELATION_RULE_JSON: &str = r#"{"policyId":"knowledge-relations-v1","version":"1.0.0","task":"knowledgeRelations","requirements":{"relations":"Return only evidence-supported source, relationType, and target triples.","evidence":"Every relation must cite one or more supplied evidenceIds.","uncertainty":"Omit unsupported relations and explain uncertainty in summary."},"prohibitions":["filesystem mutation","archive mutation","knowledge mutation","graph mutation","invented evidence"]}"#;

const MAX_RELATIONS: usize = 64;
const MAX_EVIDENCE_IDS: usize = 16;
const MAX_SEMANTIC_TEXT_CHARS: usize = 512;
const MAX_REASON_CHARS: usize = 2_048;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ComparisonTask {
    KnowledgeRelations,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleSnapshot {
    pub policy_id: String,
    pub version: String,
    pub identity: ContentIdentity,
    pub json: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceExcerpt {
    pub evidence_id: String,
    pub start_line: u32,
    pub end_line: u32,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComparisonEnvelope {
    pub schema_version: u32,
    pub task: ComparisonTask,
    pub original_identity: ContentIdentity,
    pub markdown_identity: ContentIdentity,
    pub knowledge_revision: u32,
    pub rule_snapshot: RuleSnapshot,
    pub evidence: Vec<EvidenceExcerpt>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationSuggestion {
    pub source: String,
    pub relation_type: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelProposal {
    pub summary: String,
    pub relations: Vec<RelationSuggestion>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentDecision {
    Accept,
    Revise,
    Reject,
    Review,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalSide {
    Desktop,
    Agent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentAdjudication {
    pub decision: AgentDecision,
    pub reason: String,
    pub evidence_ids: Vec<String>,
    pub selected_side: Option<ProposalSide>,
    pub revised_relations: Vec<RelationSuggestion>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderOutcomeStatus {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderOutcome {
    pub status: ProviderOutcomeStatus,
    pub model: Option<String>,
    pub proposal: Option<ModelProposal>,
    pub failure_reason: Option<String>,
}

impl ProviderOutcome {
    pub fn failed(reason: impl Into<String>) -> Self {
        Self {
            status: ProviderOutcomeStatus::Failed,
            model: None,
            proposal: None,
            failure_reason: Some(reason.into()),
        }
    }

    pub fn succeeded(model: String, proposal: ModelProposal) -> Self {
        Self {
            status: ProviderOutcomeStatus::Succeeded,
            model: Some(model),
            proposal: Some(proposal),
            failure_reason: None,
        }
    }

    pub fn validate(&self, envelope: &ComparisonEnvelope) -> Result<(), String> {
        match self.status {
            ProviderOutcomeStatus::Succeeded => {
                if self.failure_reason.is_some() {
                    return Err("Successful provider outcome cannot include a failure".to_owned());
                }
                validate_text(
                    self.model.as_deref().unwrap_or(""),
                    MAX_SEMANTIC_TEXT_CHARS,
                    "Provider model",
                )?;
                self.proposal
                    .as_ref()
                    .ok_or_else(|| "Successful provider outcome requires a proposal".to_owned())?
                    .validate(envelope)
            }
            ProviderOutcomeStatus::Failed => {
                if self.proposal.is_some() || self.model.is_some() {
                    return Err("Failed provider outcome cannot include a proposal".to_owned());
                }
                validate_text(
                    self.failure_reason.as_deref().unwrap_or(""),
                    MAX_REASON_CHARS,
                    "Provider failure reason",
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ComparisonStatus {
    Completed,
    Review,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComparisonRecord {
    pub schema_version: u32,
    pub comparison_id: String,
    pub envelope: ComparisonEnvelope,
    pub envelope_identity: ContentIdentity,
    pub desktop_config_id: String,
    pub agent_config_id: String,
    pub desktop_outcome: ProviderOutcome,
    pub agent_outcome: ProviderOutcome,
    pub adjudication: Option<AgentAdjudication>,
    pub adjudication_failure: Option<String>,
    pub status: ComparisonStatus,
    pub actor: String,
    pub recorded_at_unix_ms: u64,
}

impl ModelProposal {
    pub fn validate(&self, envelope: &ComparisonEnvelope) -> Result<(), String> {
        validate_text(&self.summary, MAX_REASON_CHARS, "Proposal summary")?;
        if self.relations.len() > MAX_RELATIONS {
            return Err("Proposal exceeds 64 relations".to_owned());
        }
        for relation in &self.relations {
            relation.validate(envelope)?;
        }
        Ok(())
    }
}

impl RelationSuggestion {
    fn validate(&self, envelope: &ComparisonEnvelope) -> Result<(), String> {
        validate_text(&self.source, MAX_SEMANTIC_TEXT_CHARS, "Relation source")?;
        validate_text(
            &self.relation_type,
            MAX_SEMANTIC_TEXT_CHARS,
            "Relation type",
        )?;
        validate_text(&self.target, MAX_SEMANTIC_TEXT_CHARS, "Relation target")?;
        validate_evidence_ids(&self.evidence_ids, envelope)
    }
}

impl AgentAdjudication {
    pub fn validate(&self, envelope: &ComparisonEnvelope) -> Result<(), String> {
        validate_text(&self.reason, MAX_REASON_CHARS, "Adjudication reason")?;
        validate_evidence_ids(&self.evidence_ids, envelope)?;
        match self.decision {
            AgentDecision::Accept if self.selected_side.is_none() => {
                return Err("Accept adjudication must select a proposal side".to_owned())
            }
            AgentDecision::Revise if self.revised_relations.is_empty() => {
                return Err("Revise adjudication must include revised relations".to_owned())
            }
            AgentDecision::Revise => {}
            _ if !self.revised_relations.is_empty() => {
                return Err("Only revise adjudication may include revised relations".to_owned())
            }
            _ => {}
        }
        if self.revised_relations.len() > MAX_RELATIONS {
            return Err("Adjudication exceeds 64 revised relations".to_owned());
        }
        for relation in &self.revised_relations {
            relation.validate(envelope)?;
        }
        Ok(())
    }
}

fn validate_evidence_ids(
    evidence_ids: &[String],
    envelope: &ComparisonEnvelope,
) -> Result<(), String> {
    if evidence_ids.is_empty() || evidence_ids.len() > MAX_EVIDENCE_IDS {
        return Err("Semantic output must cite 1 to 16 evidence IDs".to_owned());
    }
    let available = envelope
        .evidence
        .iter()
        .map(|evidence| evidence.evidence_id.as_str())
        .collect::<HashSet<_>>();
    let mut cited = HashSet::with_capacity(evidence_ids.len());
    for evidence_id in evidence_ids {
        if !available.contains(evidence_id.as_str()) || !cited.insert(evidence_id.as_str()) {
            return Err("Semantic output contains an invalid evidence ID".to_owned());
        }
    }
    Ok(())
}

fn validate_text(value: &str, max_chars: usize, field: &str) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value.chars().count() > max_chars
        || value.chars().any(char::is_control)
    {
        return Err(format!("{field} is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AgentAdjudication, AgentDecision, ComparisonEnvelope, ComparisonTask, EvidenceExcerpt,
        ModelProposal, ProposalSide, ProviderOutcome, RelationSuggestion, RuleSnapshot,
        KNOWLEDGE_RELATION_RULE_JSON,
    };
    use crate::identity::ContentIdentity;
    use std::io::Cursor;

    fn identity(bytes: &[u8]) -> ContentIdentity {
        ContentIdentity::from_reader(Cursor::new(bytes)).expect("hash protocol fixture")
    }

    fn envelope() -> ComparisonEnvelope {
        ComparisonEnvelope {
            schema_version: 1,
            task: ComparisonTask::KnowledgeRelations,
            original_identity: identity(b"original"),
            markdown_identity: identity(b"markdown"),
            knowledge_revision: 1,
            rule_snapshot: RuleSnapshot {
                policy_id: "knowledge-relations-v1".to_owned(),
                version: "1.0.0".to_owned(),
                identity: identity(KNOWLEDGE_RELATION_RULE_JSON.as_bytes()),
                json: KNOWLEDGE_RELATION_RULE_JSON.to_owned(),
            },
            evidence: vec![EvidenceExcerpt {
                evidence_id: "line-2-3".to_owned(),
                start_line: 2,
                end_line: 3,
                text: "Evidence.\n".to_owned(),
            }],
        }
    }

    fn relation(evidence_id: &str) -> RelationSuggestion {
        RelationSuggestion {
            source: "MCU".to_owned(),
            relation_type: "dependsOn".to_owned(),
            target: "Reset controller".to_owned(),
            evidence_ids: vec![evidence_id.to_owned()],
        }
    }

    #[test]
    fn validates_only_evidence_bound_proposals() {
        let envelope = envelope();
        let proposal = ModelProposal {
            summary: "One supported relation".to_owned(),
            relations: vec![relation("line-2-3")],
        };
        assert!(proposal.validate(&envelope).is_ok());
        assert!(
            ProviderOutcome::succeeded("model-v1".to_owned(), proposal.clone())
                .validate(&envelope)
                .is_ok()
        );
        let invalid = ModelProposal {
            summary: proposal.summary,
            relations: vec![relation("invented")],
        };
        assert!(invalid.validate(&envelope).is_err());
    }

    #[test]
    fn enforces_adjudication_decision_shape() {
        let envelope = envelope();
        let accepted = AgentAdjudication {
            decision: AgentDecision::Accept,
            reason: "Desktop proposal is fully supported".to_owned(),
            evidence_ids: vec!["line-2-3".to_owned()],
            selected_side: Some(ProposalSide::Desktop),
            revised_relations: vec![],
        };
        assert!(accepted.validate(&envelope).is_ok());

        let missing_side = AgentAdjudication {
            selected_side: None,
            ..accepted.clone()
        };
        assert!(missing_side.validate(&envelope).is_err());
        let invalid_revision = AgentAdjudication {
            decision: AgentDecision::Revise,
            selected_side: None,
            revised_relations: vec![relation("invented")],
            ..accepted
        };
        assert!(invalid_revision.validate(&envelope).is_err());
    }
}
