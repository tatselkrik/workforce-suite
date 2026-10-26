use super::*;
fn wire() -> Wire {
    let r = fixture();
    Wire {
        version: "government-annual-pay-v1".into(),
        ledger: r.ledger.wire,
        details: r.details,
    }
}
fn checked(w: Wire) -> Result<Report> {
    Report::from_wire(w, Uuid::from_u128(1), 2026)
}
#[test]
fn exact_source_coverage_and_immutable_component_sets() {
    let r = checked(wire()).unwrap();
    assert_eq!(r.ledger.wire.entries.len(), 7);
    for e in &r.ledger.wire.entries {
        let d = r.detail(e.fact.kind, e.fact.id);
        assert_eq!(d.employee_id, e.fact.employee_id);
        assert_eq!(d.values.len(), fields(e.fact.kind, e.fact.state).len());
    }
    let thirteenth = &r.details[2].values;
    assert!(thirteenth.contains_key("gross_cents"));
    for k in ["basic_cents", "entitlement_cents", "prior_company_cents"] {
        assert!(!thirteenth.contains_key(k));
    }
    assert!(r.details[1].values.is_empty()); // annual tax is not additional annual pay
    assert!(r.details[6].values.is_empty()); // review is not an automatic recovery
    for k in [
        "source_net_cents",
        "net_cents",
        "cash_before_tax_cents",
        "tax",
    ] {
        assert!(!r.details[4].values.contains_key(k));
    }
}
#[test]
fn foreign_duplicate_missing_and_excess_details_rejected() {
    for n in 0..7 {
        let mut w = wire();
        match n {
            0 => w.details[0].company_id = Uuid::new_v4(),
            1 => w.details[0].employee_id = Uuid::new_v4(),
            2 => w.details[0].id = Uuid::new_v4(),
            3 => w.details[0] = w.details[1].clone(),
            4 => {
                w.details.pop();
            }
            5 => w.details.push(w.details[0].clone()),
            _ => w.version = "unsupported".into(),
        }
        assert!(checked(w).is_err(), "case {n}");
    }
}
#[test]
fn missing_amounts_remain_unknown_and_signs_are_checked() {
    let mut w = wire();
    w.details[0].source_found = false;
    w.details[0].values.values_mut().for_each(|v| *v = None);
    let r = checked(w.clone()).unwrap();
    assert_eq!(r.details[0].values["basic"], None);
    w.details[0].values.insert("basic".into(), Some(0));
    assert!(checked(w).is_err());
    for n in [-1, LIMIT + 1, i64::MIN] {
        let mut w = wire();
        w.details[0].values.insert("basic".into(), Some(n));
        assert!(checked(w).is_err());
    }
    let mut w = wire();
    w.details[2]
        .values
        .insert("sss_adjustment_cents".into(), Some(-12345));
    assert!(checked(w).is_ok());
}
#[test]
fn unknown_json_fields_wrong_money_types_and_extra_private_fields_fail() {
    for (key, value) in [
        ("basic", serde_json::json!("123")),
        ("basic", serde_json::json!(1.25)),
        ("private_note", serde_json::json!(42)),
    ] {
        let mut v = serde_json::to_value(wire()).unwrap();
        v["details"][0]["values"][key] = value;
        let decoded = serde_json::from_value::<Wire>(v);
        assert!(decoded.is_err() || checked(decoded.unwrap()).is_err());
    }
    let mut w = wire();
    w.details[0].values.remove("basic");
    assert!(checked(w).is_err());
    let mut v = serde_json::to_value(wire()).unwrap();
    v["details"][0]["private_note"] = serde_json::json!("PRIVATE_SENTINEL");
    assert!(serde_json::from_value::<Wire>(v).is_err());
}
#[test]
fn pay_export_retains_literal_scope_amounts_and_one_withholding_per_source() {
    let r = fixture();
    let b = r.export(Path::new("review.csv")).unwrap();
    let text = String::from_utf8(b).unwrap();
    assert!(text.contains("000008"));
    assert!(text.contains("Ángela 李"));
    assert!(text.contains("-123.45"));
    assert!(text.contains("not income"));
    assert_eq!(text.lines().count(), 8);
    assert!(!text.contains("entitlement_cents"));
    assert!(r.export(Path::new("review.pdf")).is_err());
    let mut w = wire();
    w.ledger.entries.clear();
    w.ledger.employees.clear();
    w.details.clear();
    let r = checked(w).unwrap();
    let text = String::from_utf8(r.export(Path::new("empty.csv")).unwrap()).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains("not a zero-income"));
}
#[test]
#[ignore = "Emits fictional CSV/Excel pairs for independent workbook review."]
fn emit_fictional_pay_detail_workbooks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/paydetails060-excel");
    std::fs::create_dir(&root).unwrap();
    for name in ["complete", "unknown", "empty"] {
        let mut w = wire();
        if name == "unknown" {
            w.details[0].source_found = false;
            w.details[0].values.values_mut().for_each(|n| *n = None);
        }
        if name == "empty" {
            w.ledger.entries.clear();
            w.ledger.employees.clear();
            w.details.clear();
        }
        let r = checked(w).unwrap();
        for extension in ["csv", "xlsx"] {
            let path = root.join(format!("{name}.{extension}"));
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            f.write_all(&r.export(&path).unwrap()).unwrap();
        }
    }
}
#[test]
#[ignore = "Decode actual local PostgreSQL fictional vectors; no hosted data."]
fn decode_postgres_pay_vectors_and_export() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("work");
    let wires: Vec<Wire> =
        serde_json::from_slice(&std::fs::read(root.join("paydetails060-db-vectors.json")).unwrap())
            .unwrap();
    assert_eq!(wires.len(), 4);
    let folder = root.join("paydetails060-db-excel");
    std::fs::create_dir(&folder).unwrap();
    for (i, w) in wires.into_iter().enumerate() {
        let cid = w.ledger.company_id;
        let year = w.ledger.year;
        let r = Report::from_wire(w, cid, year).unwrap();
        for ext in ["csv", "xlsx"] {
            use std::io::Write;
            let p = folder.join(format!("postgres-{i}.{ext}"));
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)
                .unwrap()
                .write_all(&r.export(&p).unwrap())
                .unwrap();
        }
    }
}
