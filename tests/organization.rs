use hris_desktop::{
    access::Role,
    domain::{DemoStore, Employee, EmployeeStatus, Mode, Snapshot},
    employment::EmploymentFilter,
    organization::{self, Chart, Filters, RootReason},
};
use std::collections::HashSet;
use uuid::Uuid;

fn person(
    data: &Snapshot,
    id: u128,
    parent: Option<u128>,
    name: &str,
    department: &str,
    location: Option<u128>,
) -> Employee {
    Employee {
        id: Uuid::from_u128(id),
        company_id: data.company.id,
        employee_code: format!("QA-{id}"),
        full_name: name.into(),
        department: department.into(),
        job_title: format!("Position {id}"),
        joined_on: None,
        reports_to: parent.map(Uuid::from_u128),
        employment_type_id: None,
        work_location_id: location.map(Uuid::from_u128),
        annual_leave_days: 15,
        status: EmployeeStatus::Active,
    }
}
fn fixture() -> Snapshot {
    let mut data = DemoStore::new().data;
    data.employees = vec![
        person(&data, 1, None, "Avery", "Leadership", Some(2000)),
        person(&data, 2, Some(1), "Blair", "Operations", Some(2000)),
        person(&data, 3, Some(2), "Casey", "Operations", Some(2001)),
        person(&data, 4, Some(1), "Dana", "People", Some(2000)),
        person(&data, 5, Some(4), "Erin", "People", Some(2001)),
        person(&data, 6, Some(1), "Frank", "Previous", Some(2000)),
        person(&data, 7, Some(999), "Gale", "Operations", None),
        person(&data, 8, None, "Han", "", None),
    ];
    data.employees[5].status = EmployeeStatus::Archived;
    data
}
fn ids(chart: &Chart) -> Vec<u128> {
    chart
        .rows(&HashSet::new())
        .iter()
        .map(|r| chart.nodes[r.index].employee.id.as_u128())
        .collect()
}

#[test]
fn reporting_tree_is_sorted_and_read_only_and_collapse_hides_only_descendants() {
    let data = fixture();
    let before = data.employees.clone();
    let chart = Chart::build(&data, &Filters::default());
    assert_eq!(ids(&chart), vec![1, 2, 3, 4, 5, 7, 8]);
    assert_eq!(
        (
            chart.matching_count,
            chart.context_count,
            chart.eligible_count
        ),
        (7, 0, 7)
    );
    let rows = chart.rows(&HashSet::from([Uuid::from_u128(2)]));
    assert_eq!(
        rows.iter()
            .map(|r| chart.nodes[r.index].employee.id.as_u128())
            .collect::<Vec<_>>(),
        vec![1, 2, 4, 5, 7, 8]
    );
    assert_eq!(data.employees, before);
    let mut shuffled = data.clone();
    shuffled.employees.reverse();
    assert_eq!(
        ids(&Chart::build(&shuffled, &Filters::default())),
        ids(&chart)
    );
}

#[test]
fn search_and_department_location_filters_keep_only_authorized_reporting_context() {
    let data = fixture();
    let filters = Filters {
        search: "  qA-3  ".into(),
        ..Default::default()
    };
    let chart = Chart::build(&data, &filters);
    assert_eq!(ids(&chart), vec![1, 2, 3]);
    assert_eq!((chart.matching_count, chart.context_count), (1, 2));
    assert_eq!(
        chart
            .nodes
            .iter()
            .filter(|n| n.matched)
            .map(|n| n.employee.id.as_u128())
            .collect::<Vec<_>>(),
        vec![3]
    );
    let chart = Chart::build(
        &data,
        &Filters {
            department: Some("People".into()),
            location: EmploymentFilter::Choice(Uuid::from_u128(2001)),
            ..Default::default()
        },
    );
    assert_eq!(ids(&chart), vec![1, 4, 5]);
    let unset = Chart::build(
        &data,
        &Filters {
            department: Some("".into()),
            location: EmploymentFilter::NotSet,
            ..Default::default()
        },
    );
    assert_eq!(ids(&unset), vec![8]);
    assert!(
        Chart::build(
            &data,
            &Filters {
                search: "no such person".into(),
                ..Default::default()
            }
        )
        .nodes
        .is_empty()
    );
}

#[test]
fn archived_and_inactive_location_assignments_are_retained_only_when_requested() {
    let mut data = fixture();
    data.work_locations[1].active = false;
    let chart = Chart::build(
        &data,
        &Filters {
            include_archived: true,
            ..Default::default()
        },
    );
    assert_eq!(chart.eligible_count, 8);
    assert!(ids(&chart).contains(&6));
    assert_eq!(
        Chart::location_label(&data, Some(Uuid::from_u128(2001))),
        "Remote (inactive)"
    );
    assert_eq!(
        ids(&Chart::build(
            &data,
            &Filters {
                location: EmploymentFilter::Choice(Uuid::from_u128(2001)),
                ..Default::default()
            }
        )),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
fn manager_scope_excludes_indirect_unrelated_archived_and_cross_company_people() {
    let mut data = fixture();
    data.access.role = Role::Manager;
    data.access.employee_id = Some(Uuid::from_u128(2));
    data.access.direct_reports = vec![Uuid::from_u128(3), Uuid::from_u128(5), Uuid::from_u128(6)];
    data.employees[4].reports_to = Some(Uuid::from_u128(3));
    data.employees[5].reports_to = Some(Uuid::from_u128(2));
    let mut foreign = person(&data, 99, Some(2), "Foreign", "Private", None);
    foreign.company_id = Uuid::new_v4();
    data.employees.push(foreign);
    let chart = Chart::build(
        &data,
        &Filters {
            include_archived: true,
            ..Default::default()
        },
    );
    assert_eq!(ids(&chart), vec![2, 3]);
    assert_eq!(
        chart.nodes[0].root_reason,
        Some(RootReason::ManagerOutsideView)
    );
    assert_eq!(organization::readable(&data).len(), 2);
    data.access.employee_id = None;
    assert!(Chart::build(&data, &Filters::default()).nodes.is_empty());
}

#[test]
fn signed_out_employee_and_wrong_company_scopes_never_produce_cards() {
    let mut data = fixture();
    data.access.role = Role::Employee;
    assert!(organization::readable(&data).is_empty());
    data.access.role = Role::Administrator;
    data.access.company_id = Uuid::new_v4();
    assert!(Chart::build(&data, &Filters::default()).nodes.is_empty());
    data.access.company_id = data.company.id;
    data.mode = Mode::SignedOut;
    assert!(Chart::build(&data, &Filters::default()).nodes.is_empty());
}

#[test]
fn missing_managers_and_reporting_cycles_do_not_hide_people_or_invent_records() {
    let mut data = fixture();
    data.employees[0].reports_to = Some(Uuid::from_u128(3));
    data.employees[7].reports_to = Some(Uuid::from_u128(8));
    let before = data.employees.clone();
    let chart = Chart::build(&data, &Filters::default());
    assert_eq!(chart.invalid_cycles, 2);
    let ids = ids(&chart);
    assert_eq!(ids.len(), 7);
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 7);
    assert!(
        chart
            .nodes
            .iter()
            .any(|n| n.employee.id == Uuid::from_u128(7)
                && n.root_reason == Some(RootReason::ManagerOutsideView))
    );
    assert!(
        !chart
            .nodes
            .iter()
            .any(|n| n.employee.id == Uuid::from_u128(999))
    );
    assert_eq!(data.employees, before);
}

#[test]
fn deep_reporting_trees_are_iterative_and_filter_without_recursion() {
    let mut data = fixture();
    data.employees = (1..=1500)
        .map(|id| {
            person(
                &data,
                id,
                (id > 1).then_some(id - 1),
                &format!("Employee {id:04}"),
                "QA",
                None,
            )
        })
        .collect();
    let chart = Chart::build(
        &data,
        &Filters {
            search: "QA-1500".into(),
            ..Default::default()
        },
    );
    assert_eq!((chart.matching_count, chart.context_count), (1, 1499));
    assert_eq!(chart.rows(&HashSet::new()).last().unwrap().depth, 1499);
    assert_eq!(chart.rows(&HashSet::from([Uuid::from_u128(1)])).len(), 1);
}
