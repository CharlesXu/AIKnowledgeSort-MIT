use super::schema::{DeclarativeProfile, ProfileStatus, MAX_PROFILE_BYTES};

const BUNDLED_PROFILE_BYTES: &[u8] =
    include_bytes!("../../resources/profiles/ninebot-electronic-archive-0.3.0-draft.json");

pub(crate) fn bundled_profile() -> Result<DeclarativeProfile, String> {
    if BUNDLED_PROFILE_BYTES.is_empty() || BUNDLED_PROFILE_BYTES.len() > MAX_PROFILE_BYTES {
        return Err("Bundled Ninebot profile exceeds the profile size boundary".to_owned());
    }
    let profile: DeclarativeProfile = serde_json::from_slice(BUNDLED_PROFILE_BYTES)
        .map_err(|_| "Bundled Ninebot profile JSON is invalid".to_owned())?;
    profile.validate()?;
    if profile.schema_version != 2
        || profile.profile_id != "ninebot-electronic-archive"
        || profile.version != "0.3.0-draft"
        || profile.status != ProfileStatus::Draft
    {
        return Err("Bundled Ninebot profile identity is invalid".to_owned());
    }
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::bundled_profile;
    use crate::profiles::schema::{IndependentNodeTrigger, ProfileStatus, ReviewDisposition};
    use std::collections::BTreeMap;

    #[test]
    fn bundles_the_complete_inactive_four_level_discussion_taxonomy() {
        let profile = bundled_profile().expect("load bundled Ninebot draft");
        let mut level_counts = BTreeMap::new();
        for category in &profile.categories {
            *level_counts.entry(category.depth).or_insert(0_usize) += 1;
        }

        assert_eq!(profile.schema_version, 2);
        assert_eq!(profile.status, ProfileStatus::Draft);
        assert_eq!(profile.categories.len(), 466);
        assert_eq!(
            level_counts,
            BTreeMap::from([(1, 14), (2, 94), (3, 179), (4, 179)])
        );
        assert_eq!(
            profile
                .categories
                .iter()
                .map(|category| &category.category_id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            466
        );
        assert_eq!(
            profile
                .categories
                .iter()
                .map(|category| category.depth)
                .max(),
            Some(4)
        );
        assert!(profile.rules.is_empty());
    }

    #[test]
    fn preserves_the_canonical_sn02_label_and_manual_alias() {
        let category = bundled_profile()
            .expect("load bundled Ninebot draft")
            .categories
            .into_iter()
            .find(|category| category.category_id == "SN-02")
            .expect("SN-02 category");

        assert_eq!(category.label, "SN-02 IPMS 集成营销服");
        assert_eq!(category.aliases, ["SN-02 IPMS 管理营销闭环"]);
    }

    #[test]
    fn declares_archive_first_conflict_and_cross_domain_knowledge_policy() {
        let profile = bundled_profile().expect("load bundled Ninebot draft");
        let governance = profile.governance.expect("governance policy");

        assert_eq!(governance.maximum_depth, 4);
        assert!(governance.unique_primary_archive_category);
        assert!(governance.semantic_evidence_required);
        assert_eq!(
            governance.insufficient_evidence_disposition,
            ReviewDisposition::ImportantIndexed
        );
        assert_eq!(
            governance.conflicting_evidence_disposition,
            ReviewDisposition::ClassificationReview
        );
        assert!(governance.archive_first);
        assert!(governance.cross_domain_knowledge_links);
        assert_eq!(
            governance.independent_node_triggers,
            [
                IndependentNodeTrigger::HighValue,
                IndependentNodeTrigger::CrossDomain,
                IndependentNodeTrigger::UserRequested,
            ]
        );
        assert!(governance.generated_indexes_link_only);
    }
}
