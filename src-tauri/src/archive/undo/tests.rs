use super::{
    execute_with, persist_state, prepare_staging, quarantine_archive, reconcile_vault,
    ArchiveUndoExecutor, ArchiveUndoPlanRegistry, ArchiveUndoStatus, UndoLifecycleState,
};
use crate::archive::{
    commit_plan_with_faults, verified_registered_original, ArchivePlan, ArchivePlanItem,
    TransactionFaults,
};
use crate::identity::ContentIdentity;
use crate::naming::schema::{NamingDecisionEvidence, NamingFact, NamingFactKind};
use crate::vault::VaultAuthorityRegistry;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

const BYTES: &[u8] = b"bounded archive undo source\n";

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
            "aiks-archive-undo-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).expect("create undo fixture");
        let root = root.canonicalize().expect("canonical fixture");
        let source = root.join("source.txt");
        fs::write(&source, BYTES).expect("write source");
        let vault_path = root.join("vault");
        fs::create_dir(&vault_path).expect("create Vault");
        let vaults = VaultAuthorityRegistry::default();
        let summary = vaults.authorize_path(&vault_path).expect("authorize Vault");
        let lease = vaults.lease(&summary.authority_id).expect("lease Vault");
        let identity = identity();
        let result = commit_plan_with_faults(
            ArchivePlan {
                plan_id: "undo-archive-plan".to_owned(),
                plan_version: 2,
                proposal_id: "undo-proposal".to_owned(),
                naming_batch_id: "undo-naming".to_owned(),
                authority_id: summary.authority_id.clone(),
                vault_path: vault_path.to_string_lossy().into_owned(),
                expires_at_unix_ms: u64::MAX,
                confirmation_nonce: "archive-confirmation".to_owned(),
                source_preserved: true,
                items: vec![ArchivePlanItem {
                    item_id: "undo-item".to_owned(),
                    source_path: source.to_string_lossy().into_owned(),
                    destination_path: format!(
                        "Originals/{}/Bounded-archive-undo.txt",
                        identity.digest
                    ),
                    original_name: "source.txt".to_owned(),
                    canonical_name: "Bounded-archive-undo.txt".to_owned(),
                    naming: NamingDecisionEvidence {
                        naming_proposal_id: "undo-name".to_owned(),
                        original_name: "source.txt".to_owned(),
                        canonical_name: "Bounded-archive-undo.txt".to_owned(),
                        policy_id: "canonical-v1".to_owned(),
                        policy_version: "1.0.0".to_owned(),
                        applied_rule: "ordered-cited-facts-v1".to_owned(),
                        facts: vec![NamingFact {
                            kind: NamingFactKind::Subject,
                            value: "Bounded archive undo".to_owned(),
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

fn identity() -> ContentIdentity {
    ContentIdentity::from_reader(Cursor::new(BYTES)).expect("hash source")
}

#[derive(Default)]
struct RemovingExecutor {
    removed: Mutex<Vec<PathBuf>>,
    remove_source_after_trash: Option<PathBuf>,
}

impl ArchiveUndoExecutor for RemovingExecutor {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        fs::remove_file(path).map_err(|error| error.to_string())?;
        self.removed.lock().unwrap().push(path.to_owned());
        if let Some(source) = &self.remove_source_after_trash {
            fs::remove_file(source).map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

struct RejectingExecutor;

impl ArchiveUndoExecutor for RejectingExecutor {
    fn move_to_trash(&self, _path: &Path) -> Result<(), String> {
        Err("trash unavailable".to_owned())
    }
}

#[test]
fn undo_plan_requires_a_current_second_original_and_one_fresh_confirmation() {
    let fixture = Fixture::new();
    let plans = ArchiveUndoPlanRegistry::default();
    let now = Instant::now();
    let plan = plans
        .create_at(
            &fixture.lease(),
            &fixture.operation_id,
            now,
            SystemTime::now(),
        )
        .expect("create undo plan");

    assert!(plans
        .consume_at(&plan.undo_id, "wrong-confirmation", now)
        .is_err());
    assert!(plans
        .consume_at(
            &plan.undo_id,
            &plan.confirmation_nonce,
            now + Duration::from_secs(301),
        )
        .is_err());
    assert!(fixture.source.is_file());

    let fixture = Fixture::new();
    fs::write(&fixture.source, b"changed").expect("change source");
    assert!(ArchiveUndoPlanRegistry::default()
        .create_at(
            &fixture.lease(),
            &fixture.operation_id,
            Instant::now(),
            SystemTime::now(),
        )
        .is_err());
}

#[test]
fn confirmed_undo_trashes_only_the_archive_and_preserves_the_source() {
    let fixture = Fixture::new();
    let plans = ArchiveUndoPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            &fixture.operation_id,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create undo plan");
    let consumed = plans
        .consume_at(&plan.undo_id, &plan.confirmation_nonce, Instant::now())
        .expect("consume undo plan");
    let executor = RemovingExecutor::default();

    let result = execute_with(&fixture.lease(), consumed, &executor);

    assert_eq!(result.status, ArchiveUndoStatus::Committed);
    assert!(fixture.source.is_file());
    assert!(!Path::new(&plan.archived_path).exists());
    assert!(verified_registered_original(&fixture.lease(), &fixture.operation_id).is_err());
    let removed = executor.removed.lock().unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(
        removed[0].file_name().and_then(|value| value.to_str()),
        Some("Bounded-archive-undo.txt")
    );
}

#[test]
fn source_loss_during_undo_restores_the_archived_original_without_partial_commit() {
    let fixture = Fixture::new();
    let plans = ArchiveUndoPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            &fixture.operation_id,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create undo plan");
    let consumed = plans
        .consume_at(&plan.undo_id, &plan.confirmation_nonce, Instant::now())
        .expect("consume undo plan");
    let executor = RemovingExecutor {
        removed: Mutex::default(),
        remove_source_after_trash: Some(fixture.source.clone()),
    };

    let result = execute_with(&fixture.lease(), consumed, &executor);

    assert_eq!(result.status, ArchiveUndoStatus::Failed);
    assert!(Path::new(&plan.archived_path).is_file());
    assert_eq!(
        verified_registered_original(&fixture.lease(), &fixture.operation_id)
            .expect("archive restored")
            .identity,
        identity()
    );
}

#[test]
fn trash_rejection_restores_the_archive_and_keeps_registration_active() {
    let fixture = Fixture::new();
    let plans = ArchiveUndoPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            &fixture.operation_id,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create undo plan");
    let consumed = plans
        .consume_at(&plan.undo_id, &plan.confirmation_nonce, Instant::now())
        .expect("consume undo plan");

    let result = execute_with(&fixture.lease(), consumed, &RejectingExecutor);

    assert_eq!(result.status, ArchiveUndoStatus::Failed);
    assert_eq!(result.failure_reason.as_deref(), Some("trash unavailable"));
    assert!(fixture.source.is_file());
    assert!(Path::new(&plan.archived_path).is_file());
    assert_eq!(
        verified_registered_original(&fixture.lease(), &fixture.operation_id)
            .expect("archive remains active")
            .identity,
        identity()
    );
    assert!(!fixture
        .root
        .join("vault/.aiks/undo-trash")
        .join(&plan.undo_id)
        .exists());
}

#[test]
fn reconciliation_finishes_an_interrupted_undo_only_when_the_source_survives() {
    let fixture = Fixture::new();
    let plans = ArchiveUndoPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            &fixture.operation_id,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create undo plan");
    let archived = PathBuf::from(&plan.archived_path);
    prepare_staging(&fixture.lease(), &plan).expect("prepare undo staging");
    persist_state(
        &fixture.lease(),
        &plan,
        1,
        UndoLifecycleState::Executing,
        "source-and-archive-verified",
        "moving-archive-to-trash",
        None,
    )
    .expect("persist executing");
    fs::remove_file(&archived).expect("simulate archive moved to trash");

    reconcile_vault(&fixture.lease()).expect("reconcile undo");

    assert!(fixture.source.is_file());
    assert!(!archived.exists());
    assert!(verified_registered_original(&fixture.lease(), &fixture.operation_id).is_err());
}

#[test]
fn reconciliation_restores_the_archive_when_the_second_original_disappeared() {
    let fixture = Fixture::new();
    let plans = ArchiveUndoPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            &fixture.operation_id,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create undo plan");
    prepare_staging(&fixture.lease(), &plan).expect("prepare undo staging");
    persist_state(
        &fixture.lease(),
        &plan,
        1,
        UndoLifecycleState::Executing,
        "source-and-archive-verified",
        "moving-archive-to-trash",
        None,
    )
    .expect("persist executing");
    fs::remove_file(&plan.archived_path).expect("simulate archive moved to trash");
    fs::remove_file(&fixture.source).expect("simulate source loss");

    reconcile_vault(&fixture.lease()).expect("reconcile unsafe undo");

    assert!(Path::new(&plan.archived_path).is_file());
    assert_eq!(
        verified_registered_original(&fixture.lease(), &fixture.operation_id)
            .expect("archive restored")
            .identity,
        identity()
    );
}

#[test]
fn reconciliation_rolls_back_capability_quarantine_before_trash() {
    let fixture = Fixture::new();
    let plans = ArchiveUndoPlanRegistry::default();
    let plan = plans
        .create_at(
            &fixture.lease(),
            &fixture.operation_id,
            Instant::now(),
            SystemTime::now(),
        )
        .expect("create undo plan");
    prepare_staging(&fixture.lease(), &plan).expect("prepare undo staging");
    persist_state(
        &fixture.lease(),
        &plan,
        1,
        UndoLifecycleState::Executing,
        "source-and-archive-verified",
        "moving-archive-to-trash",
        None,
    )
    .expect("persist executing");
    quarantine_archive(&fixture.lease(), &plan).expect("quarantine archive");
    assert!(!Path::new(&plan.archived_path).exists());

    reconcile_vault(&fixture.lease()).expect("reconcile quarantine");

    assert!(Path::new(&plan.archived_path).is_file());
    assert_eq!(
        verified_registered_original(&fixture.lease(), &fixture.operation_id)
            .expect("archive remains active")
            .identity,
        identity()
    );
}

#[test]
fn authoritative_knowledge_makes_archive_undo_ineligible() {
    let fixture = Fixture::new();
    fs::create_dir(
        fixture
            .root
            .join("vault/.aiks/knowledge")
            .join(&fixture.operation_id),
    )
    .expect("create knowledge dependency");

    let result = ArchiveUndoPlanRegistry::default().create_at(
        &fixture.lease(),
        &fixture.operation_id,
        Instant::now(),
        SystemTime::now(),
    );

    assert!(result.is_err());
    let original = verified_registered_original(&fixture.lease(), &fixture.operation_id)
        .expect("archive remains active");
    assert!(fixture
        .root
        .join("vault")
        .join(original.relative_path)
        .is_file());
}
