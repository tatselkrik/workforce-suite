use chrono::NaiveDate;
use hris_desktop::{access::Role, domain::DemoStore, hiring::*};
fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 22).unwrap()
}
fn draft() -> ApplicantDraft {
    ApplicantDraft {
        full_name: "Synthetic Applicant".into(),
        email: "applicant@example.invalid".into(),
        position: "HR Associate".into(),
        applied_on: "2026-10-22".into(),
        ..Default::default()
    }
}
#[test]
fn applicant_contacts_and_dates_are_bounded_without_creating_accounts() {
    let good = draft();
    good.validate(today()).unwrap();
    for edit in 0..6 {
        let mut d = good.clone();
        match edit {
            0 => d.full_name.clear(),
            1 => d.email = "bad address".into(),
            2 => d.phone = "1".repeat(41),
            3 => d.position = " ".into(),
            4 => d.applied_on = "2026-10-23".into(),
            _ => d.notes = "hidden\0text".into(),
        };
        assert!(d.validate(today()).is_err());
    }
    let mut d = good;
    d.email.clear();
    d.phone.clear();
    d.notes = "Interview notes\nSecond line".into();
    d.validate(today()).unwrap();
}
#[test]
fn pipeline_changes_require_reasons_and_closed_applications_reopen_at_applied() {
    Stage::Applied
        .change(Stage::Screening, "Review application")
        .unwrap();
    Stage::Screening
        .change(Stage::Offer, "Experience reviewed")
        .unwrap();
    assert!(Stage::Applied.change(Stage::Offer, "Skip review").is_err());
    assert!(Stage::Interview.change(Stage::Offer, "").is_err());
    assert!(
        Stage::Interview
            .change(Stage::Interview, "Unchanged")
            .is_err()
    );
    for end in [Stage::Rejected, Stage::Withdrawn] {
        Stage::Offer.change(end, "Documented outcome").unwrap();
        end.change(Stage::Applied, "Applicant reapplied").unwrap();
        assert!(end.change(Stage::Offer, "Reopen directly").is_err());
    }
    assert!(serde_json::from_str::<Stage>("\"hired\"").is_err());
}
#[test]
fn normalized_draft_fields_keep_one_retry_identity_and_exclude_protected_fields() {
    let mut d = ApplicantDraft::new(today());
    d.full_name = "  Synthetic Applicant  ".into();
    d.position = " HR Associate ".into();
    let first = d.fields(today()).unwrap();
    let second = d.fields(today()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first["full_name"], "Synthetic Applicant");
    for protected in [
        "company_id",
        "stage",
        "revision",
        "user_id",
        "employee_id",
        "role",
        "password",
    ] {
        assert!(first.get(protected).is_none());
    }
    assert!(draft().fields(today()).is_err());
}
#[test]
fn stage_payload_requires_current_revision_and_an_unarchived_application() {
    let mut r = Applicant {
        id: uuid::Uuid::new_v4(),
        company_id: uuid::Uuid::new_v4(),
        full_name: "Synthetic Applicant".into(),
        email: String::new(),
        phone: String::new(),
        position: "HR Associate".into(),
        source: String::new(),
        applied_on: today(),
        notes: String::new(),
        stage: Stage::Applied,
        archived: false,
        revision: 3,
    };
    let p = r
        .stage_change(3, Stage::Screening, " Review started ")
        .unwrap();
    assert_eq!(p["reason"], "Review started");
    assert!(p.get("actor_id").is_none());
    assert!(r.stage_change(2, Stage::Screening, "Review").is_err());
    r.archived = true;
    assert!(r.stage_change(3, Stage::Screening, "Review").is_err());
}
#[test]
fn applicants_are_hr_private_and_never_inherit_manager_directory_access() {
    let mut data = DemoStore::new().data;
    for role in Role::ALL {
        data.access.role = role;
        assert_eq!(authorize(&data.access, &data).is_ok(), role.manages_hr());
    }
    data.access.role = Role::Administrator;
    data.access.company_id = uuid::Uuid::new_v4();
    assert!(authorize(&data.access, &data).is_err());
}
fn create(store: &mut DemoStore) -> HiringAction {
    let mut d = ApplicantDraft::new(store.data.company.today());
    d.full_name = "Synthetic Applicant".into();
    d.position = "HR Associate".into();
    d.notes = "Private fictional notes".into();
    let action = HiringAction::Create {
        draft: d,
        reason: "Application recorded".into(),
    };
    store
        .execute(hris_desktop::domain::Action::Hiring(action.clone()))
        .unwrap();
    action
}
#[test]
fn applicant_demo_saves_are_audited_and_retries_preserve_newer_edits() {
    let mut s = DemoStore::new();
    let employees = serde_json::to_value(&s.data.employees).unwrap();
    let memberships = serde_json::to_value(&s.data.accounts).unwrap();
    let action = create(&mut s);
    let r = s.data.hiring.records[0].clone();
    assert_eq!(r.stage, Stage::Applied);
    assert_eq!(r.revision, 0);
    s.execute(hris_desktop::domain::Action::Hiring(action.clone()))
        .unwrap();
    assert_eq!(s.data.hiring.events.len(), 1);
    let mut d = ApplicantDraft::existing(&r);
    d.phone = "555-0100".into();
    s.execute(hris_desktop::domain::Action::Hiring(HiringAction::Edit {
        draft: d,
        expected_revision: 0,
        reason: "Contact updated".into(),
    }))
    .unwrap();
    s.execute(hris_desktop::domain::Action::Hiring(action))
        .unwrap();
    assert_eq!(s.data.hiring.events.len(), 2);
    assert_eq!(s.data.hiring.records[0].phone, "555-0100");
    let event = &s.data.hiring.events[1];
    assert_eq!(event.before_record.as_ref().unwrap(), &r);
    assert_eq!(event.after_record, s.data.hiring.records[0]);
    assert_eq!(event.actor_role, Role::Administrator);
    assert_eq!(event.actor_id, s.data.access.user_id);
    assert_eq!(serde_json::to_value(&s.data.employees).unwrap(), employees);
    assert_eq!(serde_json::to_value(&s.data.accounts).unwrap(), memberships);
}
#[test]
fn applicant_demo_failures_leave_records_and_history_unchanged() {
    let mut s = DemoStore::new();
    let action = create(&mut s);
    let r = s.data.hiring.records[0].clone();
    let mut changed = match action.clone() {
        HiringAction::Create { draft, .. } => draft,
        _ => unreachable!(),
    };
    changed.full_name = "Different retry".into();
    let bad = [
        HiringAction::Create {
            draft: changed,
            reason: "Application recorded".into(),
        },
        HiringAction::Stage {
            id: r.id,
            expected_revision: 1,
            next: Stage::Screening,
            reason: "Stale".into(),
        },
        HiringAction::Stage {
            id: r.id,
            expected_revision: 0,
            next: Stage::Offer,
            reason: "Skip review".into(),
        },
        HiringAction::Archive {
            id: r.id,
            expected_revision: 0,
            archived: false,
            reason: "Already active".into(),
        },
    ];
    let before = serde_json::to_value(&s.data.hiring.events).unwrap();
    for a in bad {
        assert!(s.execute(hris_desktop::domain::Action::Hiring(a)).is_err());
        assert_eq!(s.data.hiring.records, vec![r.clone()]);
        assert_eq!(serde_json::to_value(&s.data.hiring.events).unwrap(), before);
    }
    for role in [Role::Manager, Role::Employee] {
        s.data.access.role = role;
        assert!(
            s.execute(hris_desktop::domain::Action::Hiring(action.clone()))
                .is_err()
        );
    }
    assert_eq!(serde_json::to_value(&s.data.hiring.events).unwrap(), before);
}
#[test]
fn applicant_archive_restore_reopen_and_payloads_follow_protected_revisions() {
    let mut s = DemoStore::new();
    create(&mut s);
    let id = s.data.hiring.records[0].id;
    for (revision, next) in [
        Stage::Screening,
        Stage::Interview,
        Stage::Rejected,
        Stage::Applied,
    ]
    .into_iter()
    .enumerate()
    {
        s.execute(hris_desktop::domain::Action::Hiring(HiringAction::Stage {
            id,
            expected_revision: revision as i64,
            next,
            reason: "Reviewed application".into(),
        }))
        .unwrap();
    }
    let archive = HiringAction::Archive {
        id,
        expected_revision: 4,
        archived: true,
        reason: " Retain history ".into(),
    };
    let payload = archive.payload(&s.data).unwrap();
    assert_eq!(payload["p_record"]["reason"], "Retain history");
    assert_eq!(payload["p_operation"], "archive");
    assert_eq!(payload["p_record"].as_object().unwrap().len(), 3);
    s.execute(hris_desktop::domain::Action::Hiring(archive))
        .unwrap();
    let r = s.data.hiring.records[0].clone();
    assert!(r.archived);
    assert!(
        HiringAction::Edit {
            draft: ApplicantDraft::existing(&r),
            expected_revision: 5,
            reason: "Attempt archived edit".into()
        }
        .payload(&s.data)
        .is_err()
    );
    s.execute(hris_desktop::domain::Action::Hiring(
        HiringAction::Archive {
            id,
            expected_revision: 5,
            archived: false,
            reason: "Resume review".into(),
        },
    ))
    .unwrap();
    assert!(!s.data.hiring.records[0].archived);
    assert_eq!(s.data.hiring.events.len(), 7);
}
