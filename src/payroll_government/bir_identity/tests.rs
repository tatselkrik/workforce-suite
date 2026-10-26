use super::*;
pub(crate) const FILE_NAME: &str = "0123456780000123120261604C.DAT";
pub(crate) fn source(tins: &[&str]) -> Vec<u8> {
    let mut rows = vec!["H1604C,012345678,0000,12/31/2026".into()];
    for (n, tin) in tins.iter().enumerate() {
        let mut d = vec![String::new(); 49];
        for (i, value) in [
            (0, "D1"),
            (1, "1604C"),
            (2, "012345678"),
            (3, "0000"),
            (4, "12/31/2026"),
            (7, "0000"),
            (8, "FICTIONAL FAMILY"),
            (9, "FICTIONAL GIVEN"),
            (11, "NCR"),
            (23, "01/01/2026"),
            (24, "12/31/2026"),
            (44, "FILIPINO"),
            (45, "R"),
            (46, "NA"),
            (47, "N"),
        ] {
            d[i] = value.into();
        }
        d[5] = (n + 1).to_string();
        d[6] = (*tin).into();
        for &i in &bir::MONEY_COLUMNS {
            d[i] = "0.00".into();
        }
        rows.push(d.join(","));
    }
    let mut c = vec!["C1", "1604C", "012345678", "0000", "12/31/2026"];
    c.extend(["0.00"; 31]);
    rows.push(c.join(","));
    (rows.join("\r\n") + "\r\n").into_bytes()
}
pub(crate) fn wire() -> Wire {
    let ledger = annual::fixture().wire;
    let cid = ledger.company_id;
    let label = ledger.employees[0].clone();
    Wire {
        version: "government-bir-identities-v1".into(),
        ledger,
        employer: Some(Registration {
            id: Uuid::from_u128(100),
            company_id: cid,
            employee_id: None,
            revision: 2,
            tin: "012345678".into(),
            branch: "0000".into(),
        }),
        people: vec![
            Person {
                label: label.clone(),
                registration: Some(Registration {
                    id: Uuid::from_u128(101),
                    company_id: cid,
                    employee_id: Some(label.employee_id),
                    revision: 3,
                    tin: "234567890".into(),
                    branch: "0000".into(),
                }),
            },
            Person {
                label: EmployeeLabel {
                    employee_id: Uuid::from_u128(3),
                    employee_code: "000009".into(),
                    employee_name: label.employee_name,
                },
                registration: Some(Registration {
                    id: Uuid::from_u128(102),
                    company_id: cid,
                    employee_id: Some(Uuid::from_u128(3)),
                    revision: 0,
                    tin: "345678901".into(),
                    branch: "0000".into(),
                }),
            },
        ],
    }
}
fn basis(w: Wire) -> Result<Basis> {
    let cid = w.ledger.company_id;
    Basis::from_wire(w, cid, 2026)
}
fn run(w: Wire, raw: &[u8]) -> Result<Report> {
    review(
        basis(w)?,
        raw,
        FILE_NAME,
        "2026-10-26".parse().unwrap(),
        Uuid::from_u128(4),
    )
}

#[test]
fn exact_pairs_match_namesakes_separately_even_when_file_names_differ() {
    let r = run(wire(), &source(&["234567890", "345678901"])).unwrap();
    assert_eq!(r.rows.len(), 2);
    assert!(
        r.rows
            .iter()
            .all(|r| r.status == Status::CurrentPairMatched)
    );
    assert_eq!(
        r.rows[0].employee.as_ref().unwrap().employee_name,
        r.rows[1].employee.as_ref().unwrap().employee_name
    );
    assert_ne!(
        r.rows[0].employee.as_ref().unwrap().employee_id,
        r.rows[1].employee.as_ref().unwrap().employee_id
    );
    assert_eq!(r.rows[0].received_sources, Some(3));
    assert_eq!(r.rows[1].received_sources, Some(0));
}
#[test]
fn unmatched_is_never_guessed_from_a_name_and_ambiguity_never_picks_one_employee() {
    let r = run(wire(), &source(&["456789012"])).unwrap();
    assert_eq!(r.rows[0].status, Status::Unmatched);
    assert!(r.rows[0].employee.is_none());
    assert_eq!(r.rows[0].received_sources, None);
    assert_eq!(r.received_people_not_uniquely_matched, 1);
    let mut w = wire();
    w.people[1].registration.as_mut().unwrap().tin = "234567890".into();
    let r = run(w, &source(&["234567890"])).unwrap();
    assert_eq!(r.rows[0].status, Status::Ambiguous);
    assert_eq!(r.rows[0].candidates, 2);
    assert!(r.rows[0].employee.is_none());
    assert_eq!(r.rows[0].received_sources, None);
}
#[test]
fn missing_partial_and_placeholder_current_keys_remain_unusable() {
    for variant in 0..4 {
        let mut w = wire();
        match variant {
            0 => w.people[0].registration = None,
            1 => w.people[0].registration.as_mut().unwrap().tin.clear(),
            2 => w.people[0].registration.as_mut().unwrap().branch.clear(),
            _ => w.people[0].registration.as_mut().unwrap().tin = "111111111".into(),
        };
        let r = run(w, &source(&["234567890"])).unwrap();
        assert_eq!(r.rows[0].status, Status::Unmatched);
    }
}
#[test]
fn employer_year_source_hash_and_whole_file_preflight_errors_block_identity_review() {
    let raw = source(&["234567890"]);
    for variant in 0..4 {
        let mut w = wire();
        match variant {
            0 => w.employer = None,
            1 => w.employer.as_mut().unwrap().tin.clear(),
            2 => w.employer.as_mut().unwrap().tin = "345678901".into(),
            _ => {
                w.ledger.year = 2025;
                w.ledger.through_month = "2025-12-01".parse().unwrap();
            }
        };
        assert!(run(w, &raw).is_err());
    }
    assert!(run(wire(), &source(&["234567890", "234567890"])).is_err());
    let mut invalid = raw.clone();
    invalid.extend_from_slice(b"D1,bad\r\n");
    assert!(run(wire(), &invalid).is_err());
    let checked = bir::inspect(
        &raw,
        FILE_NAME,
        "2026-10-26".parse().unwrap(),
        None,
        Uuid::from_u128(1),
        Uuid::from_u128(4),
    )
    .unwrap();
    assert!(bir::checked_identities(&invalid, &checked).is_err());
}
#[test]
fn foreign_revisions_bindings_duplicate_people_and_extra_private_fields_are_rejected() {
    for variant in 0..9 {
        let mut w = wire();
        match variant {
            0 => w.employer.as_mut().unwrap().company_id = Uuid::new_v4(),
            1 => w.employer.as_mut().unwrap().employee_id = Some(Uuid::new_v4()),
            2 => w.people[0].registration.as_mut().unwrap().employee_id = Some(Uuid::new_v4()),
            3 => w.people[0].registration.as_mut().unwrap().revision = -1,
            4 => w.people[0].registration.as_mut().unwrap().tin = "234-567-890".into(),
            5 => w.people.push(w.people[0].clone()),
            6 => w.people[0].label.employee_name = "Wrong source label".into(),
            7 => w.people[1].registration.as_mut().unwrap().id = w.employer.as_ref().unwrap().id,
            _ => w.people[1].label.employee_code = w.people[0].label.employee_code.clone(),
        };
        assert!(basis(w).is_err(), "variant {variant}");
    }
    let mut value = serde_json::to_value(wire()).unwrap();
    value["people"][0]["private_evidence"] = serde_json::json!("PRIVATE_SENTINEL");
    assert!(serde_json::from_value::<Wire>(value).is_err());
}
#[test]
fn roster_limit_and_missing_source_person_fail_without_truncation() {
    let mut w = wire();
    w.people = vec![w.people[0].clone(); 20001];
    assert!(basis(w).is_err());
    let mut w = wire();
    w.people.remove(0);
    assert!(basis(w).is_err());
}
#[test]
fn identity_exports_have_literal_codes_current_labels_and_counts_without_tax_ids_or_money() {
    let r = run(wire(), &source(&["234567890", "456789012"])).unwrap();
    let raw = r.export(Path::new("review.xlsx")).unwrap();
    let rows = crate::tabular::read_xlsx(&raw, 10, 20).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].cells[7], "000008");
    assert_eq!(rows[1].cells[11], "3");
    assert!(rows[2].cells[7].is_empty());
    assert!(rows[2].cells[11].is_empty());
    let all = rows
        .iter()
        .flat_map(|r| r.cells.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    for private in [
        "012345678",
        "234567890",
        "456789012",
        "FICTIONAL FAMILY",
        "FICTIONAL GIVEN",
        "withholding_cents",
        "PRIVATE_SENTINEL",
    ] {
        assert!(!all.contains(private));
    }
    assert!(
        String::from_utf8(r.export(Path::new("review.csv")).unwrap())
            .unwrap()
            .contains("CURRENT IDENTITY REVIEW")
    );
}
#[test]
fn standalone_mwe_uses_the_same_identity_pair_without_claiming_eligibility() {
    let r = run(wire(), &bir::mwe_tests::fictional_source()).unwrap();
    assert_eq!(r.schedule, bir::Schedule::Two);
    assert!(r.preflight_reviews > 0);
    assert!(
        r.rows
            .iter()
            .any(|r| r.status == Status::CurrentPairMatched)
    );
}

#[test]
#[ignore = "Set HRIS_IDENTITY_VECTORS to explicit fictional database or hosted snapshot evidence."]
fn actual_database_identity_snapshots_decode() {
    let path = std::env::var("HRIS_IDENTITY_VECTORS").unwrap();
    let wires: Vec<Wire> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(wires.len() >= 2);
    for w in wires {
        let cid = w.ledger.company_id;
        let year = w.ledger.year;
        let b = Basis::from_wire(w, cid, year).unwrap();
        assert!(b.people.len() >= b.ledger.wire.employees.len());
    }
}

#[test]
#[ignore = "Writes six fictional identity-review examples in this project's fixed work folder."]
fn fictional_identity_workbooks_and_reports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/identities058-fictional-cases");
    std::fs::create_dir(&root).unwrap();
    let mut index = vec![];
    for case in [
        "namesakes",
        "unmatched",
        "ambiguous",
        "mwe",
        "no-sources",
        "incomplete-registration",
    ] {
        let folder = root.join(case);
        std::fs::create_dir(&folder).unwrap();
        let mut w = wire();
        if case == "ambiguous" {
            w.people[1].registration.as_mut().unwrap().tin = "234567890".into();
        }
        if case == "incomplete-registration" {
            w.people[0].registration = None;
        }
        if case == "no-sources" {
            w.ledger.entries.clear();
            w.ledger.employees.clear();
        }
        if case == "namesakes" {
            for p in &mut w.people {
                p.label.employee_name = "=Fictional, Ángela 李".into();
            }
            w.ledger.employees[0].employee_name = "=Fictional, Ángela 李".into();
        }
        let raw = match case {
            "mwe" => bir::mwe_tests::fictional_source(),
            "unmatched" => source(&["456789012"]),
            "namesakes" => source(&["234567890", "345678901"]),
            _ => source(&["234567890"]),
        };
        let r = run(w.clone(), &raw).unwrap();
        std::fs::write(folder.join(FILE_NAME), &raw).unwrap();
        std::fs::write(
            folder.join("review.csv"),
            r.export(Path::new("review.csv")).unwrap(),
        )
        .unwrap();
        std::fs::write(
            folder.join("review.xlsx"),
            r.export(Path::new("review.xlsx")).unwrap(),
        )
        .unwrap();
        std::fs::write(folder.join("report.json"), serde_json::to_vec(&r).unwrap()).unwrap();
        std::fs::write(
            folder.join("fictional-basis.json"),
            serde_json::to_vec(&w).unwrap(),
        )
        .unwrap();
        assert_eq!(std::fs::read(folder.join(FILE_NAME)).unwrap(), raw);
        index.push(case);
    }
    std::fs::write(root.join("index.json"), serde_json::to_vec(&index).unwrap()).unwrap();
}
