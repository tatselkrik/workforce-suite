use hris_desktop::{
    access::Role,
    contacts::EmployeeContacts,
    domain::{Action, DemoStore, EmployeeDraft, EmployeeStatus},
};
use uuid::Uuid;

#[test]
fn optional_contacts_normalize_and_require_a_complete_emergency_contact() {
    let mut contacts = EmployeeContacts::default();
    assert!(contacts.normalized().is_ok());
    contacts.phone = " +63 (000) 000-0000 x123 ".into();
    contacts.personal_email = " sample@example.invalid ".into();
    contacts.address = " Fictional address\nDemo City ".into();
    assert_eq!(
        contacts.normalized().unwrap().phone,
        "+63 (000) 000-0000 x123"
    );
    assert_eq!(
        contacts.normalized().unwrap().address,
        "Fictional address\nDemo City"
    );
    contacts.emergency_alternate_phone = "000 000 0002".into();
    assert!(contacts.normalized().is_err());
    contacts.emergency_name = " Sample Contact ".into();
    contacts.emergency_relationship = " Sibling ".into();
    assert!(contacts.normalized().is_err());
    contacts.emergency_phone = "000 000 0001".into();
    assert_eq!(
        contacts.normalized().unwrap().emergency_name,
        "Sample Contact"
    );
}

#[test]
fn invalid_contacts_fail_without_changing_the_hr_record() {
    let mut store = DemoStore::new();
    let contacts = store.data.contacts[0].clone();
    let before = store.data.contacts.clone();
    let mut invalid = contacts.clone();
    invalid.personal_email = "missing-at.invalid".into();
    assert!(
        store
            .execute(Action::SaveEmployeeContacts(invalid))
            .is_err()
    );
    for phone in ["call me", "12", "000\n000", "000☎000", &"1".repeat(41)] {
        let mut invalid = contacts.clone();
        invalid.phone = phone.into();
        assert!(
            store
                .execute(Action::SaveEmployeeContacts(invalid))
                .is_err()
        );
    }
    let mut invalid = contacts.clone();
    invalid.address = "界".repeat(501);
    assert!(
        store
            .execute(Action::SaveEmployeeContacts(invalid))
            .is_err()
    );
    let mut invalid = contacts.clone();
    invalid.emergency_name = "a".repeat(121);
    assert!(
        store
            .execute(Action::SaveEmployeeContacts(invalid))
            .is_err()
    );
    let mut invalid = contacts.clone();
    invalid.emergency_relationship = "a".repeat(81);
    assert!(
        store
            .execute(Action::SaveEmployeeContacts(invalid))
            .is_err()
    );
    let mut invalid = contacts.clone();
    invalid.company_id = Uuid::new_v4();
    assert!(
        store
            .execute(Action::SaveEmployeeContacts(invalid))
            .is_err()
    );
    let mut invalid = contacts;
    invalid.employee_id = Uuid::new_v4();
    assert!(
        store
            .execute(Action::SaveEmployeeContacts(invalid))
            .is_err()
    );
    assert_eq!(store.data.contacts, before);
}

#[test]
fn hr_can_create_update_clear_and_retain_contacts_when_archiving() {
    let mut store = DemoStore::new();
    store.data.access.role = Role::HrGeneralist;
    let employee = store.data.employees[2].clone();
    let mut contacts = EmployeeContacts::empty(store.data.company.id, employee.id);
    contacts.phone = "000 000 0000".into();
    store
        .execute(Action::SaveEmployeeContacts(contacts.clone()))
        .unwrap();
    contacts.address = "Fictional sample address".into();
    store
        .execute(Action::SaveEmployeeContacts(contacts))
        .unwrap();
    assert_eq!(
        store
            .data
            .contacts
            .iter()
            .filter(|c| c.employee_id == employee.id)
            .count(),
        1
    );
    let mut draft = EmployeeDraft::from(&employee);
    draft.status = EmployeeStatus::Archived;
    store.execute(Action::SaveEmployee(draft)).unwrap();
    assert!(
        store
            .data
            .contacts
            .iter()
            .find(|c| c.employee_id == employee.id)
            .unwrap()
            .updated_at
            .is_some()
    );
    store
        .execute(Action::SaveEmployeeContacts(EmployeeContacts::empty(
            store.data.company.id,
            employee.id,
        )))
        .unwrap();
    assert!(
        store
            .data
            .contacts
            .iter()
            .find(|c| c.employee_id == employee.id)
            .unwrap()
            .address
            .is_empty()
    );
}

#[test]
fn managers_and_portal_employees_have_only_their_own_read_access_and_no_contact_writes() {
    let mut store = DemoStore::new();
    let own = store.data.employees[3].id;
    let report = store.data.employees[1].id;
    store.data.access.employee_id = Some(own);
    store.data.access.direct_reports = vec![report];
    for role in [Role::Manager, Role::Employee] {
        store.data.access.role = role;
        assert!(store.data.access.can_read_contacts(own));
        assert!(!store.data.access.can_read_contacts(report));
        assert!(
            store
                .execute(Action::SaveEmployeeContacts(store.data.contacts[0].clone()))
                .is_err()
        );
    }
}
