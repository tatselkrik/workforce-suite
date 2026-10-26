//! Current recorded BIR sources grouped by receipt year. Not an expected return,
//! annual tax computation, certificate reconciliation or filing acceptance.
use super::*;
use std::path::Path;

pub const MAX_ENTRIES: usize = 20_000;
pub const MAX_OBSERVATIONS: usize = 40_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub fact: Fact,
    /// Months in which the protected source query observed the same fact.
    /// These are neither payment dates nor an inferred service allocation.
    pub observed_months: Vec<NaiveDate>,
}
impl Entry {
    pub fn treatment(&self) -> &'static str {
        match self.fact.state {
            State::Received => {
                if self.fact.parts.employee_cents.is_some() {
                    "Included: actual receipt in selected year"
                } else {
                    "Receipt in year; amount unknown, total incomplete"
                }
            }
            State::Approved => "Excluded: approval is not an actual receipt",
            State::ReceiptMissing => "Excluded: individual receipt/date missing; year unverified",
            State::DatedAfterAsOf => "Excluded: receipt dated after report date",
            State::Withdrawn => "Excluded: withdrawn",
            State::ReviewOnly => "Excluded: review is not a charge or recovery",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub version: String,
    pub company_id: Uuid,
    pub company_name: String,
    pub generated_at: DateTime<Utc>,
    pub as_of: NaiveDate,
    pub year: i32,
    pub through_month: NaiveDate,
    pub entries: Vec<Entry>,
    pub employees: Vec<EmployeeLabel>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub received: usize,
    pub known_received_cents: i64,
    /// No receipts and unknown amounts are distinct from a recorded zero.
    pub complete_received_cents: Option<i64>,
    pub unknown_received: usize,
    pub unpaid: usize,
    pub missing_receipt: usize,
    pub postdated: usize,
    pub excluded: usize,
}
#[derive(Clone, Debug)]
pub struct Ledger {
    pub wire: Wire,
    pub summary: Summary,
    labels: HashMap<Uuid, usize>,
}
pub fn validate_year(year: i32, today: NaiveDate) -> Result<()> {
    ensure!(
        (2025..=2200.min(today.year())).contains(&year),
        "Choose an elapsed/current year from 2025 onward."
    );
    Ok(())
}
fn label_text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control)
}
impl Ledger {
    pub fn from_wire(wire: Wire, company: Uuid, year: i32) -> Result<Self> {
        validate_year(year, wire.as_of)?;
        ensure!(
            wire.version == "government-annual-ledger-v1"
                && wire.company_id == company
                && !company.is_nil()
                && wire.year == year
                && wire.through_month
                    == NaiveDate::from_ymd_opt(
                        year,
                        if year == wire.as_of.year() {
                            wire.as_of.month()
                        } else {
                            12
                        },
                        1
                    )
                    .unwrap()
                && wire.entries.len() <= MAX_ENTRIES
                && wire.employees.len() <= MAX_ENTRIES
                && label_text(&wire.company_name, 160),
            "Annual recorded sources do not match the requested protected year/company or exceed the review limit."
        );
        let mut ids = HashSet::new();
        let mut people = HashSet::new();
        let mut observations = 0usize;
        let mut summary = Summary::default();
        for entry in &wire.entries {
            let f = &entry.fact;
            super::validate_fact_amounts(f, company, wire.as_of, true)?;
            ensure!(
                f.agency == Agency::Bir
                    && f.kind != Kind::FinalContribution
                    && f.contribution_month.is_none()
                    && ids.insert((f.kind, f.id)),
                "Annual ledger contains a foreign agency, contribution allocation or duplicate source."
            );
            ensure!(
                !entry.observed_months.is_empty()
                    && entry.observed_months.len() <= 12
                    && entry
                        .observed_months
                        .iter()
                        .all(|d| d.year() == year && d.day() == 1 && *d <= wire.through_month)
                    && entry.observed_months.windows(2).all(|d| d[0] < d[1]),
                "Annual source observations are outside the selected year, duplicated or unordered."
            );
            observations += entry.observed_months.len();
            ensure!(
                observations <= MAX_OBSERVATIONS,
                "Annual observations exceed the review limit; no totals were truncated."
            );
            people.insert(f.employee_id);
            match f.state {
                State::Received => {
                    ensure!(
                        f.received_on.is_some_and(|d| d.year() == year
                            && entry.observed_months.contains(&d.with_day(1).unwrap())),
                        "Only an actual receipt in the selected year belongs in its received total."
                    );
                    summary.received += 1;
                    if let Some(n) = f.parts.employee_cents {
                        summary.known_received_cents = add(summary.known_received_cents, n)?;
                    } else {
                        summary.unknown_received += 1;
                    }
                }
                State::Approved => summary.unpaid += 1,
                State::ReceiptMissing => summary.missing_receipt += 1,
                State::DatedAfterAsOf => summary.postdated += 1,
                State::Withdrawn | State::ReviewOnly => summary.excluded += 1,
            }
        }
        if summary.received > 0 && summary.unknown_received == 0 {
            summary.complete_received_cents = Some(summary.known_received_cents);
        }
        let mut labels = HashMap::new();
        for (i, e) in wire.employees.iter().enumerate() {
            ensure!(
                people.contains(&e.employee_id)
                    && labels.insert(e.employee_id, i).is_none()
                    && label_text(&e.employee_code, 200)
                    && label_text(&e.employee_name, 200),
                "Annual employee labels are missing, duplicated or outside the source set."
            );
        }
        ensure!(
            labels.len() == people.len(),
            "Annual sources lack their protected employee labels."
        );
        Ok(Self {
            wire,
            summary,
            labels,
        })
    }
    pub fn employee(&self, id: Uuid) -> &EmployeeLabel {
        &self.wire.employees[self.labels[&id]]
    }
    pub fn export(&self, path: &Path) -> Result<Vec<u8>> {
        let headers = [
            "Worksheet",
            "Company",
            "Receipt year",
            "As of",
            "Observed through month",
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
            "Received-in-year PHP",
            "Treatment",
            "Observed months",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let context = vec![
            "INTERNAL RECORDED SOURCES; NOT A BIR RETURN OR EXPECTED TAX".into(),
            self.wire.company_name.clone(),
            self.wire.year.to_string(),
            self.wire.as_of.to_string(),
            self.wire.through_month.to_string(),
            self.wire.generated_at.to_rfc3339(),
        ];
        let mut rows = vec![];
        for e in &self.wire.entries {
            let f = &e.fact;
            let person = self.employee(f.employee_id);
            let mut row = context.clone();
            row.extend([
                person.employee_code.clone(),
                person.employee_name.clone(),
                f.kind.label().into(),
                f.id.to_string(),
                f.starts_on.to_string(),
                f.ends_on.to_string(),
                f.received_on.map(|d| d.to_string()).unwrap_or_default(),
                f.state.label().into(),
                f.parts
                    .employee_cents
                    .map(decimal)
                    .unwrap_or_else(|| "Unknown".into()),
                if f.state == State::Received {
                    f.parts
                        .employee_cents
                        .map(decimal)
                        .unwrap_or_else(|| "Unknown".into())
                } else {
                    String::new()
                },
                e.treatment().into(),
                e.observed_months
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            ]);
            rows.push(row);
        }
        // Even an empty result retains context, rather than silently implying zero.
        if rows.is_empty() {
            let mut row = context;
            row.resize(18, String::new());
            row[16] = "No recorded sources observed; not a zero-tax declaration".into();
            rows.push(row);
        }
        super::export::Table { headers, rows }.render(path)
    }
}

#[cfg(any(test, feature = "qa-screenshot"))]
pub(crate) fn fixture() -> Ledger {
    let cid = Uuid::from_u128(1);
    let eid = Uuid::from_u128(2);
    let date = |s: &str| s.parse::<NaiveDate>().unwrap();
    let states = [
        State::Received,
        State::Received,
        State::Received,
        State::Approved,
        State::ReceiptMissing,
        State::DatedAfterAsOf,
        State::ReviewOnly,
    ];
    let entries = states
        .into_iter()
        .enumerate()
        .map(|(i, state)| Entry {
            fact: Fact {
                company_id: cid,
                employee_id: eid,
                id: Uuid::from_u128(10 + i as u128),
                agency: Agency::Bir,
                kind: if i == 0 {
                    Kind::Monthly
                } else if i == 1 {
                    Kind::AnnualTax
                } else {
                    Kind::Amendment
                },
                state,
                starts_on: date("2025-12-01"),
                ends_on: date("2025-12-31"),
                contribution_month: None,
                received_on: match state {
                    State::Received => Some(date("2026-01-12")),
                    State::DatedAfterAsOf => Some(date("2026-12-12")),
                    _ => None,
                },
                parts: Parts {
                    employee_cents: Some(match i {
                        0 => 123456,
                        1 => -12345,
                        6 => 0,
                        _ => 500,
                    }),
                    ..Parts::ZERO
                },
            },
            observed_months: vec![date("2026-01-01")],
        })
        .collect();
    Ledger::from_wire(
        Wire {
            version: "government-annual-ledger-v1".into(),
            company_id: cid,
            company_name: "Fictional Harbor Company".into(),
            generated_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            as_of: date("2026-10-26"),
            year: 2026,
            through_month: date("2026-10-01"),
            entries,
            employees: vec![EmployeeLabel {
                employee_id: eid,
                employee_code: "000008".into(),
                employee_name: "Fictional, Ángela 李".into(),
            }],
        },
        cid,
        2026,
    )
    .unwrap()
}

#[cfg(test)]
mod tests;
