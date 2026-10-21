use hris_desktop::{access::Role, contacts::EmployeeContacts, domain::*, employment::*};
use uuid::Uuid;

#[test]
fn choice_rename_disable_and_reactivate_preserve_canonical_employee_assignments() {
    let mut store = DemoStore::new();
    let original = store.data.employees.clone();
    let option = store.data.employment_types[0].clone();
    let mut edit = EmploymentOptionDraft::existing(EmploymentKind::EmploymentType, &option);
    edit.name = "Company regular assignment".into();
    edit.active = false;
    store
        .execute(Action::SaveEmploymentOption(edit.clone()))
        .unwrap();
    assert_eq!(store.data.employees, original);
    assert!(
        EmploymentKind::EmploymentType
            .display(&store.data, Some(option.id))
            .contains("inactive")
    );
    assert!(
        store
            .execute(Action::SaveEmploymentOption(edit))
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    let mut current = EmploymentOptionDraft::existing(
        EmploymentKind::EmploymentType,
        &store
            .data
            .employment_types
            .iter()
            .find(|o| o.id == option.id)
            .unwrap(),
    );
    current.active = true;
    store
        .execute(Action::SaveEmploymentOption(current))
        .unwrap();
    let updated = store
        .data
        .employment_types
        .iter()
        .find(|o| o.id == option.id)
        .unwrap();
    assert_eq!(updated.revision, 2);
    assert!(updated.active);
    assert_eq!(store.data.employees, original);
}

#[test]
fn inactive_existing_assignments_can_be_retained_but_cannot_be_given_to_another_employee() {
    let mut store = DemoStore::new();
    let option = store.data.employment_types[0].clone();
    let existing = store
        .data
        .employees
        .iter()
        .find(|e| e.employment_type_id == Some(option.id))
        .unwrap()
        .clone();
    let other = store
        .data
        .employees
        .iter()
        .find(|e| e.employment_type_id != Some(option.id))
        .unwrap()
        .clone();
    let mut inactive = EmploymentOptionDraft::existing(EmploymentKind::EmploymentType, &option);
    inactive.active = false;
    store
        .execute(Action::SaveEmploymentOption(inactive))
        .unwrap();
    let mut retained = EmployeeDraft::from(&existing);
    retained.department = "Updated department".into();
    store.execute(Action::SaveEmployee(retained)).unwrap();
    let mut invalid = EmployeeDraft::from(&other);
    invalid.employment_type_id = Some(option.id);
    assert!(
        store
            .execute(Action::SaveEmployee(invalid))
            .unwrap_err()
            .to_string()
            .contains("active employment type")
    );
    let mut cleared = EmployeeDraft::from(&existing);
    cleared.employment_type_id = None;
    store.execute(Action::SaveEmployee(cleared)).unwrap();
    assert_eq!(
        store
            .data
            .employees
            .iter()
            .find(|e| e.id == existing.id)
            .unwrap()
            .employment_type_id,
        None
    );
}

#[test]
fn invalid_company_or_collection_choice_rolls_back_employee_and_private_contact_edits() {
    let mut store = DemoStore::new();
    let original = store.data.clone();
    let person = store.data.employees[1].clone();
    let mut draft = EmployeeDraft::from(&person);
    draft.work_location_id = person.employment_type_id;
    let mut contact = EmployeeContacts::empty(store.data.company.id, person.id);
    contact.phone = "000 555 0123".into();
    assert!(
        store
            .execute(Action::SaveEmployeeRecord(draft, contact))
            .is_err()
    );
    assert_eq!(store.data.employees, original.employees);
    assert_eq!(store.data.contacts, original.contacts);
    let mut foreign = store.data.work_locations[0].clone();
    foreign.id = Uuid::new_v4();
    foreign.company_id = Uuid::new_v4();
    let mut draft = EmployeeDraft::from(&person);
    draft.work_location_id = Some(foreign.id);
    store.data.work_locations.push(foreign);
    assert!(
        validate_assignments(&store.data, &draft.employee(store.data.company.id).unwrap()).is_err()
    );
}

#[test]
fn only_administrators_manage_choices_while_hr_can_assign_them() {
    for role in [
        Role::Administrator,
        Role::HrGeneralist,
        Role::Manager,
        Role::Employee,
    ] {
        let mut store = DemoStore::new();
        store.data.access.role = role;
        let mut new = EmploymentOptionDraft::new(EmploymentKind::WorkLocation);
        new.name = "Field assignment".into();
        assert_eq!(
            store.execute(Action::SaveEmploymentOption(new)).is_ok(),
            role == Role::Administrator
        );
        let mut person = EmployeeDraft::from(&store.data.employees[1]);
        person.work_location_id = Some(store.data.work_locations[0].id);
        assert_eq!(
            store.execute(Action::SaveEmployee(person)).is_ok(),
            role.manages_hr()
        );
    }
}

#[test]
fn invalid_or_duplicate_labels_cannot_change_the_company_dictionary() {
    let mut store = DemoStore::new();
    let before = store.data.work_locations.clone();
    for name in [
        String::new(),
        "  ".into(),
        "a".repeat(81),
        "Bad\nlabel".into(),
        "mAIN OFFICE".into(),
    ] {
        let mut draft = EmploymentOptionDraft::new(EmploymentKind::WorkLocation);
        draft.name = name;
        assert!(store.execute(Action::SaveEmploymentOption(draft)).is_err());
    }
    assert_eq!(store.data.work_locations, before);
    let mut good = EmploymentOptionDraft::new(EmploymentKind::WorkLocation);
    good.name = "  Field office  ".into();
    store.execute(Action::SaveEmploymentOption(good)).unwrap();
    assert!(
        store
            .data
            .work_locations
            .iter()
            .any(|c| c.name == "Field office")
    );
}

#[test]
fn employment_filters_keep_unset_and_inactive_assignments_distinct() {
    let mut store = DemoStore::new();
    let type_id = store.data.employment_types[0].id;
    let location = store.data.work_locations[0].id;
    store.data.employment_types[0].active = false;
    let matches: Vec<_> = store
        .data
        .employees
        .iter()
        .filter(|e| {
            EmploymentFilter::Choice(type_id).matches(e.employment_type_id)
                && EmploymentFilter::Choice(location).matches(e.work_location_id)
        })
        .collect();
    assert!(!matches.is_empty());
    assert!(
        matches
            .iter()
            .all(|e| e.employment_type_id == Some(type_id) && e.work_location_id == Some(location))
    );
    let unset: Vec<_> = store
        .data
        .employees
        .iter()
        .filter(|e| {
            EmploymentFilter::NotSet.matches(e.employment_type_id)
                && EmploymentFilter::NotSet.matches(e.work_location_id)
        })
        .collect();
    assert_eq!(unset.len(), 1);
    assert_ne!(unset[0].id, matches[0].id);
}

#[test]
fn legacy_employee_json_defaults_to_unset_and_new_drafts_preserve_both_ids() {
    let store = DemoStore::new();
    let person = &store.data.employees[0];
    let mut legacy = serde_json::to_value(person).unwrap();
    legacy.as_object_mut().unwrap().remove("employment_type_id");
    legacy.as_object_mut().unwrap().remove("work_location_id");
    let old: Employee = serde_json::from_value(legacy).unwrap();
    assert!(old.employment_type_id.is_none() && old.work_location_id.is_none());
    let copy = EmployeeDraft::from(person)
        .employee(store.data.company.id)
        .unwrap();
    assert_eq!(copy.employment_type_id, person.employment_type_id);
    assert_eq!(copy.work_location_id, person.work_location_id);
}
