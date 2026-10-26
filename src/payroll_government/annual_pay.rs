//! Issued components, not a BIR classification or another annual tax basis.
use super::*;
use std::{collections::BTreeMap, path::Path};

pub const NOTE: &str = "Recorded components overlap: exempt allowances are within allowances; reimbursements and returned deposits are not classified as income. Final-pay contained source net and annual tax basis are excluded. No expected BIR amounts or cross-source total is inferred.";
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub kind: Kind,
    pub id: Uuid,
    pub source_found: bool,
    pub values: BTreeMap<String, Option<i64>>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub version: String,
    pub ledger: annual::Wire,
    pub details: Vec<Detail>,
}
#[derive(Clone, Debug)]
pub struct Report {
    pub ledger: annual::Ledger,
    pub details: Vec<Detail>,
    indices: HashMap<(Kind, Uuid), usize>,
}
#[derive(Clone, Copy)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    signed: bool,
}
const fn field(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        signed: false,
    }
}
const fn signed(key: &'static str, label: &'static str) -> Field {
    Field {
        key,
        label,
        signed: true,
    }
}
const REGULAR: &[Field] = &[
    field("basic", "Issued basic pay"),
    field("paid_leave", "Paid leave"),
    field("unworked_holiday", "Unworked holiday pay"),
    field("premium", "Premium pay"),
    field("overtime", "Overtime"),
    field("night", "Night differential"),
    field("allowances", "Allowances (includes exempt part)"),
    field("exempt_allowances", "Exempt part of allowances"),
    field("reimbursement", "Reimbursement (outside gross)"),
    field("gross", "Issued gross pay"),
    field("taxable", "Recorded taxable pay"),
    field("sss_employee", "Employee SSS"),
    field("philhealth_employee", "Employee PhilHealth"),
    field("pagibig_employee", "Employee Pag-IBIG"),
];
const THIRTEENTH: &[Field] = &[
    field("gross_cents", "Current 13th-month payout"),
    field("exempt_cents", "Exempt part of current payout"),
    field("taxable_cents", "Taxable part of current payout"),
    signed("sss_adjustment_cents", "SSS adjustment"),
    signed("philhealth_adjustment_cents", "PhilHealth adjustment"),
    signed("pagibig_adjustment_cents", "Pag-IBIG adjustment"),
];
const AMENDMENT: &[Field] = &[
    field("basic_cents", "Additional basic pay"),
    field("gross_cents", "Additional gross pay"),
    field("taxable_cents", "Additional taxable pay"),
    signed("contribution_cents", "Employee contribution adjustment"),
];
const FINAL: &[Field] = &[
    field("vacation_gross_cents", "Leave conversion gross"),
    field("vacation_exempt_cents", "Exempt part of leave conversion"),
    field(
        "vacation_shared_benefit_cents",
        "Leave conversion in shared benefit cap",
    ),
    field("separation_cents", "Separation pay"),
    field("retirement_cents", "Retirement pay"),
    field("taxable_extras_cents", "Taxable extra pay"),
    field("shared_extras_cents", "Extra pay in shared benefit cap"),
    field("exempt_extras_cents", "Exempt extra pay"),
    field("deposit_returns_cents", "Returned deposits (not income)"),
    signed(
        "additional_contributions_cents",
        "Additional employee contributions",
    ),
    field(
        "extras_gross_cents",
        "Additional settlement total (includes deposits)",
    ),
];
pub fn fields(kind: Kind, state: State) -> &'static [Field] {
    if state == State::ReviewOnly {
        return &[];
    }
    match kind {
        Kind::Monthly | Kind::Cutoff => REGULAR,
        Kind::Thirteenth => THIRTEENTH,
        Kind::Amendment => AMENDMENT,
        Kind::FinalAdjustment => FINAL,
        Kind::AnnualTax | Kind::FinalContribution => &[],
    }
}
pub fn scope_note(kind: Kind, state: State) -> &'static str {
    if state == State::ReviewOnly {
        return "Review only: no income, charge or recovery created.";
    }
    match kind {
        Kind::Monthly | Kind::Cutoff => {
            "Issued pay only. Exempt allowance is part of allowances; reimbursement is outside gross."
        }
        Kind::Thirteenth => {
            "Current payout only. Annual earned basic, entitlement and earlier payouts are not additional income here."
        }
        Kind::Amendment => {
            "Linked additional amounts only; original pay remains on its original source."
        }
        Kind::FinalAdjustment => {
            "Additional settlement components only. Original contained pay is excluded. Returned deposits are not income."
        }
        Kind::AnnualTax => {
            "Tax settlement only. Underlying annual pay is not counted again as additional income."
        }
        Kind::FinalContribution => "Contribution allocation is outside this annual BIR view.",
    }
}
impl Report {
    pub fn from_wire(wire: Wire, company: Uuid, year: i32) -> Result<Self> {
        ensure!(
            wire.version == "government-annual-pay-v1" && wire.details.len() <= annual::MAX_ENTRIES,
            "Recorded pay response is unsupported or exceeds the review limit."
        );
        let ledger = annual::Ledger::from_wire(wire.ledger, company, year)?;
        ensure!(
            wire.details.len() == ledger.wire.entries.len(),
            "Recorded pay details do not cover the exact annual source set."
        );
        let mut indices = HashMap::new();
        let parents: HashMap<_, _> = ledger
            .wire
            .entries
            .iter()
            .map(|e| ((e.fact.kind, e.fact.id), &e.fact))
            .collect();
        for (i, d) in wire.details.iter().enumerate() {
            let f = parents
                .get(&(d.kind, d.id))
                .context("Recorded pay detail has no annual source.")?;
            ensure!(
                d.company_id == company
                    && d.employee_id == f.employee_id
                    && indices.insert((d.kind, d.id), i).is_none(),
                "Recorded pay detail has a foreign or duplicate source binding."
            );
            let expected = fields(d.kind, f.state);
            ensure!(
                d.values.len() == expected.len()
                    && expected.iter().all(|k| d.values.contains_key(k.key)),
                "Recorded pay contains unexpected or missing component keys."
            );
            for k in expected {
                if let Some(n) = d.values[k.key] {
                    ensure!(
                        d.source_found && (-LIMIT..=LIMIT).contains(&n) && (k.signed || n >= 0),
                        "Recorded pay has an invalid amount, sign or missing-source value."
                    );
                }
            }
        }
        Ok(Self {
            ledger,
            details: wire.details,
            indices,
        })
    }
    pub fn detail(&self, kind: Kind, id: Uuid) -> &Detail {
        &self.details[self.indices[&(kind, id)]]
    }
    pub fn export(&self, path: &Path) -> Result<Vec<u8>> {
        let w = &self.ledger.wire;
        let mut headers: Vec<String> = [
            "Worksheet",
            "Company",
            "Receipt year",
            "As of",
            "Generated UTC",
            "Employee code",
            "Employee name",
            "Source kind",
            "Source ID",
            "Service from",
            "Service to",
            "Actual receipt",
            "State",
            "Recorded withholding PHP",
            "Received-in-year withholding PHP",
            "Treatment",
            "Issued source found",
            "Component scope",
            "Component cautions",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let groups = [
            ("Regular", REGULAR),
            ("13th-month", THIRTEENTH),
            ("Amendment", AMENDMENT),
            ("Final additional", FINAL),
        ];
        for (prefix, fs) in groups {
            for f in fs {
                headers.push(format!("{prefix}: {} PHP", f.label));
            }
        }
        let context = vec![
            "INTERNAL RECORDED PAY; NOT A BIR RETURN OR EXPECTED TAX".into(),
            w.company_name.clone(),
            w.year.to_string(),
            w.as_of.to_string(),
            w.generated_at.to_rfc3339(),
        ];
        let mut rows = vec![];
        for e in &w.entries {
            let f = &e.fact;
            let d = self.detail(f.kind, f.id);
            let p = self.ledger.employee(f.employee_id);
            let amount = || {
                f.parts
                    .employee_cents
                    .map(decimal)
                    .unwrap_or_else(|| "Unknown".into())
            };
            let mut row = context.clone();
            row.extend([
                p.employee_code.clone(),
                p.employee_name.clone(),
                f.kind.label().into(),
                f.id.to_string(),
                f.starts_on.to_string(),
                f.ends_on.to_string(),
                f.received_on.map(|d| d.to_string()).unwrap_or_default(),
                f.state.label().into(),
                amount(),
                if f.state == State::Received {
                    amount()
                } else {
                    String::new()
                },
                e.treatment().into(),
                if d.source_found {
                    "Yes"
                } else {
                    "No; amounts unresolved"
                }
                .into(),
                scope_note(f.kind, f.state).into(),
                NOTE.into(),
            ]);
            for (prefix, fs) in groups {
                let applies = f.state != State::ReviewOnly
                    && match prefix {
                        "Regular" => matches!(f.kind, Kind::Monthly | Kind::Cutoff),
                        "13th-month" => f.kind == Kind::Thirteenth,
                        "Amendment" => f.kind == Kind::Amendment,
                        _ => f.kind == Kind::FinalAdjustment,
                    };
                row.extend(fs.iter().map(|k| {
                    if applies {
                        d.values[k.key]
                            .map(decimal)
                            .unwrap_or_else(|| "Unknown".into())
                    } else {
                        String::new()
                    }
                }));
            }
            rows.push(row);
        }
        if rows.is_empty() {
            let mut row = context;
            row.resize(headers.len(), String::new());
            row[15] =
                "No recorded sources observed; not a zero-income or zero-tax declaration".into();
            row[18] = NOTE.into();
            rows.push(row);
        }
        super::export::Table { headers, rows }.render(path)
    }
}

use anyhow::Context;
#[cfg(any(test, feature = "qa-screenshot"))]
pub(crate) fn fixture() -> Report {
    let mut ledger = annual::fixture().wire;
    for (i, kind) in [
        Kind::Monthly,
        Kind::AnnualTax,
        Kind::Thirteenth,
        Kind::Cutoff,
        Kind::FinalAdjustment,
        Kind::Amendment,
        Kind::Amendment,
    ]
    .into_iter()
    .enumerate()
    {
        ledger.entries[i].fact.kind = kind;
    }
    let details = ledger
        .entries
        .iter()
        .map(|e| Detail {
            company_id: e.fact.company_id,
            employee_id: e.fact.employee_id,
            kind: e.fact.kind,
            id: e.fact.id,
            source_found: true,
            values: fields(e.fact.kind, e.fact.state)
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    (
                        f.key.into(),
                        Some(if f.signed { -12345 } else { 123456 + i as i64 }),
                    )
                })
                .collect(),
        })
        .collect();
    Report::from_wire(
        Wire {
            version: "government-annual-pay-v1".into(),
            ledger,
            details,
        },
        Uuid::from_u128(1),
        2026,
    )
    .unwrap()
}
#[cfg(test)]
mod tests;
