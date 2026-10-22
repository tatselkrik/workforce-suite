use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore},
    hiring::{self, ApplicantDraft},
    onboarding::*,
};
fn fixture() -> (DemoStore, OfferDraft) {
    let mut s = DemoStore::new();
    let own = s.data.access.employee_id.unwrap();
    s.data
        .recruiting
        .interviewers
        .push(hris_desktop::recruiting::Interviewer {
            employee_id: own,
            full_name: "Fictional staff".into(),
            role: Role::Administrator,
        });
    let mut a = ApplicantDraft::new(s.data.company.today());
    a.full_name = "Synthetic approved hire".into();
    a.position = "Associate".into();
    a.email = "synthetic@example.invalid".into();
    let id = a.id.unwrap();
    s.execute(Action::Hiring(hiring::HiringAction::Create {
        draft: a,
        reason: "Application received".into(),
    }))
    .unwrap();
    let mut d = OfferDraft::new(&s.data, id);
    d.amount = "25000.50".into();
    d.department = "Operations".into();
    (s, d)
}
fn decision(s: &mut DemoStore, id: uuid::Uuid, status: OfferStatus) {
    let revision = s
        .data
        .onboarding
        .offers
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .revision;
    s.execute(Action::Onboarding(HiringAction::OfferStatus {
        id,
        revision,
        status,
        reason: "Synthetic evidence recorded".into(),
    }))
    .unwrap();
}
#[test]
fn currency_amounts_are_exact_bounded_minor_units() {
    for (s, v) in [
        ("0", 0),
        ("25.1", 2510),
        ("25000.50", 2500050),
        ("9999999999.99", 999999999999),
    ] {
        assert_eq!(amount_minor(s).unwrap(), v);
        assert_eq!(amount_minor(&money(v)).unwrap(), v);
    }
    for s in [
        "-1",
        "1.005",
        "2.",
        "1,000",
        "NaN",
        "1e2",
        "999999999999999999999999999",
        "10000000000.00",
    ] {
        assert!(amount_minor(s).is_err(), "{s}");
    }
}
#[test]
fn offers_require_admin_approval_keep_revision_history_and_retry_original_identity() {
    let (mut s, d) = fixture();
    s.data.access.role = Role::HrGeneralist;
    let a = HiringAction::OfferSave(d.clone());
    s.execute(Action::Onboarding(a.clone())).unwrap();
    s.execute(Action::Onboarding(a.clone())).unwrap();
    assert_eq!(s.data.onboarding.offers.len(), 1);
    assert_eq!(s.data.onboarding.offer_events.len(), 1);
    let approve = HiringAction::OfferStatus {
        id: d.id,
        revision: 0,
        status: OfferStatus::Approved,
        reason: "Approval".into(),
    };
    assert!(approve.payload(&s.data).is_err());
    s.data.access.role = Role::Administrator;
    s.execute(Action::Onboarding(approve.clone())).unwrap();
    assert!(approve.payload(&s.data).is_err());
    let mut edit = OfferDraft::existing(&s.data.onboarding.offers[0]);
    edit.reason = "Locked terms".into();
    assert!(HiringAction::OfferSave(edit).payload(&s.data).is_err());
    s.execute(Action::Onboarding(a)).unwrap();
    assert_eq!(s.data.onboarding.offers[0].status, OfferStatus::Approved);
    assert_eq!(s.data.onboarding.offer_events.len(), 2);
    s.data.access.role = Role::Manager;
    assert!(offer(&s.data, d.id).is_err());
    assert!(HiringAction::OfferSave(d).payload(&s.data).is_err());
}
#[test]
fn approved_hiring_creates_one_employee_contacts_history_and_required_checklist() {
    let (mut s, d) = fixture();
    s.execute(Action::Onboarding(HiringAction::OfferSave(d.clone())))
        .unwrap();
    let initial = s.data.employees.len();
    let accounts = s.data.accounts.len();
    assert!(
        HiringAction::Hire(HireDraft::new(&s.data.onboarding.offers[0]))
            .payload(&s.data)
            .is_err()
    );
    for status in [
        OfferStatus::Approved,
        OfferStatus::Sent,
        OfferStatus::Accepted,
    ] {
        decision(&mut s, d.id, status);
    }
    let mut hire = HireDraft::new(&s.data.onboarding.offers[0]);
    hire.employee_code = "HIRE-001".into();
    let a = HiringAction::Hire(hire.clone());
    s.execute(Action::Onboarding(a.clone())).unwrap();
    s.execute(Action::Onboarding(a)).unwrap();
    assert_eq!(s.data.employees.len(), initial + 1);
    assert_eq!(s.data.accounts.len(), accounts);
    assert_eq!(s.data.onboarding.conversions.len(), 1);
    assert_eq!(s.data.onboarding.cases.len(), 1);
    assert_eq!(s.data.onboarding.tasks.len(), 5);
    let employee = s.data.onboarding.conversions[0].employee_id;
    assert_eq!(
        s.data
            .contacts
            .iter()
            .find(|c| c.employee_id == employee)
            .unwrap()
            .personal_email,
        "synthetic@example.invalid"
    );
    assert_eq!(
        s.data
            .employment_history
            .iter()
            .filter(|e| e.employee_id == employee)
            .count(),
        1
    );
    let close = HiringAction::CaseStatus {
        id: hire.id,
        revision: 0,
        status: "completed".into(),
        reason: "Finish onboarding".into(),
    };
    assert!(close.payload(&s.data).is_err());
    assert!(
        hiring::HiringAction::Stage {
            id: d.applicant_id,
            expected_revision: 0,
            next: hiring::Stage::Interview,
            reason: "Invalid after hiring".into()
        }
        .payload(&s.data)
        .is_err()
    );
    for t in s.data.onboarding.tasks.clone() {
        s.execute(Action::Onboarding(HiringAction::TaskStatus {
            id: t.id,
            revision: t.revision,
            status: "waived".into(),
            reason: "Synthetic waiver".into(),
        }))
        .unwrap();
    }
    s.execute(Action::Onboarding(close)).unwrap();
    assert!(
        s.data
            .onboarding
            .tasks
            .iter()
            .all(|t| t.case_status == "completed")
    );
    assert_eq!(s.data.onboarding.cases[0].revision, 1);
}
#[test]
fn assigned_manager_tasks_are_scoped_and_waivers_remain_hr_only() {
    let (mut s, d) = fixture();
    s.execute(Action::Onboarding(HiringAction::OfferSave(d.clone())))
        .unwrap();
    for status in [
        OfferStatus::Approved,
        OfferStatus::Sent,
        OfferStatus::Accepted,
    ] {
        decision(&mut s, d.id, status);
    }
    let mut hire = HireDraft::new(&s.data.onboarding.offers[0]);
    hire.employee_code = "HIRE-MGR".into();
    s.execute(Action::Onboarding(HiringAction::Hire(hire.clone())))
        .unwrap();
    let mut task = TaskDraft::existing(&s.data.onboarding.tasks[0]);
    task.assigned_employee_id = s.data.access.employee_id;
    task.reason = "Assign task".into();
    let id = task.id;
    s.execute(Action::Onboarding(HiringAction::TaskSave(task)))
        .unwrap();
    let version = s
        .data
        .onboarding
        .tasks
        .iter()
        .find(|t| t.id == id)
        .unwrap()
        .revision;
    s.data.access.role = Role::Manager;
    let done = HiringAction::TaskStatus {
        id,
        revision: version,
        status: "done".into(),
        reason: "Completed own task".into(),
    };
    s.execute(Action::Onboarding(done.clone())).unwrap();
    assert!(done.payload(&s.data).is_err());
    assert!(
        HiringAction::TaskStatus {
            id,
            revision: version + 1,
            status: "waived".into(),
            reason: "Not authorized".into()
        }
        .payload(&s.data)
        .is_err()
    );
    let other = s.data.onboarding.tasks[1].clone();
    assert!(
        HiringAction::TaskStatus {
            id: other.id,
            revision: other.revision,
            status: "done".into(),
            reason: "Not assigned".into()
        }
        .payload(&s.data)
        .is_err()
    );
    let serialized = serde_json::to_value(&s.data.onboarding.tasks[0]).unwrap();
    for k in ["amount_minor", "terms", "applicant_id", "email"] {
        assert!(serialized.get(k).is_none());
    }
}
