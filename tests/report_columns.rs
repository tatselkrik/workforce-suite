use chrono::{Duration, Utc};
use hris_desktop::{
    domain::DemoStore,
    report_views::{self, Period, View},
    reports::{self, Filter, Kind},
};
#[test]
fn every_report_projects_exactly_the_requested_columns_in_order() {
    let s = DemoStore::new();
    let today = s.data.company.today();
    for kind in Kind::ALL {
        let mut f = Filter::new(today);
        f.kind = kind;
        f.ends_on = today + Duration::days(20);
        let full = reports::build(&s.data, &f, Utc::now()).unwrap();
        let available = reports::available_columns(kind);
        assert_eq!(full.headers, available);
        let selected = vec![
            available.last().unwrap().to_string(),
            available[0].to_string(),
        ];
        f.columns = Some(selected.clone());
        let projected = reports::build(&s.data, &f, full.generated_at).unwrap();
        assert_eq!(projected.headers, selected);
        assert_eq!(projected.rows.len(), full.rows.len());
        assert_eq!(projected.employees, full.employees);
        for (row, original) in projected.rows.iter().zip(&full.rows) {
            assert_eq!(
                row,
                &vec![original.last().unwrap().clone(), original[0].clone()]
            );
        }
        let csv = String::from_utf8(projected.csv().unwrap()).unwrap();
        assert!(
            csv.lines()
                .next()
                .unwrap()
                .ends_with(&format!("\"{}\",\"{}\"", selected[0], selected[1]))
        );
    }
}
#[test]
fn empty_duplicate_foreign_kind_and_private_columns_are_rejected() {
    let s = DemoStore::new();
    let mut f = Filter::new(s.data.company.today());
    for columns in [
        vec![],
        vec!["Full name".into(), "Full name".into()],
        vec!["Personal phone".into()],
        vec!["Active employees".into()],
    ] {
        f.columns = Some(columns);
        assert!(f.validate().is_err());
        assert!(reports::build(&s.data, &f, Utc::now()).is_err());
    }
    f.columns = Some(vec!["Full name".into()]);
    f.validate().unwrap();
}
#[test]
fn saved_views_retain_column_order_and_older_views_default_to_all_columns() {
    let s = DemoStore::new();
    let mut f = Filter::new(s.data.company.today());
    f.columns = Some(vec!["Work date".into(), "Employee code".into()]);
    let view = View::new(
        s.data.company.id,
        "Synthetic columns".into(),
        f.clone(),
        Period::Last7,
    )
    .unwrap();
    let encoded = serde_json::to_value(vec![view]).unwrap();
    let decoded = report_views::from_metadata(Some(&encoded));
    assert_eq!(
        decoded[0]
            .resolve(&s.data, s.data.company.today())
            .unwrap()
            .columns,
        f.columns
    );
    let mut legacy = encoded.clone();
    legacy[0]["filter"]
        .as_object_mut()
        .unwrap()
        .remove("columns");
    let legacy = report_views::from_metadata(Some(&legacy));
    assert!(legacy[0].filter.columns.is_none());
}
#[test]
fn csv_blocks_a_filter_changed_after_generation_and_retains_required_metadata() {
    let s = DemoStore::new();
    let mut f = Filter::new(s.data.company.today());
    f.columns = Some(vec!["Employee code".into()]);
    let mut r = reports::build(&s.data, &f, Utc::now()).unwrap();
    let csv = String::from_utf8(r.csv().unwrap()).unwrap();
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.contains("\"Company\",\"Report\",\"From\",\"To\",\"Generated at (UTC)\",\"Company time zone\",\"Employee code\""));
    assert!(!csv.contains("\"Full name\""));
    r.filter.columns = Some(vec!["Full name".into()]);
    assert!(r.csv().is_err());
}
