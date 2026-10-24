use hris_desktop::payroll_wages as w;
use serde_json::{Value, json};
#[test]
fn recorded_rate_boundaries_and_sql_centavos_match() {
    let vectors: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/regional-wage-vectors.json")).unwrap();
    assert_eq!(vectors.len(), 39);
    for v in vectors {
        let source: w::Source = serde_json::from_value(v["source"].clone()).unwrap();
        let got = w::calculate(&source).unwrap();
        let e = &v["expected"];
        assert_eq!(
            got.days
                .iter()
                .map(|d| d.status.as_str())
                .collect::<Vec<_>>(),
            serde_json::from_value::<Vec<String>>(e["statuses"].clone()).unwrap(),
            "{}",
            v["label"]
        );
        assert_eq!(
            got.blocking_days as u64,
            e["blocking_days"].as_u64().unwrap(),
            "{}",
            v["label"]
        );
        assert_eq!(
            got.max_gap_salary_cents,
            e["max_gap_salary_cents"].as_i64().unwrap(),
            "{}",
            v["label"]
        );
        assert_eq!(
            got.exemption_days as u64,
            e["exemption_days"].as_u64().unwrap(),
            "{}",
            v["label"]
        );
        if let Some(required) = e["max_required_salary_cents"].as_i64() {
            assert_eq!(got.max_required_salary_cents, required, "{}", v["label"]);
        }
        if let Some(result) = v.get("result") {
            assert_eq!(
                serde_json::to_value(got).unwrap(),
                *result,
                "SQL parity {}",
                v["label"]
            );
        }
    }
}
#[test]
fn catalog_has_no_zero_rates_or_overlapping_families() {
    let c = w::catalog();
    assert_eq!(c.version, w::VERSION);
    assert_eq!(w::series().len(), 40);
    assert_eq!(
        c.rates
            .iter()
            .map(|r| &r.region)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        17
    );
    assert_eq!(c.rates.len(), 69);
    for r in &c.rates {
        assert!(
            r.daily_basic_cents > 0
                && r.starts_on <= r.ends_on
                && r.source_url.starts_with("https://nwpc.dole.gov.ph/")
        );
        assert!(!c.rates.iter().any(|x| x.id != r.id
            && x.series_key == r.series_key
            && x.starts_on <= r.ends_on
            && x.ends_on >= r.starts_on));
    }
}
#[test]
fn invalid_dates_companies_and_basic_pay_fail() {
    let v: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/regional-wage-vectors.json")).unwrap();
    let base = v[0]["source"].clone();
    for changed in [
        json!({"company_id":"00000000-0000-0000-0000-000000000000"}),
        json!({"starts_on":"2027-01-01","ends_on":"2027-01-01"}),
        json!({"starts_on":"2026-12-01","ends_on":"2026-11-30"}),
        json!({"pay":{"basis":"unknown","rate_cents":55000,"annual_divisor":365}}),
        json!({"pay":{"basis":"daily","rate_cents":0,"annual_divisor":365}}),
        json!({"pay":{"basis":"monthly","rate_cents":55000,"annual_divisor":313}}),
    ] {
        let mut s = base.clone();
        for (k, v) in changed.as_object().unwrap() {
            s[k] = v.clone();
        }
        let s: w::Source = serde_json::from_value(s).unwrap();
        assert!(w::calculate(&s).is_err());
    }
    let mut s: w::Source = serde_json::from_value(base).unwrap();
    s.classifications[0].company_id = uuid::Uuid::new_v4();
    assert!(w::calculate(&s).is_err());
}
#[test]
fn basic_comparison_excludes_tax_flags_identifiers_and_private_allowance_evidence() {
    let mut p = hris_desktop::payroll::Pay {
        rate_cents: 2_000_000,
        ..Default::default()
    };
    p.minimum_wage_earner = true;
    p.coverage_note = "PRIVATE_TAX_EXCEPTION".into();
    p.tin = "PRIVATE_TAX_ID".into();
    let basic = serde_json::to_value(w::BasicPay::from(&p)).unwrap();
    assert_eq!(basic.as_object().unwrap().len(), 3);
    assert!(!basic.to_string().contains("PRIVATE"));
    assert!(basic.get("minimum_wage_earner").is_none());
}
