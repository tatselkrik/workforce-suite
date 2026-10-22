use hris_desktop::{
    access::Role,
    domain::*,
    imports::{self, Preview},
};
fn preview(data: &Snapshot, text: &str) -> Preview {
    Preview::from_csv(data, "Synthetic.csv".into(), text.as_bytes()).unwrap()
}

#[test]
fn template_is_parseable_and_optional_columns_default_without_creating_accounts() {
    let mut s = DemoStore::new();
    let p = Preview::from_csv(
        &s.data,
        "Template.csv".into(),
        &imports::template().unwrap(),
    )
    .unwrap();
    assert!(p.valid());
    assert_eq!(p.rows[0].contacts.emergency_name, "Synthetic Contact");
    let p = preview(
        &s.data,
        "full_name,employee_code\r\nSynthetic Minimal, new-901\r\n",
    );
    assert!(p.valid());
    assert_eq!(
        p.rows[0].employee.as_ref().unwrap().annual_leave_days,
        s.data.company.annual_leave_days
    );
    let accounts = s.data.accounts.len();
    s.execute(Action::ImportEmployees(
        p,
        "Synthetic onboarding batch".into(),
    ))
    .unwrap();
    assert_eq!(s.data.accounts.len(), accounts);
    assert!(
        s.data
            .employees
            .iter()
            .any(|e| e.employee_code == "NEW-901")
    );
}

#[test]
fn csv_quotes_multiline_cells_bom_and_physical_source_lines_are_preserved() {
    let s = DemoStore::new();
    let p = preview(
        &s.data,
        "\u{feff}employee_code,full_name,address\r\nNEW-901,\"Synthetic \"\"José\"\", employee\",\"Line one\r\nLine two\"\r\nNEW-902,Synthetic Second,Street\r\n",
    );
    assert!(p.valid());
    assert_eq!(p.rows[0].full_name, "Synthetic \"José\", employee");
    assert_eq!(p.rows[0].contacts.address, "Line one\nLine two");
    assert_eq!(p.rows[0].source_row, 2);
    assert_eq!(p.rows[1].source_row, 4);
}

#[test]
fn malformed_encoding_headers_quotes_and_unsupported_account_columns_are_rejected() {
    let s = DemoStore::new();
    for text in [
        "",
        "employee_code\nx",
        "employee_code,full_name,password\nA,B,secret",
        "employee_code,employee_code,full_name\nA,A,B",
        "employee_code,full_name\nA,\"unfinished",
        "employee_code,full_name\nA,un\"quoted",
        "employee_code,full_name\nA,\"closed\" trailing",
        "employee_code,full_name\nA,bad\0name",
    ] {
        assert!(
            Preview::from_csv(&s.data, "Synthetic.csv".into(), text.as_bytes()).is_err(),
            "{text:?}"
        );
    }
    assert!(Preview::from_csv(&s.data, "Synthetic.csv".into(), &[0xff, 0xfe, 0x00]).is_err());
}

#[test]
fn duplicate_existing_and_within_file_codes_report_errors_and_block_entire_payload() {
    let s = DemoStore::new();
    let code = &s.data.employees[0].employee_code;
    let p = preview(
        &s.data,
        &format!(
            "employee_code,full_name\n{code},Synthetic Existing\nnew-901,Synthetic One\nNEW-901,Synthetic Two\n"
        ),
    );
    assert!(!p.valid());
    assert!(
        p.rows[0]
            .errors
            .iter()
            .any(|e| e.contains("already exists"))
    );
    assert!(p.rows[1].errors.iter().any(|e| e.contains("repeated")));
    assert!(p.rows[2].errors.iter().any(|e| e.contains("repeated")));
    assert!(p.rows_payload().is_err());
}

#[test]
fn wrong_cell_counts_empty_quoted_rows_dates_allowances_and_contacts_are_reported() {
    let s = DemoStore::new();
    let p = preview(
        &s.data,
        "employee_code,full_name,joined_on,annual_leave_days,emergency_name,phone\nNEW-901,Synthetic Bad,bad-date,-1,Partial,12\nNEW-902\n\"\"\n",
    );
    assert!(!p.valid());
    assert_eq!(p.rows.len(), 3);
    assert!(p.rows[0].errors.len() >= 3);
    assert!(p.rows[1].errors.iter().any(|s| s.contains("Expected")));
    assert!(p.rows[2].errors.iter().any(|s| s.contains("Expected")));
}

#[test]
fn active_options_and_existing_reporting_manager_are_resolved_and_revalidated() {
    let mut s = DemoStore::new();
    let kind = s
        .data
        .employment_types
        .iter()
        .find(|o| o.active)
        .unwrap()
        .name
        .clone();
    let manager = s.data.employees[0].employee_code.clone();
    let text = format!(
        "employee_code,full_name,employment_type,reports_to_code\nNEW-901,Synthetic One,{kind},{manager}\n"
    );
    let p = preview(&s.data, &text);
    assert!(p.valid());
    p.validate_current(&s.data).unwrap();
    s.data.employees[0].full_name = "Synthetic renamed manager".into();
    assert!(p.validate_current(&s.data).is_err());
    let p = preview(
        &s.data,
        "employee_code,full_name,employment_type,reports_to_code\nNEW-902,Synthetic Two,Nonexistent type,NEW-MANAGER\n",
    );
    assert!(!p.valid());
    assert_eq!(p.rows[0].errors.len(), 2);
}

#[test]
fn inactive_options_and_archived_managers_are_not_silently_assigned() {
    let mut s = DemoStore::new();
    s.data.employment_types[0].active = false;
    let kind = &s.data.employment_types[0].name;
    s.data.employees[0].status = EmployeeStatus::Archived;
    let manager = &s.data.employees[0].employee_code;
    let p = preview(
        &s.data,
        &format!(
            "employee_code,full_name,employment_type,reports_to_code\nNEW-901,Synthetic One,{kind},{manager}\n"
        ),
    );
    assert!(!p.valid());
    assert_eq!(p.rows[0].errors.len(), 2);
}

#[test]
fn successful_batch_is_atomic_audited_idempotent_and_has_one_history_event_per_person() {
    let mut s = DemoStore::new();
    let p = preview(
        &s.data,
        "employee_code,full_name,phone\nNEW-901,Synthetic One,000 000 0901\nNEW-902,Synthetic Two,000 000 0902\n",
    );
    let id = p.id;
    let n = s.data.employees.len();
    let history = s.data.employment_history.len();
    s.execute(Action::ImportEmployees(
        p.clone(),
        "Synthetic onboarding".into(),
    ))
    .unwrap();
    assert_eq!(s.data.employees.len(), n + 2);
    assert_eq!(s.data.employment_history.len(), history + 2);
    assert_eq!(s.data.imports.batches[0].row_count, 2);
    assert_eq!(s.data.imports.items.len(), 2);
    assert_eq!(s.data.imports.batches[0].id, id);
    s.execute(Action::ImportEmployees(
        p.clone(),
        "Synthetic onboarding".into(),
    ))
    .unwrap();
    assert_eq!(s.data.employees.len(), n + 2);
    assert_eq!(s.data.imports.items.len(), 2);
    assert!(
        s.execute(Action::ImportEmployees(p, "Different reason".into()))
            .is_err()
    );
    assert_eq!(s.data.imports.batches.len(), 1);
}

#[test]
fn a_late_validation_failure_rolls_back_people_contacts_history_and_batch_items() {
    let mut s = DemoStore::new();
    let mut p = preview(
        &s.data,
        "employee_code,full_name\nNEW-901,Synthetic One\nNEW-902,Synthetic Two\n",
    );
    p.rows[1].contacts.phone = "invalid phone".into();
    let before = s.data.clone();
    assert!(
        s.execute(Action::ImportEmployees(
            p,
            "Synthetic rollback verification".into()
        ))
        .is_err()
    );
    assert_eq!(s.data.employees, before.employees);
    assert_eq!(s.data.contacts, before.contacts);
    assert_eq!(
        s.data.employment_history.len(),
        before.employment_history.len()
    );
    assert!(s.data.imports.batches.is_empty());
    assert!(s.data.imports.items.is_empty());
}

#[test]
fn changed_codes_and_option_revisions_require_a_new_preview() {
    let mut s = DemoStore::new();
    let kind = s.data.employment_types[0].name.clone();
    let p = preview(
        &s.data,
        &format!("employee_code,full_name,employment_type\nNEW-901,Synthetic One,{kind}\n"),
    );
    assert!(p.valid());
    s.data.employment_types[0].revision += 1;
    assert!(p.validate_current(&s.data).is_err());
    s.data.employment_types[0].revision -= 1;
    s.data.employees[1].employee_code = "NEW-901".into();
    assert!(p.validate_current(&s.data).is_err());
}

#[test]
fn manager_employee_cross_company_and_empty_reason_cannot_commit_imports() {
    let mut s = DemoStore::new();
    let p = preview(&s.data, "employee_code,full_name\nNEW-901,Synthetic One\n");
    for role in [Role::Manager, Role::Employee] {
        s.data.access.role = role;
        assert!(
            Preview::from_csv(
                &s.data,
                "Synthetic.csv".into(),
                b"employee_code,full_name\nNEW-902,Synthetic Two"
            )
            .is_err()
        );
        assert!(
            s.execute(Action::ImportEmployees(
                p.clone(),
                "Synthetic forbidden".into()
            ))
            .is_err()
        );
    }
    s.data.access.role = Role::HrGeneralist;
    assert!(
        s.execute(Action::ImportEmployees(p.clone(), "".into()))
            .is_err()
    );
    s.data.company.id = uuid::Uuid::new_v4();
    assert!(
        s.execute(Action::ImportEmployees(p, "Synthetic wrong company".into()))
            .is_err()
    );
}

#[test]
fn error_export_escapes_formulas_and_excludes_private_contact_cells() {
    let s = DemoStore::new();
    let p = preview(
        &s.data,
        "employee_code,full_name,phone,address\n=FORMULA(),@NAME(),bad,PRIVATE_ADDRESS\n",
    );
    let csv = String::from_utf8(p.error_csv().unwrap()).unwrap();
    assert!(csv.contains("\"'=FORMULA()\""));
    assert!(csv.contains("\"'@NAME()\""));
    assert!(!csv.contains("PRIVATE_ADDRESS"));
    assert!(csv.starts_with('\u{feff}'));
}

#[test]
fn file_size_and_row_limits_fail_before_applying_any_data() {
    let s = DemoStore::new();
    let text = "x".repeat(imports::MAX_BYTES + 1);
    assert!(Preview::from_csv(&s.data, "Big.csv".into(), text.as_bytes()).is_err());
    let text = format!(
        "employee_code,full_name\n{}",
        (0..501)
            .map(|i| format!("NEW-{i},Synthetic {i}\n"))
            .collect::<String>()
    );
    assert!(Preview::from_csv(&s.data, "Many.csv".into(), text.as_bytes()).is_err());
    let text = format!(
        "employee_code,full_name\n{}",
        (0..500)
            .map(|i| format!("NEW-{i},Synthetic {i}\n"))
            .collect::<String>()
    );
    assert_eq!(preview(&s.data, &text).rows.len(), 500);
}

#[test]
fn unicode_rows_that_exceed_the_normalized_server_budget_are_flagged_in_preview() {
    let s = DemoStore::new();
    let wide = "😀";
    let rows = (0..500).map(|i| {
        vec![
            format!("WIDE-{i}"),
            wide.repeat(120),
            wide.repeat(80),
            wide.repeat(100),
            wide.repeat(500),
            wide.repeat(120),
            wide.repeat(80),
            "000".into(),
        ]
    });
    let bytes = imports::csv(
        &[
            "employee_code",
            "full_name",
            "department",
            "job_title",
            "address",
            "emergency_name",
            "emergency_relationship",
            "emergency_phone",
        ],
        rows,
    )
    .unwrap();
    assert!(bytes.len() <= imports::MAX_BYTES);
    let p = Preview::from_csv(&s.data, "Synthetic wide.csv".into(), &bytes).unwrap();
    assert!(!p.valid());
    assert!(
        p.rows[0]
            .errors
            .iter()
            .any(|e| e.contains("normalized import"))
    );
    assert!(p.rows_payload().is_err());
}
