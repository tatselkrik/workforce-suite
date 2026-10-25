//! Local decoder for protected fictional QA responses; never contains credentials.
use anyhow::{Result, ensure};
use hris_desktop::{
    access::Access,
    approvals::{Context, Scope},
    domain::*,
    leave, overtime, requests,
};
use serde::Deserialize;
#[derive(Deserialize)]
struct Case {
    company: Company,
    access: Access,
    delegation: Context,
    leave: Vec<LeaveRequest>,
    attendance: Vec<requests::Request>,
    overtime: Vec<overtime::Request>,
}
fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Supply a local protected QA case file."))?;
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path)?)?;
    let mut reviewed = 0;
    for case in &cases {
        let mut d = Snapshot::signed_out();
        d.company = case.company.clone();
        d.access = case.access.clone();
        d.mode = Mode::Supabase;
        d.leave = case.leave.clone();
        d.requests.records = case.attendance.clone();
        d.overtime.records = case.overtime.clone();
        d.approvals = Some(case.delegation.clone());
        case.delegation.validate(&d)?;
        for p in &case.delegation.permits {
            match p.scope {
                Scope::Leave => {
                    let r = d.leave.iter().find(|r| r.id == p.request_id).unwrap();
                    ensure!(
                        leave::can_review(&d, r),
                        "Protected leave permission was not accepted by the native queue."
                    );
                    d.access.authorize(
                        &Action::ReviewLeave(
                            r.id,
                            r.policy.revision,
                            false,
                            "Fictional native DTO verification".into(),
                        ),
                        &d,
                    )?;
                }
                Scope::Attendance => {
                    let r = d
                        .requests
                        .records
                        .iter()
                        .find(|r| r.id == p.request_id)
                        .unwrap();
                    ensure!(
                        requests::can_review_in(&d, r),
                        "Protected correction permission was not accepted by the native queue."
                    );
                    requests::payload(
                        &requests::RequestAction::Resolve {
                            id: r.id,
                            expected_revision: r.revision,
                            status: if p.source_current {
                                requests::Status::Approved
                            } else {
                                requests::Status::Rejected
                            },
                            reason: "Fictional native DTO verification".into(),
                        },
                        &d,
                        chrono::Utc::now(),
                    )?;
                }
                Scope::Overtime => {
                    let r = d
                        .overtime
                        .records
                        .iter()
                        .find(|r| r.id == p.request_id)
                        .unwrap();
                    ensure!(
                        overtime::can_review_in(&d, r),
                        "Protected overtime permission was not accepted by the native queue."
                    );
                    overtime::payload(
                        &overtime::OvertimeAction::Resolve {
                            id: r.id,
                            expected_revision: r.revision,
                            status: if r.status == overtime::Status::Approved {
                                overtime::Status::Revoked
                            } else if p.source_current {
                                overtime::Status::Approved
                            } else {
                                overtime::Status::Rejected
                            },
                            minutes: Some(1),
                            reason: "Fictional native DTO verification".into(),
                        },
                        &d,
                    )?;
                }
            };
            reviewed += 1;
        }
    }
    println!(
        "Verified {} protected native delegation snapshots and {} exact review permissions.",
        cases.len(),
        reviewed
    );
    Ok(())
}
