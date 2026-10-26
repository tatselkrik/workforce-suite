//! Agency-specific internal worksheets. Never an agency upload or official form.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub revision: i64,
    pub details: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationWire {
    pub version: String,
    pub review: ReviewWire,
    pub registrations: Vec<Identity>,
}
#[derive(Clone, Debug)]
pub struct Preparation {
    pub report: Reconciliation,
    pub registrations: Vec<Identity>,
}
pub fn keys(agency: Agency, person: bool) -> Vec<&'static str> {
    let mut fields = if person {
        vec!["last_name", "first_name", "middle_name", "name_suffix"]
    } else {
        vec!["registered_name"]
    };
    fields.extend(match agency {
        Agency::Bir => vec!["bir_tin", "bir_branch"],
        Agency::Sss => vec!["sss_number"],
        Agency::Philhealth => vec!["philhealth_number"],
        Agency::Pagibig => vec!["pagibig_number"],
    });
    if person {
        match agency {
            Agency::Philhealth => fields.extend(["birth_date", "sex"]),
            Agency::Bir => fields.push("nationality"),
            _ => {}
        }
    }
    fields
}
impl Preparation {
    pub fn from_wire(wire: PreparationWire, cid: Uuid, filter: Filter) -> Result<Self> {
        ensure!(
            wire.version == "government-preparation-v1" && wire.registrations.len() <= 20001,
            "Unsupported preparation response version/size."
        );
        let report = Reconciliation::from_wire(wire.review, cid, filter)?;
        let mut records = HashSet::new();
        let mut targets = HashSet::new();
        for r in &wire.registrations {
            ensure!(
                !r.id.is_nil()
                    && r.company_id == cid
                    && records.insert(r.id)
                    && targets.insert(r.employee_id)
                    && (0..=100_000_000_000_000).contains(&r.revision)
                    && r.employee_id.is_none_or(|id| report.employee(id).is_some()),
                "Preparation identity is duplicated or outside the selected payroll scope."
            );
            let fields = keys(filter.agency, r.employee_id.is_some());
            ensure!(
                r.details.len() == fields.len()
                    && fields.iter().all(|k| r.details.contains_key(*k)),
                "Preparation contains fields outside the requested agency."
            );
            let normalized = registration::normalize(
                &r.details,
                r.employee_id.is_some(),
                report.wire.source.as_of,
            )?;
            ensure!(
                r.details.iter().all(|(k, v)| normalized.get(k) == Some(v)),
                "Preparation identifiers/text are not canonical."
            );
        }
        Ok(Self {
            report,
            registrations: wire.registrations,
        })
    }
    fn identity(&self, employee: Option<Uuid>) -> Option<&Identity> {
        self.registrations
            .iter()
            .find(|r| r.employee_id == employee)
    }
    /// Contains only the selected agency's legal fields. Supporting references,
    /// HR notes, actor IDs and other agencies' identifiers are never projected.
    pub fn csv(&self) -> Result<Vec<u8>> {
        self.table()?.csv()
    }
    pub fn xlsx(&self) -> Result<Vec<u8>> {
        self.table()?.xlsx()
    }
    pub fn export(&self, path: &std::path::Path) -> Result<Vec<u8>> {
        self.table()?.render(path)
    }
    fn table(&self) -> Result<super::export::Table> {
        let checked = Self::from_wire(
            PreparationWire {
                version: "government-preparation-v1".into(),
                review: self.report.wire.clone(),
                registrations: self.registrations.clone(),
            },
            self.report.wire.source.company_id,
            self.report.filter(),
        )?;
        checked.checked_table()
    }
    fn checked_table(&self) -> Result<super::export::Table> {
        let s = &self.report.wire.source;
        let company_keys = keys(s.agency, false);
        let person_keys = keys(s.agency, true);
        let mut headers: Vec<String> = [
            "worksheet_kind",
            "agency",
            "report_month",
            "generated_utc",
            "as_of",
            "company_display_name",
            "company_registration_revision",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        headers.extend(company_keys.iter().map(|k| format!("employer_{k}")));
        headers.extend(
            [
                "row_type",
                "source_record_id",
                "source_kind",
                "period_start",
                "period_end",
                "received_on",
                "source_state",
                "included_in_liability",
                "employee_code_current",
                "employee_name_current",
                "employee_registration_revision",
            ]
            .map(str::to_owned),
        );
        headers.extend(person_keys.iter().map(|k| format!("registered_{k}")));
        headers.extend(
            [
                "known_employee_php",
                "known_employer_php",
                "known_ec_php",
                "known_source_total_php",
                "complete_liability_php",
                "recorded_remitted_php",
                "difference_php",
                "preparation_review_flags",
            ]
            .map(str::to_owned),
        );
        let mut rows = Vec::new();
        let employer = self.identity(None);
        let prefix = || {
            let mut row = vec![
                "INTERNAL_AGENCY_PREPARATION_NOT_AN_UPLOAD_FILE".into(),
                s.agency.label().into(),
                s.month.to_string(),
                s.generated_at.to_rfc3339(),
                s.as_of.to_string(),
                s.company_name.clone(),
                employer.map(|r| r.revision.to_string()).unwrap_or_default(),
            ];
            row.extend(company_keys.iter().map(|k| {
                employer
                    .and_then(|r| r.details.get(*k))
                    .cloned()
                    .unwrap_or_default()
            }));
            row
        };
        let amount = |n: Option<i64>| n.map(decimal).unwrap_or("UNRESOLVED".into());
        let mut facts: Vec<_> = self.report.wire.effective_facts.iter().collect();
        facts.sort_by_key(|f| (f.employee_id, f.starts_on, f.id));
        for f in facts {
            let p = self.identity(Some(f.employee_id));
            let label = self.report.employee(f.employee_id);
            let included = self.report.included(f);
            let mut flags = Vec::new();
            for k in &company_keys {
                if employer
                    .and_then(|r| r.details.get(*k))
                    .is_none_or(|v| v.is_empty())
                {
                    flags.push(format!("missing employer {k}"));
                }
            }
            for k in &person_keys {
                if !matches!(*k, "middle_name" | "name_suffix")
                    && p.and_then(|r| r.details.get(*k))
                        .is_none_or(|v| v.is_empty())
                {
                    flags.push(format!("missing employee {k}"));
                }
            }
            if !included {
                flags.push("source excluded from selected liability".into());
            }
            if f.parts.total().is_none() {
                flags.push("source amounts unresolved".into());
            }
            if !self.report.summary.complete {
                flags.push("selected liability unresolved".into());
            }
            flags.push(
                "internal worksheet; agency-specific declarations/format validation still required"
                    .into(),
            );
            let mut row = prefix();
            row.extend([
                "SOURCE".into(),
                f.id.to_string(),
                f.kind.label().into(),
                f.starts_on.to_string(),
                f.ends_on.to_string(),
                f.received_on.map(|d| d.to_string()).unwrap_or_default(),
                f.state.label().into(),
                if included { "YES" } else { "NO" }.into(),
                label.map(|e| e.employee_code.clone()).unwrap_or_default(),
                label.map(|e| e.employee_name.clone()).unwrap_or_default(),
                p.map(|r| r.revision.to_string()).unwrap_or_default(),
            ]);
            row.extend(person_keys.iter().map(|k| {
                p.and_then(|r| r.details.get(*k))
                    .cloned()
                    .unwrap_or_default()
            }));
            row.extend([
                amount(f.parts.employee_cents),
                amount(f.parts.employer_cents),
                amount(f.parts.ec_cents),
                amount(f.parts.total()),
                String::new(),
                String::new(),
                String::new(),
                flags.join("; "),
            ]);
            rows.push(row);
        }
        let sum = &self.report.summary;
        let mut row = prefix();
        row.extend([
            "SUMMARY".into(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ]);
        row.extend(person_keys.iter().map(|_| String::new()));
        row.extend([amount(sum.known_parts.employee_cents), amount(sum.known_parts.employer_cents), amount(sum.known_parts.ec_cents),
            amount(sum.known_parts.total()), if sum.complete {amount(sum.known_parts.total())}else{"UNRESOLVED".into()},
            decimal(sum.remitted_cents), amount(sum.difference_cents),
            format!("INTERNAL REVIEW ONLY; {} unallocated; {} missing receipts; {} postdated; {} negative person balances; other agency declarations not inferred",
                sum.unallocated_count,sum.missing_receipt_count,sum.dated_after_as_of_count,sum.negative_balance_count)]);
        rows.push(row);
        Ok(super::export::Table { headers, rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PreparationWire {
        let report = super::super::preview();
        let cid = report.wire.source.company_id;
        let person = report.wire.employees[0].employee_id;
        let identities = [(None, "=Fictional Employer"), (Some(person), "Fictional")]
            .into_iter()
            .enumerate()
            .map(|(i, (employee_id, name))| {
                let mut details: BTreeMap<_, _> = keys(Agency::Sss, employee_id.is_some())
                    .into_iter()
                    .map(|k| (k.into(), String::new()))
                    .collect();
                details.insert(
                    if employee_id.is_some() {
                        "last_name"
                    } else {
                        "registered_name"
                    }
                    .into(),
                    name.into(),
                );
                details.insert(
                    "sss_number".into(),
                    if employee_id.is_some() {
                        "0212345678"
                    } else {
                        "0123456789"
                    }
                    .into(),
                );
                if employee_id.is_some() {
                    details.insert("first_name".into(), "Sample".into());
                }
                Identity {
                    id: Uuid::from_u128(190001 + i as u128),
                    company_id: cid,
                    employee_id,
                    revision: 0,
                    details,
                }
            })
            .collect();
        PreparationWire {
            version: "government-preparation-v1".into(),
            review: report.wire,
            registrations: identities,
        }
    }
    #[test]
    fn preparation_is_purpose_limited_formula_safe_and_visibly_unresolved() {
        let wire = fixture();
        let cid = wire.review.source.company_id;
        let filter = Filter {
            agency: Agency::Sss,
            month: wire.review.source.month,
        };
        let p = Preparation::from_wire(wire, cid, filter).unwrap();
        let bytes = p.csv().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with('\u{feff}'));
        assert!(text.contains("INTERNAL_AGENCY_PREPARATION_NOT_AN_UPLOAD_FILE"));
        assert!(text.contains("'=Fictional Employer"));
        assert!(text.contains("registered_sss_number"));
        for private in [
            "PRIVATE_REGISTRATION_NOTE",
            "reference",
            "evidence",
            "birth_date",
            "philhealth_number",
            "pagibig_number",
            "bir_tin",
            "actor_id",
        ] {
            assert!(!text.contains(private), "{private}");
        }
        assert!(text.contains("UNRESOLVED"));
        assert!(text.contains("missing employee"));
        assert!(text.contains("source excluded"));
        assert_eq!(
            text.lines().count(),
            p.report.wire.effective_facts.len() + 2
        );
    }
    #[test]
    fn foreign_extra_duplicate_and_unselected_agency_identity_fields_fail_closed() {
        let original = fixture();
        let cid = original.review.source.company_id;
        let filter = Filter {
            agency: Agency::Sss,
            month: original.review.source.month,
        };
        for case in 0..5 {
            let mut w = original.clone();
            match case {
                0 => w.registrations[0].company_id = Uuid::new_v4(),
                1 => {
                    w.registrations[0]
                        .details
                        .insert("bir_tin".into(), "123456789".into());
                }
                2 => w.registrations.push(w.registrations[0].clone()),
                3 => w.registrations[1].employee_id = Some(Uuid::new_v4()),
                4 => {
                    w.registrations[1]
                        .details
                        .insert("sss_number".into(), "0000000000".into());
                }
                _ => unreachable!(),
            }
            assert!(Preparation::from_wire(w, cid, filter).is_err(), "{case}");
        }
    }
    #[test]
    #[ignore = "Requires the explicit PostgreSQL preparation vector run."]
    fn four_agency_postgres_projections_decode_with_only_their_registered_fields() {
        let path = std::env::var("HRIS_PREPARATION_VECTORS").expect("Set preparation vectors.");
        let vectors: Vec<PreparationWire> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(vectors.len(), 4);
        for wire in vectors {
            let cid = wire.review.source.company_id;
            let filter = Filter {
                month: wire.review.source.month,
                agency: wire.review.source.agency,
            };
            let p = Preparation::from_wire(wire, cid, filter).unwrap();
            assert!(p.csv().unwrap().starts_with(b"\xef\xbb\xbf"));
        }
    }
}
