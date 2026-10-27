use hris_desktop::{
    access::{ManagedAccount, Role},
    domain::{Action, DemoStore, EmployeeStatus},
    performance::*,
};
use uuid::Uuid;
fn fixture() -> (DemoStore, Uuid, Uuid, Uuid) {
    let mut s = DemoStore::new();
    let m = s
        .data
        .accounts
        .iter()
        .find(|a| a.role == Role::Manager)
        .unwrap()
        .clone();
    let mut alt = m.clone();
    alt.user_id = Uuid::new_v4();
    let mut ae = s
        .data
        .employees
        .iter()
        .find(|e| Some(e.id) == m.employee_id)
        .unwrap()
        .clone();
    ae.id = Uuid::new_v4();
    ae.employee_code = "P-ALT".into();
    ae.full_name = "Synthetic alternate Manager".into();
    ae.reports_to = None;
    ae.status = EmployeeStatus::Active;
    alt.employee_id = Some(ae.id);
    s.data.employees.push(ae);
    s.data.accounts.push(alt.clone());
    let mut e = s.data.employees[0].clone();
    e.id = Uuid::new_v4();
    e.employee_code = "P-STAFF".into();
    e.full_name = "Synthetic performance employee".into();
    e.reports_to = m.employee_id;
    e.status = EmployeeStatus::Active;
    e.joined_on = Some("2020-01-01".parse().unwrap());
    let person = e.id;
    s.data.employees.push(e);
    s.data.recruiting.interviewers = s
        .data
        .accounts
        .iter()
        .filter(|a| a.active && a.role.uses_desktop())
        .filter_map(|a| {
            a.employee_id
                .map(|id| hris_desktop::recruiting::Interviewer {
                    employee_id: id,
                    full_name: s.data.employee_name(id).into(),
                    role: a.role,
                    roles: None,
                })
        })
        .collect();
    (s, person, m.user_id, alt.user_id)
}
fn as_user(s: &mut DemoStore, user: Uuid) {
    let a = s.data.accounts.iter().find(|a| a.user_id == user).unwrap();
    s.data.access.user_id = a.user_id;
    s.data.access.role = a.role;
    s.data.access.employee_id = a.employee_id;
}
fn account(s: &DemoStore, user: Uuid) -> ManagedAccount {
    s.data
        .accounts
        .iter()
        .find(|a| a.user_id == user)
        .unwrap()
        .clone()
}
fn setup() -> (DemoStore, Uuid, Uuid, Uuid, Uuid, Uuid) {
    let (mut s, person, m, alt) = fixture();
    let mut g = GoalDraft::new(&s.data, Some(person));
    g.title = "Synthetic agreed objective".into();
    g.reason = "Synthetic goal".into();
    let goal = g.id;
    s.execute(Action::Performance(PerformanceAction::GoalSave(g)))
        .unwrap();
    let mut c = CycleDraft::new(&s.data);
    c.name = "Synthetic cycle".into();
    c.reason = "Synthetic period".into();
    let cycle = c.id;
    s.execute(Action::Performance(PerformanceAction::CycleSave(c)))
        .unwrap();
    (s, person, m, alt, goal, cycle)
}
fn assign(s: &mut DemoStore, person: Uuid, cycle: Uuid, reviewer: Uuid) -> Uuid {
    let mut d = AssignmentDraft::new(Some(person), Some(cycle));
    d.reviewer_employee_id = Some(reviewer);
    d.reason = "Synthetic assignment".into();
    let id = d.id;
    s.execute(Action::Performance(PerformanceAction::Assign(d)))
        .unwrap();
    id
}
fn status(s: &mut DemoStore, id: Uuid, to: &str) -> anyhow::Result<()> {
    let revision = s
        .data
        .performance
        .reviews
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .revision;
    s.execute(Action::Performance(PerformanceAction::ReviewStatus {
        id,
        revision,
        status: to.into(),
        reason: "Synthetic workflow evidence".into(),
    }))
}
#[test]
fn original_actor_retries_keep_current_goal_progress_and_do_not_duplicate_events() {
    let (mut s, person, _, _) = fixture();
    let mut d = GoalDraft::new(&s.data, Some(person));
    d.title = "Agreed goal".into();
    d.reason = "Assigned".into();
    let a = PerformanceAction::GoalSave(d.clone());
    s.execute(Action::Performance(a.clone())).unwrap();
    s.execute(Action::Performance(a.clone())).unwrap();
    assert!(a.saved(&s.data));
    assert_eq!(s.data.performance.events.len(), 1);
    s.execute(Action::Performance(PerformanceAction::Progress {
        id: d.id,
        revision: 0,
        progress: 45,
        note: "Measured".into(),
        reason: "Progress evidence".into(),
    }))
    .unwrap();
    s.execute(Action::Performance(a.clone())).unwrap();
    assert_eq!(s.data.performance.goals[0].progress, 45);
    assert!(!a.saved(&s.data));
    let peer = s
        .data
        .accounts
        .iter()
        .find(|a| a.role == Role::HrGeneralist)
        .unwrap()
        .user_id;
    as_user(&mut s, peer);
    assert!(s.execute(Action::Performance(a)).is_err());
}
#[test]
fn null_reporting_relationships_do_not_allow_manager_definition_or_status_writes() {
    let (mut s, _, m, _) = fixture();
    let own = account(&s, m).employee_id.unwrap();
    let mut d = GoalDraft::new(&s.data, Some(own));
    d.title = "Own goal".into();
    d.reason = "HR setup".into();
    let id = d.id;
    s.execute(Action::Performance(PerformanceAction::GoalSave(d.clone())))
        .unwrap();
    s.data
        .employees
        .iter_mut()
        .find(|e| e.id == own)
        .unwrap()
        .reports_to = None;
    as_user(&mut s, m);
    assert!(!manages_goal(&s.data, own));
    assert!(
        s.execute(Action::Performance(PerformanceAction::GoalSave(d)))
            .is_err()
    );
    s.execute(Action::Performance(PerformanceAction::Progress {
        id,
        revision: 0,
        progress: 20,
        note: String::new(),
        reason: "Own progress".into(),
    }))
    .unwrap();
    assert!(
        s.execute(Action::Performance(PerformanceAction::GoalStatus {
            id,
            revision: 1,
            status: "cancelled".into(),
            reason: "Own decision".into()
        }))
        .is_err()
    );
}
#[test]
fn reassignment_revokes_old_manager_and_hides_their_draft_history_from_replacement() {
    let (mut s, person, m, alt, _, cycle) = setup();
    let admin = s.data.access.user_id;
    let reviewer = account(&s, m).employee_id.unwrap();
    let id = assign(&mut s, person, cycle, reviewer);
    as_user(&mut s, m);
    let mut d = AssessmentDraft::existing(&s.data.performance.reviews[0]);
    d.rating = Some(3);
    d.assessment = "Old reviewer private draft".into();
    d.reason = "Draft evidence".into();
    s.execute(Action::Performance(PerformanceAction::Assessment(d)))
        .unwrap();
    as_user(&mut s, admin);
    let alternate = account(&s, alt).employee_id.unwrap();
    s.data
        .employees
        .iter_mut()
        .find(|e| e.id == person)
        .unwrap()
        .reports_to = Some(alternate);
    let mut d = AssignmentDraft::existing(&s.data.performance.reviews[0]);
    d.reviewer_employee_id = Some(alternate);
    d.reason = "Reporting changed".into();
    s.execute(Action::Performance(PerformanceAction::Assign(d)))
        .unwrap();
    as_user(&mut s, m);
    assert!(review(&s.data, id).is_err());
    as_user(&mut s, alt);
    let r = review(&s.data, id).unwrap();
    assert_eq!(r.rating, None);
    assert!(r.assessment.is_empty());
    assert!(!visible_events(&s.data).iter().any(|e| {
        e.after_record
            .to_string()
            .contains("Old reviewer private draft")
    }));
}
#[test]
fn publication_keeps_submitted_goal_snapshot_and_is_immutable() {
    let (mut s, person, m, _, goal, cycle) = setup();
    let admin = s.data.access.user_id;
    let reviewer = account(&s, m).employee_id.unwrap();
    let id = assign(&mut s, person, cycle, reviewer);
    s.execute(Action::Performance(PerformanceAction::Progress {
        id: goal,
        revision: 0,
        progress: 40,
        note: String::new(),
        reason: "Measured before review".into(),
    }))
    .unwrap();
    as_user(&mut s, m);
    let mut d = AssessmentDraft::existing(&s.data.performance.reviews[0]);
    d.rating = Some(4);
    d.assessment = "Synthetic agreed assessment".into();
    d.reason = "Assessment recorded".into();
    s.execute(Action::Performance(PerformanceAction::Assessment(d)))
        .unwrap();
    status(&mut s, id, "submitted").unwrap();
    assert!(status(&mut s, id, "published").is_err());
    as_user(&mut s, admin);
    s.execute(Action::Performance(PerformanceAction::Progress {
        id: goal,
        revision: 1,
        progress: 100,
        note: String::new(),
        reason: "Later completion".into(),
    }))
    .unwrap();
    status(&mut s, id, "published").unwrap();
    assert_eq!(s.data.performance.reviews[0].goals_snapshot[0].progress, 40);
    assert!(status(&mut s, id, "draft").is_err());
    assert_eq!(s.data.performance.reviews[0].published_by, Some(admin));
}
#[test]
fn hr_notes_are_separate_and_cannot_change_a_published_receipt() {
    let (mut s, person, m, _, _, cycle) = setup();
    let reviewer = account(&s, m).employee_id.unwrap();
    let id = assign(&mut s, person, cycle, reviewer);
    let mut d = NoteDraft::new(id);
    d.note = "Private HR note".into();
    d.reason = "Private context".into();
    let action = PerformanceAction::NoteSave(d);
    s.execute(Action::Performance(action.clone())).unwrap();
    assert!(action.saved(&s.data));
    assert!(
        !serde_json::to_string(&s.data.performance.reviews[0])
            .unwrap()
            .contains("Private HR")
    );
    as_user(&mut s, m);
    assert!(action.payload(&s.data).is_err());
    assert!(!visible_events(&s.data).iter().any(|e| e.note_id.is_some()));
}
#[test]
fn cycles_freeze_after_assignment_and_cannot_close_with_pending_reviews() {
    let (mut s, person, m, _, _, cycle) = setup();
    let reviewer = account(&s, m).employee_id.unwrap();
    assign(&mut s, person, cycle, reviewer);
    let mut d = CycleDraft::existing(&s.data.performance.cycles[0]);
    d.name = "Changed label".into();
    d.reason = "Change".into();
    assert!(
        s.execute(Action::Performance(PerformanceAction::CycleSave(d)))
            .is_err()
    );
    assert!(
        s.execute(Action::Performance(PerformanceAction::CycleStatus {
            id: cycle,
            revision: 0,
            status: "closed".into(),
            reason: "Too early".into()
        }))
        .is_err()
    );
}
#[test]
fn self_assignment_and_incomplete_submission_are_denied() {
    let (mut s, person, m, _, _, cycle) = setup();
    let mut own = AssignmentDraft::new(s.data.access.employee_id, Some(cycle));
    own.reviewer_employee_id = account(&s, m).employee_id;
    own.reason = "Own review".into();
    assert!(
        s.execute(Action::Performance(PerformanceAction::Assign(own)))
            .is_err()
    );
    let reviewer = account(&s, m).employee_id.unwrap();
    let id = assign(&mut s, person, cycle, reviewer);
    as_user(&mut s, m);
    assert!(status(&mut s, id, "submitted").is_err());
}
#[test]
fn employee_role_and_changed_company_cannot_invoke_staff_workflows() {
    let (mut s, person, _, _) = fixture();
    let mut d = GoalDraft::new(&s.data, Some(person));
    d.title = "Goal".into();
    d.reason = "Evidence".into();
    s.data.access.role = Role::Employee;
    assert!(
        s.execute(Action::Performance(PerformanceAction::GoalSave(d.clone())))
            .is_err()
    );
    s.data.access.role = Role::Administrator;
    s.data.access.company_id = Uuid::new_v4();
    assert!(
        s.execute(Action::Performance(PerformanceAction::GoalSave(d)))
            .is_err()
    );
}
#[test]
fn exit_readiness_counts_subject_and_reviewer_open_appraisals() {
    let (mut s, person, m, _, _, cycle) = setup();
    let reviewer = account(&s, m).employee_id.unwrap();
    let id = assign(&mut s, person, cycle, reviewer);
    assert_eq!(unfinished_reviews(&s.data, person), 1);
    assert_eq!(unfinished_reviews(&s.data, reviewer), 1);
    status(&mut s, id, "cancelled").unwrap();
    assert_eq!(unfinished_reviews(&s.data, person), 0);
}
