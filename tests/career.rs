use hris_desktop::{
    access::Role,
    career::*,
    domain::{Action, DemoStore, EmployeeStatus},
};
use serde_json::json;
use uuid::Uuid;
fn setup() -> (DemoStore, Uuid, Uuid) {
    let mut s = DemoStore::new();
    for e in &mut s.data.employees {
        e.joined_on = Some("2025-01-01".parse().unwrap());
    }
    let manager = s
        .data
        .accounts
        .iter()
        .find(|a| a.role == Role::Manager)
        .unwrap()
        .employee_id
        .unwrap();
    let mut e = s.data.employees[0].clone();
    e.id = Uuid::new_v4();
    e.employee_code = "C-STAFF".into();
    e.full_name = "Synthetic career employee".into();
    e.joined_on = Some("2025-01-01".parse().unwrap());
    e.reports_to = Some(manager);
    let id = e.id;
    s.data.employees.push(e);
    (s, id, manager)
}
fn as_manager(s: &mut DemoStore, id: Uuid) {
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
fn assign(s: &mut DemoStore, e: Uuid) -> CareerAction {
    let mut d = PlanDraft::new(Some(e), s.data.company.today());
    d.title = "Synthetic career pathway".into();
    d.starts_on = "2026-01-01".into();
    d.target_on = "2026-12-31".into();
    d.target_role = "Team lead aspiration".into();
    d.reason = "Synthetic independent plan".into();
    let a = CareerAction::PlanSave(d);
    s.execute(Action::Career(a.clone())).unwrap();
    a
}
fn task(s: &mut DemoStore, p: Uuid) -> CareerAction {
    let mut d = MilestoneDraft::new(p);
    d.title = "Practice coaching".into();
    d.action_steps = "Lead a fictional session".into();
    d.due_on = "2026-02-01".into();
    d.reason = "Synthetic assigned milestone".into();
    let a = CareerAction::MilestoneSave(d);
    s.execute(Action::Career(a.clone())).unwrap();
    a
}
fn decide(s: &DemoStore, id: Uuid, status: &str) -> CareerAction {
    let t = s
        .data
        .career
        .milestones
        .iter()
        .find(|t| t.id == id)
        .unwrap();
    CareerAction::MilestoneStatus {
        id,
        revision: t.revision,
        status: status.into(),
        completed_on: if status == "done" {
            "2026-02-01".into()
        } else {
            String::new()
        },
        result: if status == "open" {
            String::new()
        } else {
            "Synthetic reviewed outcome".into()
        },
        reason: "Synthetic independent review".into(),
    }
}
fn close(s: &DemoStore, p: Uuid, status: &str) -> CareerAction {
    let p = plan(&s.data, p).unwrap();
    CareerAction::PlanStatus {
        id: p.id,
        revision: p.revision,
        status: status.into(),
        completed_on: if status == "completed" {
            "2026-02-01".into()
        } else {
            String::new()
        },
        summary: if status == "completed" {
            "Synthetic development outcome".into()
        } else {
            String::new()
        },
        reason: "Synthetic plan decision".into(),
    }
}
#[test]
fn retry_is_actor_bound_and_preserves_later_progress() {
    let (mut s, e, m) = setup();
    let a = assign(&mut s, e);
    let p = a.id();
    let t = task(&mut s, p);
    let tid = t.id();
    s.execute(Action::Career(CareerAction::Progress {
        id: tid,
        revision: 0,
        note: "Later practice".into(),
        reason: "Synthetic progress".into(),
    }))
    .unwrap();
    let n = s.data.career.events.len();
    s.execute(Action::Career(a.clone())).unwrap();
    s.execute(Action::Career(t.clone())).unwrap();
    assert_eq!(s.data.career.events.len(), n);
    assert_eq!(s.data.career.milestones[0].progress_note, "Later practice");
    as_manager(&mut s, m);
    assert!(s.execute(Action::Career(a)).is_err());
    assert!(s.execute(Action::Career(t)).is_err());
}
#[test]
fn self_definition_review_and_waiver_are_denied_but_own_staff_progress_allowed() {
    let (mut s, _, m) = setup();
    let a = assign(&mut s, m);
    let t = task(&mut s, a.id());
    as_manager(&mut s, m);
    let mut d = PlanDraft::new(Some(m), s.data.company.today());
    d.title = "Self defined".into();
    d.reason = "Synthetic denial".into();
    assert!(CareerAction::PlanSave(d).payload(&s.data).is_err());
    s.execute(Action::Career(CareerAction::Progress {
        id: t.id(),
        revision: 0,
        note: "Own learning practice".into(),
        reason: "Synthetic own progress".into(),
    }))
    .unwrap();
    assert!(decide(&s, t.id(), "done").payload(&s.data).is_err());
    assert!(close(&s, a.id(), "cancelled").payload(&s.data).is_err());
}
#[test]
fn current_reporting_scope_is_checked_again_on_every_action() {
    let (mut s, e, m) = setup();
    let a = assign(&mut s, e);
    let t = task(&mut s, a.id());
    as_manager(&mut s, m);
    assert!(can_manage(&s.data, e));
    let mut d = PlanDraft::existing(&s.data.career.plans[0]);
    d.reason = "Current Manager plan edit".into();
    assert!(CareerAction::PlanSave(d.clone()).payload(&s.data).is_ok());
    s.data
        .employees
        .iter_mut()
        .find(|p| p.id == e)
        .unwrap()
        .reports_to = None;
    assert!(!in_scope(&s.data, e));
    assert!(CareerAction::PlanSave(d).payload(&s.data).is_err());
    assert!(visible_events(&s.data, a.id()).is_empty());
    assert!(
        CareerAction::Progress {
            id: t.id(),
            revision: 0,
            note: "Former Manager".into(),
            reason: "Denied".into()
        }
        .payload(&s.data)
        .is_err()
    );
}
#[test]
fn manager_cannot_waive_but_independent_hr_can() {
    let (mut s, e, m) = setup();
    let a = assign(&mut s, e);
    let t = task(&mut s, a.id());
    let admin = s.data.access.clone();
    as_manager(&mut s, m);
    assert!(decide(&s, t.id(), "waived").payload(&s.data).is_err());
    s.data.access = admin;
    s.execute(Action::Career(decide(&s, t.id(), "waived")))
        .unwrap();
    assert_eq!(s.data.career.milestones[0].status, "waived");
    assert!(close(&s, a.id(), "completed").payload(&s.data).is_err());
}
#[test]
fn completion_requires_done_milestone_no_open_required_and_retains_snapshot() {
    let (mut s, e, _) = setup();
    let a = assign(&mut s, e);
    let t = task(&mut s, a.id());
    assert!(close(&s, a.id(), "completed").payload(&s.data).is_err());
    s.execute(Action::Career(decide(&s, t.id(), "done")))
        .unwrap();
    let second = task(&mut s, a.id());
    assert!(close(&s, a.id(), "completed").payload(&s.data).is_err());
    s.execute(Action::Career(decide(&s, second.id(), "waived")))
        .unwrap();
    let complete = close(&s, a.id(), "completed");
    s.execute(Action::Career(complete.clone())).unwrap();
    assert!(complete.saved(&s.data));
    let frozen = s.data.career.plans[0].milestones_snapshot.clone();
    assert_eq!(frozen.len(), 2);
    assert!(decide(&s, t.id(), "open").payload(&s.data).is_err());
    s.execute(Action::Career(close(&s, a.id(), "active")))
        .unwrap();
    let p = &s.data.career.plans[0];
    assert!(
        p.completed_on.is_none()
            && p.completed_by.is_none()
            && p.completion_summary.is_empty()
            && p.milestones_snapshot.is_empty()
    );
    assert_eq!(
        s.data
            .career
            .events
            .iter()
            .find(|e| e.kind == "plan_completed")
            .unwrap()
            .after_record["milestones_snapshot"],
        json!(frozen)
    );
}
#[test]
fn dates_results_and_revisions_fail_without_mutating_data() {
    let (mut s, e, _) = setup();
    let a = assign(&mut s, e);
    let t = task(&mut s, a.id());
    for (date, result) in [
        ("2200-01-01", "Future"),
        ("2025-12-31", "Before start"),
        ("2026-02-01", ""),
    ] {
        let n = s.data.career.events.len();
        assert!(
            s.execute(Action::Career(CareerAction::MilestoneStatus {
                id: t.id(),
                revision: 0,
                status: "done".into(),
                completed_on: date.into(),
                result: result.into(),
                reason: "Synthetic invalid review".into()
            }))
            .is_err()
        );
        assert_eq!(s.data.career.events.len(), n);
        assert_eq!(s.data.career.milestones[0].revision, 0);
    }
    s.execute(Action::Career(decide(&s, t.id(), "done")))
        .unwrap();
    assert!(
        CareerAction::MilestoneStatus {
            id: t.id(),
            revision: 0,
            status: "open".into(),
            completed_on: String::new(),
            result: String::new(),
            reason: "Stale revision".into()
        }
        .payload(&s.data)
        .is_err()
    );
    let mut d = PlanDraft::existing(&s.data.career.plans[0]);
    d.starts_on = "2026-03-01".into();
    d.reason = "Would exclude milestone".into();
    assert!(CareerAction::PlanSave(d).payload(&s.data).is_err());
    let mut c = close(&s, a.id(), "completed");
    if let CareerAction::PlanStatus { completed_on, .. } = &mut c {
        *completed_on = "2026-01-31".into();
    }
    assert!(c.payload(&s.data).is_err());
}
#[test]
fn related_context_is_same_employee_single_source_and_metadata_only() {
    use hris_desktop::learning::{CertificateDraft, LearningAction};
    let (mut s, e, _) = setup();
    let a = assign(&mut s, e);
    let mut c = CertificateDraft::new(Some(e), s.data.company.today());
    c.title = "Synthetic certificate".into();
    c.issuer = "Fictional issuer".into();
    c.reference_number = "PRIVATE-REFERENCE".into();
    c.reason = "Synthetic context".into();
    s.execute(Action::Learning(LearningAction::CertificateSave(c)))
        .unwrap();
    let cert = s.data.learning.certificates[0].id;
    let mut d = MilestoneDraft::new(a.id());
    d.title = "Practice with context".into();
    d.reason = "Synthetic context".into();
    d.certificate_id = Some(cert);
    let t = CareerAction::MilestoneSave(d.clone());
    s.execute(Action::Career(t.clone())).unwrap();
    d.id = Uuid::new_v4();
    d.training_id = Some(Uuid::new_v4());
    assert!(
        CareerAction::MilestoneSave(d.clone())
            .payload(&s.data)
            .is_err()
    );
    d.training_id = None;
    s.data.learning.certificates[0].employee_id = s.data.access.employee_id.unwrap();
    assert!(CareerAction::MilestoneSave(d).payload(&s.data).is_err());
    s.data.learning.certificates[0].employee_id = e;
    s.execute(Action::Career(decide(&s, t.id(), "done")))
        .unwrap();
    s.execute(Action::Career(close(&s, a.id(), "completed")))
        .unwrap();
    let frozen = s.data.career.plans[0].milestones_snapshot.clone();
    let encoded = serde_json::to_string(&frozen).unwrap();
    for secret in ["PRIVATE-REFERENCE", "document_id", "storage_key"] {
        assert!(!encoded.contains(secret));
    }
    s.data.learning.certificates[0].status = "revoked".into();
    assert_eq!(s.data.career.plans[0].milestones_snapshot, frozen);
    assert_eq!(frozen[0]["reference"]["status"], "valid");
}
#[test]
fn aspirations_never_modify_employment_attendance_learning_or_performance() {
    let (mut s, e, _) = setup();
    let before = (
        json!(s.data.employees),
        json!(s.data.attendance),
        json!(s.data.leave),
        json!(s.data.performance.goals),
        json!(s.data.learning.training),
    );
    let a = assign(&mut s, e);
    let t = task(&mut s, a.id());
    s.execute(Action::Career(decide(&s, t.id(), "done")))
        .unwrap();
    s.execute(Action::Career(close(&s, a.id(), "completed")))
        .unwrap();
    assert_eq!(
        (
            json!(s.data.employees),
            json!(s.data.attendance),
            json!(s.data.leave),
            json!(s.data.performance.goals),
            json!(s.data.learning.training)
        ),
        before
    );
}
#[test]
fn archived_employee_retains_history_only_hr_can_cancel() {
    let (mut s, e, m) = setup();
    let a = assign(&mut s, e);
    let t = task(&mut s, a.id());
    s.data
        .employees
        .iter_mut()
        .find(|p| p.id == e)
        .unwrap()
        .status = EmployeeStatus::Archived;
    let admin = s.data.access.clone();
    as_manager(&mut s, m);
    assert!(!in_scope(&s.data, e));
    assert!(
        CareerAction::PlanStatus {
            id: a.id(),
            revision: 0,
            status: "cancelled".into(),
            completed_on: String::new(),
            summary: String::new(),
            reason: "Archived Manager denied".into()
        }
        .payload(&s.data)
        .is_err()
    );
    s.data.access = admin;
    let mut d = PlanDraft::existing(&s.data.career.plans[0]);
    d.reason = "Archived edit".into();
    assert!(CareerAction::PlanSave(d).payload(&s.data).is_err());
    assert!(
        CareerAction::Progress {
            id: t.id(),
            revision: 0,
            note: "Archived progress".into(),
            reason: "Denied".into()
        }
        .payload(&s.data)
        .is_err()
    );
    s.execute(Action::Career(close(&s, a.id(), "cancelled")))
        .unwrap();
    assert!(close(&s, a.id(), "active").payload(&s.data).is_err());
    assert!(s.data.career.events.len() >= 3);
}
#[test]
fn employee_role_missing_identity_and_foreign_company_cannot_mutate() {
    let (mut s, e, _) = setup();
    let a = assign(&mut s, e);
    let t = task(&mut s, a.id());
    let progress = CareerAction::Progress {
        id: t.id(),
        revision: 0,
        note: "Own progress".into(),
        reason: "Synthetic denial".into(),
    };
    s.data.access.role = Role::Employee;
    s.data.access.employee_id = Some(e);
    assert!(progress.payload(&s.data).is_err());
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = None;
    assert!(!can_manage(&s.data, e));
    assert!(progress.payload(&s.data).is_err());
    s.data.access.role = Role::Administrator;
    s.data.access.employee_id = Some(e);
    s.data.access.company_id = Uuid::new_v4();
    assert!(progress.payload(&s.data).is_err());
}
#[test]
fn bounded_milestones_and_changed_receipts_preserve_drafts() {
    let (mut s, e, _) = setup();
    let a = assign(&mut s, e);
    let t = task(&mut s, a.id());
    assert!(a.saved(&s.data) && t.saved(&s.data));
    let mut wrong = s.data.clone();
    wrong.career.plans[0].company_id = Uuid::new_v4();
    assert!(!a.saved(&wrong));
    wrong = s.data.clone();
    wrong.career.milestones[0].title = "Unexpected response".into();
    assert!(!t.saved(&wrong));
    let original = s.data.career.milestones[0].clone();
    for _ in 1..50 {
        let mut copy = original.clone();
        copy.id = Uuid::new_v4();
        s.data.career.milestones.push(copy);
    }
    let mut d = MilestoneDraft::new(a.id());
    d.title = "Beyond limit".into();
    d.reason = "Synthetic limit".into();
    assert!(CareerAction::MilestoneSave(d).payload(&s.data).is_err());
}
