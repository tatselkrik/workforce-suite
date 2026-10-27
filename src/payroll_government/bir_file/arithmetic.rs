//! Local relationships between declared compensation fields, not expected tax.
//! The published paper schedule and electronic layout must both support a rule.
use super::{MAX_DETAILS, Schedule, money};
use crate::imports::CsvRow;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

const MAX_FINDINGS: usize = 500;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Review {
    pub rows: usize,
    pub comparisons: usize,
    pub differences: usize,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Finding {
    pub line: usize,
    pub field: String,
    /// One-based electronic columns only; no source names, identifiers or money.
    pub equation: String,
}

impl Review {
    pub fn checked(&self) -> bool {
        self.rows > 0 && self.comparisons > 0
    }
    pub fn omitted(&self) -> usize {
        self.differences.saturating_sub(self.findings.len())
    }
    pub fn summary(&self) -> String {
        if !self.checked() {
            return "Not checked. Recheck the complete source after resolving any structure/control errors; no partial arithmetic result is reported.".into();
        }
        format!(
            "{} declared sums checked across {} detail rows; {} differences; {} further findings omitted. This compares declarations only, without calculating tax, certifying classifications or comparing HRIS payroll.",
            self.comparisons,
            self.rows,
            self.differences,
            self.omitted()
        )
    }
}

struct Rule {
    target: usize,
    terms: &'static [usize],
}

// Zero-based electronic columns, not paper-form box numbers. Do not add the
// ambiguous combined-compensation field or copy unexecuted VFP tax formulas.
const S1: [Rule; 5] = [
    Rule {
        target: 12,
        terms: &[18, 22],
    },
    Rule {
        target: 22,
        terms: &[19, 20, 21],
    },
    Rule {
        target: 25,
        terms: &[31, 35],
    },
    Rule {
        target: 35,
        terms: &[32, 33, 34],
    },
    Rule {
        target: 37,
        terms: &[22, 35],
    },
];
const S2: [Rule; 7] = [
    Rule {
        target: 12,
        terms: &[22, 25],
    },
    Rule {
        target: 22,
        terms: &[13, 14, 15, 16, 17, 18, 19, 20, 21],
    },
    Rule {
        target: 25,
        terms: &[23, 24],
    },
    Rule {
        target: 28,
        terms: &[41, 44],
    },
    // Actual net statutory basic is column59. Day/month/year figures and the
    // days/year factor must never enter this sum or be used to infer eligibility.
    Rule {
        target: 41,
        terms: &[58, 33, 34, 35, 36, 37, 38, 39, 40],
    },
    Rule {
        target: 44,
        terms: &[42, 43],
    },
    Rule {
        target: 46,
        terms: &[25, 44],
    },
];

pub(super) fn inspect(schedule: Schedule, rows: &[CsvRow]) -> Result<Review> {
    let layout = schedule.layout();
    let rules = match schedule {
        Schedule::One => &S1[..],
        Schedule::Two => &S2[..],
    };
    let mut result = Review::default();
    for row in rows.iter().filter(|r| {
        r.cells
            .first()
            .is_some_and(|s| s.trim().eq_ignore_ascii_case(layout.detail))
    }) {
        ensure!(
            row.cells.len() == layout.fields.len() && result.rows < MAX_DETAILS,
            "Recheck the complete BIR source before compensation arithmetic review."
        );
        result.rows += 1;
        for rule in rules {
            let declared = money(row.cells[rule.target].trim())
                .context("A required declared subtotal is invalid; recheck the complete source.")?;
            let mut sum = 0i64;
            for &column in rule.terms {
                sum = sum
                    .checked_add(money(row.cells[column].trim()).context(
                        "A required declared component is invalid; recheck the complete source.",
                    )?)
                    .context("Declared compensation sum exceeds its integer bound.")?;
            }
            result.comparisons += 1;
            if declared != sum {
                result.differences += 1;
                if result.findings.len() < MAX_FINDINGS {
                    result.findings.push(Finding {
                        line: row.line,
                        field: layout.fields[rule.target].into(),
                        equation: format!(
                            "{} column {} = {}",
                            layout.detail,
                            rule.target + 1,
                            rule.terms
                                .iter()
                                .map(|i| format!("column {}", i + 1))
                                .collect::<Vec<_>>()
                                .join(" + ")
                        ),
                    });
                }
            }
        }
    }
    ensure!(
        result.rows > 0,
        "No complete detail rows for compensation arithmetic review."
    );
    Ok(result)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn fictional_source(schedule: Schedule, different: bool) -> Vec<u8> {
    tests::sample_source(schedule, different)
}

#[cfg(feature = "qa-screenshot")]
pub(crate) fn preview(schedule: Schedule, different: bool) -> Review {
    let rules = match schedule {
        Schedule::One => &S1[..],
        Schedule::Two => &S2[..],
    };
    let mut review = Review {
        rows: 2,
        comparisons: rules.len() * 2,
        ..Default::default()
    };
    if different {
        review.differences = 2;
        for line in [2, 3] {
            review.findings.push(Finding {
                line,
                field: "Previous gross compensation".into(),
                equation: format!(
                    "{} column 13 = column {} + column {}",
                    schedule.layout().detail,
                    rules[0].terms[0] + 1,
                    rules[0].terms[1] + 1
                ),
            });
        }
    }
    review
}
