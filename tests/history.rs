use hris_desktop::{
    access::Role, contacts::EmployeeContacts, domain::*, employment::*, history::*,
};
use uuid::Uuid;

#[test]
fn initial_snapshots_are_explicit_and_do_not_invent_earlier_changes() {
    let store = DemoStore::new();
    assert_eq!(
        store.data.employment_history.len(),
        store.data.employees.len()
    );
    for employee in &store.data.employees {
        let events = timeline(&store.data, employee.id);
        assert_eq!(events.len(), 1);
        let event = events[0];
        assert_eq!(event.kind, HistoryKind::StartingSnapshot);
        assert!(event.before_record.is_none() && event.actor_id.is_none());
        assert_eq!(event.after_record.employee, *employee);
        assert_eq!(event.changes().len(), 10);
    }
}

#[test]
fn employment_edits_capture_before_after_actor_and_only_changed_fields() {
    let mut store = DemoStore::new();
    let person = store.data.employees[1].clone();
    let mut edit = EmployeeDraft::from(&person);
    edit.department = "Client Support".into();
    edit.job_title = "Senior associate".into();
    store.execute(Action::SaveEmployee(edit)).unwrap();
    let events = timeline(&store.data, person.id);
    assert_eq!(events.len(), 2);
    let event = events[0];
    assert_eq!(event.kind, HistoryKind::Updated);
    assert_eq!(event.before_record.as_ref().unwrap().employee, person);
    assert_eq!(event.after_record.employee.job_title, "Senior associate");
    assert_eq!(event.actor_name, "Alex Rivera");
    assert_eq!(event.actor_role, Some(Role::Administrator));
    assert_eq!(
        event.changes().iter().map(|c| c.label).collect::<Vec<_>>(),
        vec!["Department", "Job title"]
    );
}

#[test]
fn contacts_only_and_unchanged_saves_do_not_add_events_and_invalid_saves_rollback() {
    let mut store = DemoStore::new();
    let person = store.data.employees[1].clone();
    let before = store.data.employment_history.clone();
    let mut contact = EmployeeContacts::empty(store.data.company.id, person.id);
    contact.phone = "000 555 0111".into();
    store
        .execute(Action::SaveEmployeeRecord(
            EmployeeDraft::from(&person),
            contact.clone(),
        ))
        .unwrap();
    assert_eq!(store.data.employment_history, before);
    let mut draft = EmployeeDraft::from(&person);
    draft.department = "Must roll back".into();
    contact.emergency_name = "Incomplete".into();
    assert!(
        store
            .execute(Action::SaveEmployeeRecord(draft, contact))
            .is_err()
    );
    assert_eq!(
        store
            .data
            .employees
            .iter()
            .find(|e| e.id == person.id)
            .unwrap(),
        &person
    );
    assert_eq!(store.data.employment_history, before);
}

#[test]
fn a_new_employee_has_one_creation_event_and_archiving_retains_it() {
    let mut store = DemoStore::new();
    let draft = EmployeeDraft {
        employee_code: "QA-HISTORY".into(),
        full_name: "Synthetic history employee".into(),
        annual_leave_days: 15,
        ..Default::default()
    };
    let contact = EmployeeContacts::empty(store.data.company.id, Uuid::nil());
    store
        .execute(Action::SaveEmployeeRecord(draft, contact))
        .unwrap();
    let person = store
        .data
        .employees
        .iter()
        .find(|e| e.employee_code == "QA-HISTORY")
        .unwrap()
        .clone();
    assert_eq!(
        timeline(&store.data, person.id)[0].kind,
        HistoryKind::Created
    );
    let mut edit = EmployeeDraft::from(&person);
    edit.status = EmployeeStatus::Archived;
    store.execute(Action::SaveEmployee(edit)).unwrap();
    let events = timeline(&store.data, person.id);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].kind, HistoryKind::Created);
    assert_eq!(events[0].changes()[0].after, "Archived");
}

#[test]
fn historical_labels_and_actor_names_survive_related_renames() {
    let mut store = DemoStore::new();
    let person = store.data.employees[1].clone();
    let prior = timeline(&store.data, person.id)[0].after_record.clone();
    let option = store
        .data
        .employment_types
        .iter()
        .find(|o| Some(o.id) == person.employment_type_id)
        .unwrap()
        .clone();
    let mut choice = EmploymentOptionDraft::existing(EmploymentKind::EmploymentType, &option);
    choice.name = "New label".into();
    store.execute(Action::SaveEmploymentOption(choice)).unwrap();
    assert_eq!(timeline(&store.data, person.id).len(), 1);
    assert_eq!(timeline(&store.data, person.id)[0].after_record, prior);
    let mut edit = EmployeeDraft::from(&person);
    edit.job_title = "New title".into();
    store.execute(Action::SaveEmployee(edit)).unwrap();
    let mut actor = EmployeeDraft::from(&store.data.employees[0]);
    actor.full_name = "Renamed HR".into();
    store.execute(Action::SaveEmployee(actor)).unwrap();
    assert_eq!(
        timeline(&store.data, person.id)[0].actor_name,
        "Alex Rivera"
    );
    assert_eq!(timeline(&store.data, person.id)[1].after_record, prior);
}

#[test]
fn history_remains_hr_only_company_scoped_and_sequence_ordered() {
    let mut store = DemoStore::new();
    let person = store.data.employees[1].clone();
    let mut edit = EmployeeDraft::from(&person);
    edit.department = "Updated".into();
    store.execute(Action::SaveEmployee(edit)).unwrap();
    store.data.employment_history.reverse();
    assert!(
        timeline(&store.data, person.id)[0].sequence > timeline(&store.data, person.id)[1].sequence
    );
    let mut foreign = timeline(&store.data, person.id)[0].clone();
    foreign.id = Uuid::new_v4();
    foreign.company_id = Uuid::new_v4();
    foreign.sequence = i64::MAX;
    store.data.employment_history.push(foreign);
    assert_eq!(timeline(&store.data, person.id).len(), 2);
    for role in Role::ALL {
        store.data.access.role = role;
        assert_eq!(
            !timeline(&store.data, person.id).is_empty(),
            role.manages_hr()
        );
    }
    assert!(Snapshot::signed_out().employment_history.is_empty());
}

#[test]
fn a_same_name_relationship_change_is_still_visible() {
    let store = DemoStore::new();
    let mut event = timeline(&store.data, store.data.employees[1].id)[0].clone();
    event.before_record = Some(event.after_record.clone());
    event.kind = HistoryKind::Updated;
    event.after_record.employee.reports_to = Some(Uuid::new_v4());
    assert_eq!(event.changes().len(), 1);
    assert_eq!(event.changes()[0].label, "Reports to");
}
