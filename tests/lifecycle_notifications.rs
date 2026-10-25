use chrono::Utc;
use hris_desktop::{
    access::Role,
    domain::DemoStore,
    notifications::{
        self,
        lifecycle::{Inbox, Item},
    },
};
use uuid::Uuid;

fn fixture() -> (hris_desktop::domain::Snapshot, Inbox) {
    let mut data = DemoStore::new().data;
    let own = data.employees[0].id;
    data.access.employee_id = Some(own);
    data.access.role = Role::Manager;
    data.access.direct_reports = vec![data.employees[1].id];
    let id = Uuid::from_u128(46001);
    let day = data.company.today();
    let inbox = Inbox {
        version: "lifecycle-notifications-v1".into(),
        company_id: data.company.id,
        user_id: data.access.user_id,
        employee_id: Some(own),
        role: Role::Manager,
        surface: "desktop".into(),
        as_of: day,
        generated_at: Utc::now(),
        read_keys: vec![],
        total_items: 1,
        truncated: false,
        items: vec![Item {
            key: format!("payroll-monthly-issued:{id}:0"),
            kind: "payroll-monthly-issued".into(),
            title: "Your monthly payslip is available".into(),
            detail: "Approval is separate from payment".into(),
            date: day,
            urgent: false,
            target_id: id,
            target_page: "payroll_monthly".into(),
            employee_id: own,
        }],
    };
    (data, inbox)
}
#[test]
fn new_notice_types_validate_without_broadening_legacy_marker_contract() {
    let (data, inbox) = fixture();
    inbox.validate(&data.access, data.company.today()).unwrap();
    let key = &inbox.items[0].key;
    assert!(notifications::lifecycle::valid_key(key));
    assert!(!notifications::valid_key(key));
    let mut legacy = notifications::ReadState {
        user_id: data.access.user_id,
        company_id: data.company.id,
        read_keys: vec![key.clone()],
    };
    assert!(
        legacy
            .validate(data.access.user_id, data.company.id)
            .is_err()
    );
    legacy.read_keys.clear();
    assert!(
        legacy
            .apply(&notifications::ReadAction::Mark {
                keys: vec![key.clone()],
                read: true
            })
            .is_err()
    );
    let id = Uuid::from_u128(46001);
    for key in [
        format!("training-due:{id}:00:soon"),
        format!("training-due:{id}:0"),
        format!("payroll-monthly-issued:{id}:0:soon"),
        format!("appraisal-review:{id}:-1:today"),
        format!("certificate-expiry:{id}:9223372036854775808:soon"),
    ] {
        assert!(!notifications::lifecycle::valid_key(&key));
    }
}
#[test]
fn malformed_foreign_scope_private_fields_and_count_mismatches_are_rejected() {
    let (data, inbox) = fixture();
    for case in 0..11 {
        let mut bad = inbox.clone();
        match case {
            0 => bad.company_id = Uuid::new_v4(),
            1 => bad.user_id = Uuid::new_v4(),
            2 => bad.employee_id = Some(Uuid::new_v4()),
            3 => bad.role = Role::Administrator,
            4 => bad.surface = "portal".into(),
            5 => bad.as_of = bad.as_of.succ_opt().unwrap(),
            6 => bad.items.push(bad.items[0].clone()),
            7 => bad.items[0].employee_id = data.employees[1].id,
            8 => bad.items[0].target_page = "private_files".into(),
            9 => bad.items[0].title = "bad\nmessage".into(),
            _ => bad.truncated = true,
        }
        assert!(
            bad.validate(&data.access, data.company.today()).is_err(),
            "case{case}"
        );
    }
    let mut wire = serde_json::to_value(&inbox).unwrap();
    wire["items"][0]["payment_reference"] = serde_json::json!("private reference");
    assert!(serde_json::from_value::<Inbox>(wire).is_err());
}
#[test]
fn reporting_changes_revoke_team_learning_and_hr_publication_tasks_stay_restricted() {
    let (mut data, mut inbox) = fixture();
    let item = &mut inbox.items[0];
    let id = item.target_id;
    item.key = format!("training-due:{id}:1:soon");
    item.kind = "training-due".into();
    item.target_page = "training".into();
    item.employee_id = data.employees[1].id;
    inbox.validate(&data.access, data.company.today()).unwrap();
    data.access.direct_reports.clear();
    assert!(inbox.validate(&data.access, data.company.today()).is_err());
    let item = &mut inbox.items[0];
    item.key = format!("appraisal-publish:{id}:1");
    item.kind = "appraisal-publish".into();
    item.target_page = "appraisal".into();
    assert!(inbox.validate(&data.access, data.company.today()).is_err());
    data.access.role = Role::HrGeneralist;
    inbox.role = Role::HrGeneralist;
    inbox.validate(&data.access, data.company.today()).unwrap();
    data.access.role = Role::Employee;
    inbox.role = Role::Employee;
    assert!(inbox.validate(&data.access, data.company.today()).is_err());
}
#[test]
fn mixed_read_updates_are_atomic_and_reset_does_not_change_source_items() {
    let (mut data, inbox) = fixture();
    let new = inbox.items[0].key.clone();
    let source = inbox.items.clone();
    data.lifecycle_notifications = Some(inbox);
    let old = format!("checkout:{}:0", Uuid::from_u128(46010));
    notifications::apply_reads(
        &mut data,
        &notifications::ReadAction::Mark {
            keys: vec![old.clone(), new.clone()],
            read: true,
        },
    )
    .unwrap();
    assert_eq!(data.notification_reads.read_keys, vec![old.clone()]);
    assert!(notifications::is_read(&data, &new));
    assert_eq!(notifications::read_count(&data), 2);
    let unknown = format!("payroll-monthly-issued:{}:0", Uuid::from_u128(46099));
    assert!(
        notifications::apply_reads(
            &mut data,
            &notifications::ReadAction::Mark {
                keys: vec![old.clone(), unknown],
                read: false
            }
        )
        .is_err()
    );
    assert!(notifications::is_read(&data, &old));
    notifications::apply_reads(&mut data, &notifications::ReadAction::Reset).unwrap();
    assert_eq!(notifications::read_count(&data), 0);
    assert_eq!(
        serde_json::to_value(&data.lifecycle_notifications.as_ref().unwrap().items).unwrap(),
        serde_json::to_value(source).unwrap()
    );
}
