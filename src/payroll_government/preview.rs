//! Fictional renderer/test data, excluded from normal distributed builds.
use super::*;
use serde_json::json;

pub(crate) fn fixture() -> Reconciliation {
    let id = Uuid::from_u128;
    let cid = id(1);
    let month: NaiveDate = "2026-09-01".parse().unwrap();
    let at: DateTime<Utc> = "2026-10-25T01:00:00Z".parse().unwrap();
    let regular = Fact {
        company_id: cid,
        employee_id: id(101),
        id: id(91001),
        agency: Agency::Sss,
        kind: Kind::Monthly,
        state: State::Received,
        starts_on: month,
        ends_on: "2026-09-30".parse().unwrap(),
        contribution_month: Some(month),
        received_on: Some("2026-10-01".parse().unwrap()),
        parts: Parts {
            employee_cents: Some(150000),
            employer_cents: Some(300000),
            ec_cents: Some(3000),
        },
    };
    let special = Fact {
        employee_id: id(102),
        id: id(91002),
        kind: Kind::Thirteenth,
        starts_on: "2026-01-01".parse().unwrap(),
        contribution_month: None,
        parts: Parts {
            employee_cents: Some(10000),
            employer_cents: None,
            ec_cents: None,
        },
        ..regular.clone()
    };
    let review: Review = serde_json::from_value(json!({
        "id": id(91003), "company_id": cid, "employee_id": special.employee_id,
        "source_kind": special.kind, "source_id": special.id, "agency": special.agency,
        "status": "submitted", "revision": 1, "previous_id": null,
        "source_basis": Basis::from(&special),
        "allocations": [{"month": month, "employee_cents": 10000, "employer_cents": 20000, "ec_cents": 0}],
        "reference": "FICTIONAL-REVIEW-2026-09", "note": "Fictional coverage evidence for screen verification.",
        "created_by": id(91010), "prepared_by": id(91010), "contributors": [id(91010)],
        "submitted_by": id(91010), "approved_by": null, "approved_at": null,
        "created_at": at, "updated_at": at
    })).unwrap();
    let receipt: Receipt = serde_json::from_value(json!({
        "id": id(91004), "company_id": cid, "agency": "sss", "paid_on": "2026-10-01",
        "amount_cents": 500000,
        "allocations": [{"month": month, "amount_cents": 460000}, {"month": "2026-08-01", "amount_cents": 40000}],
        "reference": "FICTIONAL-SSS-RECEIPT", "evidence": "Fictional bank receipt; no actual payment.",
        "note": "Renderer example only", "replaces_id": null, "recorded_by": id(91011), "recorded_at": at,
        "void_id": null, "void_reason": null, "voided_by": null, "voided_at": null, "replacement_id": null
    })).unwrap();
    let wire = ReviewWire {
        version: "government-reconciliation-v1".into(),
        source: Wire {
            version: "government-payroll-sources-v1".into(),
            company_id: cid,
            company_name: "Northstar Company".into(),
            generated_at: at,
            as_of: "2026-10-25".parse().unwrap(),
            month,
            agency: Agency::Sss,
            facts: vec![regular.clone(), special.clone()],
        },
        effective_facts: vec![regular, special],
        reviews: vec![review.clone()],
        remittances: vec![receipt],
        history: vec![Event {
            id: id(91005),
            company_id: cid,
            record_id: review.id,
            kind: "allocation_submit".into(),
            actor_name: "Fictional HR preparer".into(),
            reason: "Submitted explicit shares for independent review.".into(),
            recorded_at: at,
        }],
        employees: vec![
            EmployeeLabel {
                employee_id: id(101),
                employee_code: "DEMO-001".into(),
                employee_name: "Alex Rivera".into(),
            },
            EmployeeLabel {
                employee_id: id(102),
                employee_code: "DEMO-002".into(),
                employee_name: "Taylor Santos".into(),
            },
        ],
    };
    Reconciliation::from_wire(
        wire,
        cid,
        Filter {
            month,
            agency: Agency::Sss,
        },
    )
    .unwrap()
}
