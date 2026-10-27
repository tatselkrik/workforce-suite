use crate::{
    domain::{Employee, EmployeeStatus, Mode, Snapshot},
    employment::EmploymentFilter,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    pub search: String,
    // None means all departments; Some("") means an unset department.
    pub department: Option<String>,
    pub location: EmploymentFilter,
    pub include_archived: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootReason {
    NoManager,
    ManagerOutsideView,
    InvalidCycle,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub employee: Employee,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub matched: bool,
    pub root_reason: Option<RootReason>,
}

#[derive(Clone, Debug, Default)]
pub struct Chart {
    pub nodes: Vec<Node>,
    pub roots: Vec<usize>,
    pub matching_count: usize,
    pub context_count: usize,
    pub eligible_count: usize,
    pub invalid_cycles: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub index: usize,
    pub depth: usize,
}

/// Mirror the existing directory scope without widening the server-authorized snapshot.
pub fn readable(data: &Snapshot) -> Vec<&Employee> {
    if data.mode == Mode::SignedOut
        || data.company.id.is_nil()
        || data.access.company_id != data.company.id
        || !data.access.uses_desktop()
    {
        return Vec::new();
    }
    data.employees
        .iter()
        .filter(|employee| {
            employee.company_id == data.company.id
                && (data.access.has_full_employee_directory()
                    || (data.access.allows(crate::staff_roles::Capability::TeamReview)
                        && (data.access.employee_id == Some(employee.id)
                            || (employee.status == EmployeeStatus::Active
                                && employee.reports_to == data.access.employee_id
                                && data.access.employee_id.is_some()
                                && data.access.direct_reports.contains(&employee.id)))))
        })
        .collect()
}

impl Chart {
    pub fn build(data: &Snapshot, filters: &Filters) -> Self {
        let mut people: Vec<_> = readable(data)
            .into_iter()
            .filter(|e| {
                e.status == EmployeeStatus::Active
                    || (data.access.has_full_employee_directory() && filters.include_archived)
            })
            .cloned()
            .collect();
        people.sort_by_cached_key(|e| {
            (
                e.full_name.to_lowercase(),
                e.employee_code.to_lowercase(),
                e.id,
            )
        });
        let by_id: HashMap<_, _> = people.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
        let mut parents: Vec<_> = people
            .iter()
            .map(|e| e.reports_to.and_then(|id| by_id.get(&id).copied()))
            .collect();
        // Database writes already prohibit cycles. A malformed/legacy snapshot must still
        // render every authorized employee once without recursion, mutation or a hang.
        let mut color = vec![0u8; people.len()];
        let mut invalid = HashSet::new();
        for start in 0..people.len() {
            let mut path = Vec::new();
            let mut current = Some(start);
            while let Some(index) = current {
                if color[index] != 0 {
                    if color[index] == 1 {
                        parents[index] = None;
                        invalid.insert(index);
                    }
                    break;
                }
                color[index] = 1;
                path.push(index);
                current = parents[index];
            }
            for index in path {
                color[index] = 2;
            }
        }
        let query = filters.search.trim().to_lowercase();
        let mut nodes: Vec<_> = people
            .into_iter()
            .enumerate()
            .map(|(index, employee)| {
                let matched = filters
                    .department
                    .as_ref()
                    .is_none_or(|d| d == &employee.department)
                    && filters.location.matches(employee.work_location_id)
                    && (query.is_empty()
                        || format!(
                            "{} {} {}",
                            employee.full_name, employee.employee_code, employee.job_title
                        )
                        .to_lowercase()
                        .contains(&query));
                let root_reason = parents[index].is_none().then(|| {
                    if invalid.contains(&index) {
                        RootReason::InvalidCycle
                    } else if employee.reports_to.is_some() {
                        RootReason::ManagerOutsideView
                    } else {
                        RootReason::NoManager
                    }
                });
                Node {
                    employee,
                    parent: parents[index],
                    children: Vec::new(),
                    matched,
                    root_reason,
                }
            })
            .collect();
        let mut roots = Vec::new();
        for (index, parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                nodes[*parent].children.push(index);
            } else {
                roots.push(index);
            }
        }
        let eligible_count = nodes.len();
        let matching_count = nodes.iter().filter(|n| n.matched).count();
        let mut ordered = Vec::with_capacity(nodes.len());
        let mut stack = roots.clone();
        while let Some(index) = stack.pop() {
            ordered.push(index);
            stack.extend(nodes[index].children.iter().copied());
        }
        let mut keep: Vec<_> = nodes.iter().map(|n| n.matched).collect();
        for &index in ordered.iter().rev() {
            if keep[index] {
                if let Some(parent) = parents[index] {
                    keep[parent] = true;
                }
            }
        }
        // Remap indexes after filtering; keep authorized ancestors only as context.
        let remap: HashMap<_, _> = (0..nodes.len())
            .filter(|&i| keep[i])
            .enumerate()
            .map(|(new, old)| (old, new))
            .collect();
        let context_count = keep.iter().filter(|&&v| v).count() - matching_count;
        let roots = roots
            .into_iter()
            .filter_map(|old| remap.get(&old).copied())
            .collect();
        let nodes = nodes
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep[*i])
            .map(|(_, mut node)| {
                node.parent = node.parent.and_then(|old| remap.get(&old).copied());
                node.children = node
                    .children
                    .iter()
                    .filter_map(|old| remap.get(old).copied())
                    .collect();
                node
            })
            .collect();
        Self {
            nodes,
            roots,
            matching_count,
            context_count,
            eligible_count,
            invalid_cycles: invalid.len(),
        }
    }

    pub fn rows(&self, collapsed: &HashSet<Uuid>) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut stack: Vec<_> = self
            .roots
            .iter()
            .rev()
            .map(|&index| Row { index, depth: 0 })
            .collect();
        while let Some(row) = stack.pop() {
            let node = &self.nodes[row.index];
            rows.push(row);
            if !collapsed.contains(&node.employee.id) {
                stack.extend(node.children.iter().rev().map(|&index| Row {
                    index,
                    depth: row.depth + 1,
                }));
            }
        }
        rows
    }

    pub fn location_label(data: &Snapshot, id: Option<Uuid>) -> String {
        id.and_then(|id| {
            data.work_locations
                .iter()
                .find(|c| c.id == id && c.company_id == data.company.id)
        })
        .map(|c| format!("{}{}", c.name, if c.active { "" } else { " (inactive)" }))
        .unwrap_or_else(|| "Location not set".into())
    }
}
