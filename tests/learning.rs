use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore, EmployeeStatus},
    learning::*,
};
use uuid::Uuid;
fn setup() -> (DemoStore, Uuid, Uuid) {
    let mut s = DemoStore::new();
    let m = s
        .data
        .accounts
        .iter()
        .find(|a| a.role == Role::Manager)
        .unwrap()
        .clone();
    let mut e = s.data.employees[0].clone();
    e.id = Uuid::new_v4();
    e.employee_code = "L-STAFF".into();
    e.full_name = "Synthetic learning employee".into();
    e.reports_to = m.employee_id;
    e.status = EmployeeStatus::Active;
    let id = e.id;
    s.data.employees.push(e);
    (s, id, m.employee_id.unwrap())
}
fn assigned(s: &mut DemoStore, id: Uuid) -> LearningAction {
    let mut d = TrainingDraft::new(Some(id));
    d.title = "Synthetic training".into();
    d.reason = "Synthetic learning assignment".into();
    let a = LearningAction::TrainingSave(d);
    s.execute(Action::Learning(a.clone())).unwrap();
    a
}
fn manager(s: &mut DemoStore, id: Uuid) {
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = Some(id);
    s.data.access.user_id = s
        .data
        .accounts
        .iter()
        .find(|a| a.employee_id == Some(id))
        .unwrap()
        .user_id;
}
fn cert(s: &mut DemoStore, id: Uuid) -> LearningAction {
    let mut d = CertificateDraft::new(Some(id), s.data.company.today());
    d.title = "Synthetic certificate".into();
    d.reason = "Synthetic certification record".into();
    let a = LearningAction::CertificateSave(d);
    s.execute(Action::Learning(a.clone())).unwrap();
    a
}
#[test]
fn training_retries_preserve_later_progress_and_are_actor_bound() {
    let (mut s, id, m) = setup();
    let a = assigned(&mut s, id);
    let tid = s.data.learning.training[0].id;
    let original = s.data.access.clone();
    manager(&mut s, m);
    s.execute(Action::Learning(LearningAction::Progress {
        id: tid,
        revision: 0,
        note: "Synthetic completed lesson".into(),
        reason: "Synthetic progress".into(),
    }))
    .unwrap();
    s.data.access = original.clone();
    let events = s.data.learning.events.len();
    s.execute(Action::Learning(a.clone())).unwrap();
    assert_eq!(s.data.learning.events.len(), events);
    assert_eq!(
        s.data.learning.training[0].progress_note,
        "Synthetic completed lesson"
    );
    s.data.access.user_id = Uuid::new_v4();
    assert!(s.execute(Action::Learning(a)).is_err());
    assert_eq!(s.data.learning.events.len(), events);
}
#[test]
fn current_manager_progress_is_separate_from_hr_completion() {
    let (mut s, id, m) = setup();
    assigned(&mut s, id);
    let tid = s.data.learning.training[0].id;
    manager(&mut s, m);
    let mut d = TrainingDraft::new(Some(id));
    d.title = "Denied definition".into();
    d.reason = "Manager denied".into();
    assert!(
        s.execute(Action::Learning(LearningAction::TrainingSave(d)))
            .is_err()
    );
    let a = LearningAction::TrainingStatus {
        id: tid,
        revision: 0,
        status: "completed".into(),
        completed_on: s.data.company.today().to_string(),
        learning_minutes: Some(90),
        outcome: "Synthetic evidence".into(),
        reason: "Manager cannot verify".into(),
    };
    assert!(s.execute(Action::Learning(a)).is_err());
    s.execute(Action::Learning(LearningAction::Progress {
        id: tid,
        revision: 0,
        note: "Synthetic progress".into(),
        reason: "Current Manager".into(),
    }))
    .unwrap();
    s.data
        .employees
        .iter_mut()
        .find(|e| e.id == id)
        .unwrap()
        .reports_to = None;
    assert!(
        s.execute(Action::Learning(LearningAction::Progress {
            id: tid,
            revision: 1,
            note: "Former Manager".into(),
            reason: "Denied current scope".into()
        }))
        .is_err()
    );
    assert!(training(&s.data, tid).is_err());
    assert!(visible_events(&s.data, Some(tid), None).is_empty());
}
#[test]
fn completion_reopening_and_receipts_preserve_history() {
    let (mut s, id, _) = setup();
    assigned(&mut s, id);
    let tid = s.data.learning.training[0].id;
    let a = LearningAction::TrainingStatus {
        id: tid,
        revision: 0,
        status: "completed".into(),
        completed_on: s.data.company.today().to_string(),
        learning_minutes: Some(90),
        outcome: "Synthetic demonstrated checklist".into(),
        reason: "HR recorded completion".into(),
    };
    let before = s.data.clone();
    assert!(!a.saved(&before));
    s.execute(Action::Learning(a.clone())).unwrap();
    assert!(a.saved(&s.data));
    let mut wrong = s.data.clone();
    wrong.learning.training[0].outcome = "Wrong response".into();
    assert!(!a.saved(&wrong));
    s.execute(Action::Learning(LearningAction::TrainingStatus {
        id: tid,
        revision: 1,
        status: "in_progress".into(),
        completed_on: String::new(),
        learning_minutes: None,
        outcome: String::new(),
        reason: "Synthetic completion correction".into(),
    }))
    .unwrap();
    assert!(s.data.learning.training[0].completed_on.is_none());
    assert!(
        s.data
            .learning
            .events
            .iter()
            .any(|e| e.kind == "training_completed" && e.after_record["learning_minutes"] == 90)
    );
    assert!(s.execute(Action::Learning(a)).is_err());
}
#[test]
fn blank_or_future_completion_and_bad_dates_cannot_change_data() {
    let (mut s, id, _) = setup();
    assigned(&mut s, id);
    let tid = s.data.learning.training[0].id;
    for (date, outcome) in [
        ("2200-01-01", "Future"),
        ("", "Missing date"),
        ("2020-01-01", ""),
    ] {
        let n = s.data.learning.events.len();
        assert!(
            s.execute(Action::Learning(LearningAction::TrainingStatus {
                id: tid,
                revision: 0,
                status: "completed".into(),
                completed_on: date.into(),
                learning_minutes: None,
                outcome: outcome.into(),
                reason: "Synthetic invalid completion".into()
            }))
            .is_err()
        );
        assert_eq!(s.data.learning.events.len(), n);
        assert_eq!(s.data.learning.training[0].revision, 0);
    }
    let mut d = TrainingDraft::new(Some(id));
    d.title = "Invalid dates".into();
    d.starts_on = "2026-02-01".into();
    d.ends_on = "2026-01-01".into();
    d.reason = "Synthetic date denial".into();
    assert!(
        s.execute(Action::Learning(LearningAction::TrainingSave(d)))
            .is_err()
    );
}
#[test]
fn expiry_and_archive_preserve_revocation_state() {
    let (mut s, id, _) = setup();
    cert(&mut s, id);
    let cid = s.data.learning.certificates[0].id;
    let today = s.data.company.today();
    assert_eq!(
        s.data.learning.certificates[0].label(today),
        "Valid (recorded)"
    );
    s.data.learning.certificates[0].expires_on = Some(today);
    assert_eq!(
        s.data.learning.certificates[0].label(today),
        "Expires within 30 days"
    );
    assert_eq!(
        s.data.learning.certificates[0].label(today.succ_opt().unwrap()),
        "Expired"
    );
    s.execute(Action::Learning(LearningAction::CertificateStatus {
        id: cid,
        revision: 0,
        status: "revoked".into(),
        reason: "Synthetic revocation".into(),
    }))
    .unwrap();
    s.execute(Action::Learning(LearningAction::CertificateArchive {
        id: cid,
        revision: 1,
        archived: true,
        reason: "Synthetic archive".into(),
    }))
    .unwrap();
    assert_eq!(s.data.learning.certificates[0].label(today), "Archived");
    s.execute(Action::Learning(LearningAction::CertificateArchive {
        id: cid,
        revision: 2,
        archived: false,
        reason: "Synthetic restore".into(),
    }))
    .unwrap();
    assert_eq!(s.data.learning.certificates[0].label(today), "Revoked");
}
fn document(s: &mut DemoStore, person: Uuid) -> Uuid {
    use hris_desktop::documents::*;
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    s.data.documents.push(EmployeeDocument {
        id,
        company_id: s.data.company.id,
        employee_id: person,
        details: DocumentDetails {
            title: "Synthetic certificate file".into(),
            category: DocumentCategory::Certificate,
            notes: String::new(),
            issued_on: None,
            expires_on: None,
            contract_start: None,
            contract_end: None,
        },
        original_filename: "synthetic.pdf".into(),
        mime_type: "application/pdf".into(),
        byte_size: 100,
        sha256: "a".repeat(64),
        object_name: format!("{}/{person}/{id}/file.pdf", s.data.company.id),
        state: DocumentState::Ready,
        archived: false,
        revision: 0,
        created_by: s.data.access.user_id,
        created_by_name: "Synthetic HR".into(),
        created_at: now,
        updated_at: now,
    });
    id
}
#[test]
fn evidence_is_canonical_private_and_unlinks_without_removing_files() {
    let (mut s, id, m) = setup();
    cert(&mut s, id);
    let cid = s.data.learning.certificates[0].id;
    let doc = document(&mut s, id);
    let mut d = EvidenceDraft::new(cid);
    d.document_id = Some(doc);
    d.reason = "Synthetic evidence".into();
    let a = LearningAction::EvidenceSave(d);
    s.execute(Action::Learning(a.clone())).unwrap();
    let admin = s.data.access.clone();
    manager(&mut s, m);
    assert!(s.execute(Action::Learning(a.clone())).is_err());
    assert!(
        !visible_events(&s.data, None, Some(cid))
            .iter()
            .any(|e| e.evidence_id.is_some())
    );
    let summary = serde_json::to_string(&s.data.learning.certificates[0]).unwrap();
    assert!(!summary.contains("document_id") && !summary.contains(&doc.to_string()));
    s.data.access = admin;
    let mut d = EvidenceDraft::existing(&s.data.learning.evidence[0]);
    d.document_id = None;
    d.reason = "Synthetic unlink".into();
    s.execute(Action::Learning(LearningAction::EvidenceSave(d)))
        .unwrap();
    assert_eq!(s.data.learning.evidence[0].document_id, None);
    s.execute(Action::Learning(a)).unwrap();
    assert_eq!(s.data.learning.evidence[0].document_id, None);
    assert!(s.data.documents.iter().any(|d| d.id == doc));
}
#[test]
fn evidence_rejects_another_employee_and_incomplete_or_archived_file() {
    let (mut s, id, m) = setup();
    cert(&mut s, id);
    let cid = s.data.learning.certificates[0].id;
    let doc = document(&mut s, m);
    let mut d = EvidenceDraft::new(cid);
    d.document_id = Some(doc);
    d.reason = "Synthetic wrong employee".into();
    assert!(
        s.execute(Action::Learning(LearningAction::EvidenceSave(d)))
            .is_err()
    );
    let doc = document(&mut s, id);
    s.data
        .documents
        .iter_mut()
        .find(|d| d.id == doc)
        .unwrap()
        .archived = true;
    let mut d = EvidenceDraft::new(cid);
    d.document_id = Some(doc);
    d.reason = "Synthetic archived file".into();
    assert!(
        s.execute(Action::Learning(LearningAction::EvidenceSave(d)))
            .is_err()
    );
    assert!(s.data.learning.evidence.is_empty());
}
#[test]
fn employee_desktop_and_foreign_company_actions_are_denied() {
    let (mut s, id, _) = setup();
    let a = assigned(&mut s, id);
    let mut foreign = s.data.clone();
    foreign.company.id = Uuid::new_v4();
    assert!(a.payload(&foreign).is_err());
    s.data.access.role = Role::Employee;
    s.data.access.employee_id = Some(id);
    assert!(a.payload(&s.data).is_err());
    assert!(
        LearningAction::Progress {
            id: s.data.learning.training[0].id,
            revision: 0,
            note: "Employee desktop".into(),
            reason: "Synthetic denial".into()
        }
        .payload(&s.data)
        .is_err()
    );
}
#[test]
fn archived_employee_retains_records_and_hr_can_resolve_training() {
    let (mut s, id, _) = setup();
    assigned(&mut s, id);
    cert(&mut s, id);
    s.data
        .employees
        .iter_mut()
        .find(|e| e.id == id)
        .unwrap()
        .status = EmployeeStatus::Archived;
    let tid = s.data.learning.training[0].id;
    let mut d = TrainingDraft::new(Some(id));
    d.title = "Archived new training denied".into();
    d.reason = "Synthetic denial".into();
    assert!(
        s.execute(Action::Learning(LearningAction::TrainingSave(d)))
            .is_err()
    );
    s.execute(Action::Learning(LearningAction::TrainingStatus {
        id: tid,
        revision: 0,
        status: "cancelled".into(),
        completed_on: String::new(),
        learning_minutes: None,
        outcome: String::new(),
        reason: "Synthetic historical resolution".into(),
    }))
    .unwrap();
    assert_eq!(s.data.learning.certificates.len(), 1);
    assert_eq!(s.data.learning.training[0].status, "cancelled");
}
