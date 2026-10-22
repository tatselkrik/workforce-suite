use hris_desktop::{
    access::Role,
    domain::{DemoStore, Mode},
    report_views::{self, Period, View},
    reports::Filter,
};
#[test]
fn relative_views_resolve_against_today_with_leap_year_and_fixed_dates_retained() {
    let s = DemoStore::new();
    let today = "2024-03-03".parse().unwrap();
    let f = Filter::new("2024-01-08".parse().unwrap());
    for (period, start, end) in [
        (Period::Fixed, "2024-01-01", "2024-01-08"),
        (Period::Last7, "2024-02-26", "2024-03-03"),
        (Period::Last30, "2024-02-03", "2024-03-03"),
        (Period::ThisMonth, "2024-03-01", "2024-03-03"),
    ] {
        let v = View::new(
            s.data.company.id,
            "Synthetic view".into(),
            f.clone(),
            period,
        )
        .unwrap();
        let actual = v.resolve(&s.data, today).unwrap();
        assert_eq!(actual.starts_on.to_string(), start);
        assert_eq!(actual.ends_on.to_string(), end);
    }
}
#[test]
fn a_saved_view_cannot_grant_an_outside_employee_company_or_portal_role_access() {
    let mut s = DemoStore::new();
    let mut f = Filter::new(s.data.company.today());
    f.employee_id = Some(s.data.employees[2].id);
    let mut v = View::new(
        s.data.company.id,
        "Synthetic team".into(),
        f,
        Period::Last30,
    )
    .unwrap();
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = Some(s.data.employees[0].id);
    s.data.access.direct_reports.clear();
    assert!(v.resolve(&s.data, s.data.company.today()).is_err());
    s.data.access.direct_reports.push(s.data.employees[2].id);
    v.resolve(&s.data, s.data.company.today()).unwrap();
    s.data.access.role = Role::Employee;
    assert!(v.resolve(&s.data, s.data.company.today()).is_err());
    s.data.access.role = Role::Administrator;
    v.company_id = uuid::Uuid::new_v4();
    assert!(v.resolve(&s.data, s.data.company.today()).is_err());
    v.company_id = s.data.company.id;
    s.data.mode = Mode::SignedOut;
    assert!(v.resolve(&s.data, s.data.company.today()).is_err());
}
#[test]
fn saved_views_bound_names_dates_count_ids_and_duplicate_names() {
    let s = DemoStore::new();
    let f = Filter::new(s.data.company.today());
    for name in ["", "bad\nname", &"x".repeat(81)] {
        assert!(View::new(s.data.company.id, name.into(), f.clone(), Period::Fixed).is_err());
    }
    let v = View::new(
        s.data.company.id,
        "Synthetic".into(),
        f.clone(),
        Period::Fixed,
    )
    .unwrap();
    assert!(report_views::validate_collection(&[v.clone(), v.clone()]).is_err());
    let mut other = v.clone();
    other.id = uuid::Uuid::new_v4();
    other.name = "SYNTHETIC".into();
    assert!(report_views::validate_collection(&[v.clone(), other]).is_err());
    let many: Vec<_> = (0..21)
        .map(|i| {
            View::new(
                s.data.company.id,
                format!("Synthetic{i}"),
                f.clone(),
                Period::Fixed,
            )
            .unwrap()
        })
        .collect();
    assert!(report_views::validate_collection(&many).is_err());
    let mut invalid = v;
    invalid.filter.ends_on = invalid.filter.starts_on + chrono::Duration::days(366);
    assert!(invalid.validate().is_err());
}
#[test]
fn metadata_contract_contains_filters_only_and_rejects_unknown_periods() {
    let s = DemoStore::new();
    let v = View::new(
        s.data.company.id,
        "Synthetic".into(),
        Filter::new(s.data.company.today()),
        Period::ThisMonth,
    )
    .unwrap();
    let mut value = serde_json::to_value(&v).unwrap();
    assert_eq!(value["period"], "this_month");
    assert!(value.get("contacts").is_none());
    assert_eq!(serde_json::from_value::<View>(value.clone()).unwrap(), v);
    value["period"] = serde_json::json!("arbitrary_sql");
    assert!(serde_json::from_value::<View>(value).is_err());
}

#[test]
fn malformed_or_oversized_account_metadata_is_ignored_without_granting_access() {
    use serde_json::json;
    for value in [
        json!(null),
        json!({"role":"administrator"}),
        json!([{}]),
        json!(vec![serde_json::Value::Null; 21]),
    ] {
        assert!(report_views::from_metadata(Some(&value)).is_empty());
    }
    assert!(report_views::from_metadata(None).is_empty());
    let s = DemoStore::new();
    let v = View::new(
        s.data.company.id,
        "Synthetic".into(),
        Filter::new(s.data.company.today()),
        Period::Fixed,
    )
    .unwrap();
    let valid = serde_json::to_value(vec![v.clone()]).unwrap();
    assert_eq!(report_views::from_metadata(Some(&valid)), vec![v.clone()]);
    assert!(
        report_views::from_metadata(Some(&serde_json::to_value(vec![v.clone(), v]).unwrap()))
            .is_empty()
    );
}
