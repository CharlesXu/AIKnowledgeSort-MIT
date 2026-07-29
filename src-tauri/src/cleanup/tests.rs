use super::{
    execute_with, persist_state, read_json, reconcile_vault, CleanupAudit, CleanupDisposition,
    CleanupExecutor, CleanupLifecycleState, CleanupPlanRegistry, CleanupStatus,
};
use crate::archive::{commit_plan_with_faults, ArchivePlan, ArchivePlanItem, TransactionFaults};
use crate::identity::ContentIdentity;
use crate::naming::schema::{NamingDecisionEvidence, NamingFact, NamingFactKind};
use crate::vault::VaultAuthorityRegistry;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

const BYTES: &[u8] = b"governed cleanup source\n";

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    vaults: VaultAuthorityRegistry,
    authority_id: String,
    operation_id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "aiks-cleanup-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).expect("create cleanup fixture");
        let root = root.canonicalize().expect("canonical fixture");
        let source = root.join("source.txt");
        fs::write(&source, BYTES).expect("write source");
        let vault_path = root.join("vault");
        fs::create_dir(&vault_path).expect("create Vault");
        let vaults = VaultAuthorityRegistry::default();
        let summary = vaults.authorize_path(&vault_path).expect("authorize Vault");
        let lease = vaults.lease(&summary.authority_id).expect("lease Vault");
        let identity = ContentIdentity::from_reader(Cursor::new(BYTES)).expect("hash fixture");
        let destination_path = format!("Originals/{}/Cleanup-source.txt", identity.digest);
        let result = commit_plan_with_faults(
            ArchivePlan {
                plan_id: "cleanup-archive-plan".to_owned(),
                plan_version: 2,
                proposal_id: "cleanup-proposal".to_owned(),
                naming_batch_id: "cleanup-naming".to_owned(),
                authority_id: summary.authority_id.clone(),
                vault_path: vault_path.to_string_lossy().into_owned(),
                expires_at_unix_ms: u64::MAX,
                confirmation_nonce: "archive-confirmation".to_owned(),
                source_preserved: true,
                items: vec![ArchivePlanItem {
                    item_id: "cleanup-item".to_owned(),
                    source_path: source.to_string_lossy().into_owned(),
                    destination_path,
                    original_name: "source.txt".to_owned(),
                    canonical_name: "Cleanup-source.txt".to_owned(),
                    naming: NamingDecisionEvidence {
                        naming_proposal_id: "cleanup-name".to_owned(),
                        original_name: "source.txt".to_owned(),
                        canonical_name: "Cleanup-source.txt".to_owned(),
                        policy_id: "canonical-v1".to_owned(),
                        policy_version: "1.0.0".to_owned(),
                        applied_rule: "ordered-cited-facts-v1".to_owned(),
                        facts: vec![NamingFact {
                            kind: NamingFactKind::Subject,
                            value: "Cleanup source".to_owned(),
                            evidence_location: "line:1".to_owned(),
                        }],
                    },
                    byte_size: BYTES.len() as u64,
                    identity,
                }],
            },
            &lease,
            TransactionFaults::default(),
        );
        drop(lease);
        Self {
            root,
            source,
            vaults,
            authority_id: summary.authority_id,
            operation_id: result.items[0].operation_id.clone(),
        }
    }

    fn lease(&self) -> crate::vault::VaultLease {
        self.vaults.lease(&self.authority_id).expect("lease Vault")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Default)]
struct TestExecutor {
    trash: Mutex<Vec<PathBuf>>,
    permanent: Mutex<Vec<PathBuf>>,
}

impl CleanupExecutor for TestExecutor {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        self.trash.lock().unwrap().push(path.to_owned());
        fs::remove_file(path).map_err(|error| error.to_string())
    }

    fn delete_permanently(&self, path: &Path) -> Result<(), String> {
        self.permanent.lock().unwrap().push(path.to_owned());
        fs::remove_file(path).map_err(|error| error.to_string())
    }
}

#[test]
fn cleanup_is_disabled_until_the_user_enables_it() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let result = plans.create_at(
        &fixture.lease(),
        std::slice::from_ref(&fixture.operation_id),
        false,
        Instant::now(),
        SystemTime::now(),
    );
    assert!(result.is_err());
    assert!(fixture.source.is_file());
}

#[test]
fn trash_plan_reverifies_and_preserves_the_registered_original() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create trash plan");
    assert_eq!(plan.disposition, CleanupDisposition::Trash);
    let consumed = plans
        .consume_at(&plan.plan_id, &plan.confirmation_nonce, Instant::now())
        .expect("consume plan");
    let executor = TestExecutor::default();
    let result = execute_with(&fixture.lease(), consumed, &executor);
    assert_eq!(result.status, CleanupStatus::Committed);
    assert!(!fixture.source.exists());
    assert!(Path::new(&plan.items[0].retained_path).is_file());
    assert_eq!(
        executor.trash.lock().unwrap().as_slice(),
        std::slice::from_ref(&fixture.source)
    );
    assert!(executor.permanent.lock().unwrap().is_empty());
    assert!(plans
        .consume_at(&plan.plan_id, &plan.confirmation_nonce, Instant::now())
        .is_err());
}

#[test]
fn changed_source_or_retained_copy_prevents_cleanup() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create plan");
    fs::write(&fixture.source, b"changed").expect("change source");
    let consumed = plans
        .consume_at(&plan.plan_id, &plan.confirmation_nonce, Instant::now())
        .expect("consume plan");
    let executor = TestExecutor::default();
    let result = execute_with(&fixture.lease(), consumed, &executor);
    assert_eq!(result.status, CleanupStatus::Failed);
    assert!(fixture.source.is_file());
    assert!(executor.trash.lock().unwrap().is_empty());

    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create plan");
    fs::write(&plan.items[0].retained_path, b"changed").expect("change retained original");
    let consumed = plans
        .consume_at(&plan.plan_id, &plan.confirmation_nonce, Instant::now())
        .expect("consume plan");
    let executor = TestExecutor::default();
    let result = execute_with(&fixture.lease(), consumed, &executor);
    assert_eq!(result.status, CleanupStatus::Failed);
    assert!(fixture.source.is_file());
    assert!(executor.trash.lock().unwrap().is_empty());
}

#[test]
fn permanent_delete_requires_a_second_bound_confirmation() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let trash_plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create trash plan");
    let permanent = plans
        .escalate_at(
            &fixture.lease(),
            &trash_plan.plan_id,
            &trash_plan.confirmation_nonce,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("request permanent plan");
    assert_eq!(permanent.disposition, CleanupDisposition::PermanentDelete);
    assert_ne!(permanent.plan_id, trash_plan.plan_id);
    assert_ne!(permanent.confirmation_nonce, trash_plan.confirmation_nonce);
    assert!(fixture.source.is_file());

    let consumed = plans
        .consume_at(
            &permanent.plan_id,
            &permanent.confirmation_nonce,
            Instant::now(),
        )
        .expect("consume permanent plan");
    let executor = TestExecutor::default();
    let result = execute_with(&fixture.lease(), consumed, &executor);
    assert_eq!(result.status, CleanupStatus::Committed);
    assert!(executor.trash.lock().unwrap().is_empty());
    assert_eq!(
        executor.permanent.lock().unwrap().as_slice(),
        std::slice::from_ref(&fixture.source)
    );
}

#[test]
fn wrong_confirmation_does_not_consume_the_reviewed_plan() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create plan");

    assert!(plans
        .consume_at(&plan.plan_id, "wrong-confirmation", Instant::now())
        .is_err());
    assert!(plans
        .consume_at(&plan.plan_id, &plan.confirmation_nonce, Instant::now())
        .is_ok());
}

#[test]
fn reconciliation_abandons_an_unconfirmed_plan_without_touching_the_source() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create plan");

    let reopened = VaultAuthorityRegistry::default();
    let summary = reopened
        .authorize_path(&fixture.root.join("vault"))
        .expect("reopen and reconcile Vault");
    let lease = reopened
        .lease(&summary.authority_id)
        .expect("lease reopened Vault");

    assert!(fixture.source.is_file());
    let record: CleanupAudit = read_json(
        &lease.directory,
        &Path::new(".aiks/cleanup")
            .join(&plan.plan_id)
            .join("00000001.json"),
    )
    .expect("read recovered record");
    assert_eq!(record.state, CleanupLifecycleState::Abandoned);
}

#[test]
fn reconciliation_abandons_an_executing_plan_when_no_source_was_mutated() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create plan");
    persist_state(
        &fixture.lease(),
        &plan,
        1,
        CleanupLifecycleState::Executing,
        "retained-original-verified",
        None,
    )
    .expect("persist executing");

    let report = reconcile_vault(&fixture.lease()).expect("reconcile executing plan");

    assert_eq!(report.abandoned, 1);
    assert!(fixture.source.is_file());
    let record: CleanupAudit = read_json(
        &fixture.lease().directory,
        &Path::new(".aiks/cleanup")
            .join(&plan.plan_id)
            .join("00000002.json"),
    )
    .expect("read recovered record");
    assert_eq!(record.state, CleanupLifecycleState::Abandoned);
}

#[test]
fn reconciliation_commits_an_executing_plan_only_after_the_source_is_absent() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create plan");
    persist_state(
        &fixture.lease(),
        &plan,
        1,
        CleanupLifecycleState::Executing,
        "retained-original-verified",
        None,
    )
    .expect("persist executing");
    fs::remove_file(&fixture.source).expect("simulate completed trash move");

    let report = reconcile_vault(&fixture.lease()).expect("reconcile committed plan");

    assert_eq!(report.recovered_committed, 1);
    assert!(Path::new(&plan.items[0].retained_path).is_file());
    let record: CleanupAudit = read_json(
        &fixture.lease().directory,
        &Path::new(".aiks/cleanup")
            .join(&plan.plan_id)
            .join("00000002.json"),
    )
    .expect("read recovered record");
    assert_eq!(record.state, CleanupLifecycleState::Committed);
}

#[test]
fn reconciliation_rejects_an_audit_record_moved_to_another_sequence() {
    let fixture = Fixture::new();
    let plans = CleanupPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            std::slice::from_ref(&fixture.operation_id),
            true,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create plan");
    let audit = fixture.root.join("vault/.aiks/cleanup").join(&plan.plan_id);
    fs::rename(audit.join("00000000.json"), audit.join("00000001.json")).expect("reorder audit");

    let error = reconcile_vault(&fixture.lease()).expect_err("reject invalid history");

    assert!(error.contains("binding is invalid"), "{error}");
    assert!(fixture.source.is_file());
}
