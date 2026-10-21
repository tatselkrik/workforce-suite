use hris_desktop::{
    access::Role,
    account::{DisplayPreferences, ThemeChoice},
    contacts::EmployeeContacts,
    domain::*,
};

#[test]
fn own_contacts_are_editable_without_permission_to_change_employment_or_another_person() {
    let mut store = DemoStore::new();
    let own = store.data.employees[2].id;
    store.data.access.role = Role::Manager;
    store.data.access.employee_id = Some(own);
    let mut contacts = EmployeeContacts::empty(store.data.company.id, own);
    contacts.phone = "000 000 0000".into();
    store
        .execute(Action::SaveOwnContacts(contacts.clone()))
        .unwrap();
    assert_eq!(store.data.own_contacts().unwrap().phone, "000 000 0000");
    let employment = EmployeeDraft::from(store.data.own_employee().unwrap());
    assert!(
        store
            .execute(Action::SaveEmployeeRecord(employment, contacts.clone()))
            .is_err()
    );
    contacts.employee_id = store.data.employees[4].id;
    assert!(store.execute(Action::SaveOwnContacts(contacts)).is_err());
}

#[test]
fn profile_reports_to_name_does_not_require_loading_a_supervisors_employment_record() {
    let mut data = DemoStore::new().data;
    let own = data.access.employee_id.unwrap();
    let manager = uuid::Uuid::from_u128(9001);
    data.employees
        .iter_mut()
        .find(|e| e.id == own)
        .unwrap()
        .reports_to = Some(manager);
    data.access.reports_to_name = Some("Synthetic Supervisor".into());
    assert_eq!(data.employee_name(manager), "Synthetic Supervisor");
    assert_eq!(
        data.employee_name(uuid::Uuid::from_u128(9002)),
        "Unknown employee"
    );
}

#[test]
fn employee_record_saves_employment_and_contacts_together_and_keeps_invalid_edits_out() {
    let mut store = DemoStore::new();
    let person = store.data.employees[2].clone();
    let mut draft = EmployeeDraft::from(&person);
    draft.full_name = "Updated synthetic employee".into();
    let mut contacts = EmployeeContacts::empty(store.data.company.id, person.id);
    contacts.emergency_name = "Synthetic Contact".into();
    assert!(
        store
            .execute(Action::SaveEmployeeRecord(draft.clone(), contacts.clone()))
            .is_err()
    );
    assert_eq!(store.data.employee_name(person.id), person.full_name);
    contacts.emergency_relationship = "Sibling".into();
    contacts.emergency_phone = "000 000 0000".into();
    store
        .execute(Action::SaveEmployeeRecord(draft, contacts))
        .unwrap();
    assert_eq!(
        store.data.employee_name(person.id),
        "Updated synthetic employee"
    );
    assert_eq!(
        store
            .data
            .contacts
            .iter()
            .find(|c| c.employee_id == person.id)
            .unwrap()
            .emergency_name,
        "Synthetic Contact"
    );
}

#[test]
fn reports_to_is_an_employee_relationship_with_cycle_and_archive_guards() {
    let mut store = DemoStore::new();
    let mut first = EmployeeDraft::from(&store.data.employees[2]);
    let mut second = EmployeeDraft::from(&store.data.employees[4]);
    first.reports_to = second.id;
    store.execute(Action::SaveEmployee(first.clone())).unwrap();
    second.reports_to = first.id;
    assert!(store.execute(Action::SaveEmployee(second.clone())).is_err());
    second.reports_to = None;
    second.status = EmployeeStatus::Archived;
    assert!(store.execute(Action::SaveEmployee(second)).is_err());
    first.reports_to = first.id;
    assert!(store.execute(Action::SaveEmployee(first)).is_err());
}

#[test]
fn automatic_employee_details_can_be_completed_later_and_legacy_preferences_keep_light_theme() {
    let employee = EmployeeDraft {
        employee_code: "AUTO-001".into(),
        full_name: "Synthetic Employee".into(),
        ..Default::default()
    }
    .employee(Company::default().id)
    .unwrap();
    assert!(employee.joined_on.is_none());
    assert!(employee.department.is_empty());
    let old: DisplayPreferences =
        serde_json::from_str(r#"{"scale_percent":115,"clock_24h":false}"#).unwrap();
    assert_eq!(old.theme, ThemeChoice::Light);
    for theme in [ThemeChoice::Light, ThemeChoice::Dark, ThemeChoice::System] {
        let prefs = DisplayPreferences { theme, ..old };
        let saved: DisplayPreferences =
            serde_json::from_value(serde_json::to_value(prefs).unwrap()).unwrap();
        assert_eq!(saved.theme, theme);
        assert_eq!(saved.scale_percent, 115);
    }
}
