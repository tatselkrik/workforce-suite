//! One literal-data contract for internal CSV and Excel review worksheets.
//! Amounts remain decimal text; no floating-point conversion or spreadsheet
//! formula is used to recompute immutable payroll or government receipt facts.
use anyhow::{Result, ensure};
use std::path::Path;

pub(crate) struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.headers.is_empty()
                && self.headers.len() <= 64
                && self.rows.len() <= 50_000
                && (self.rows.len() + 1) * self.headers.len() <= 2_000_000
                && self.rows.iter().all(|r| r.len() == self.headers.len()),
            "Government worksheet exceeds its row/column limits or has mismatched columns. Narrow the report."
        );
        Ok(())
    }

    pub fn csv(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut out = String::from("\u{feff}");
        for row in std::iter::once(&self.headers).chain(self.rows.iter()) {
            out.push_str(
                &row.iter()
                    .map(|v| crate::reports::csv_cell(v))
                    .collect::<Result<Vec<_>>>()?
                    .join(","),
            );
            out.push_str("\r\n");
        }
        Ok(out.into_bytes())
    }

    pub fn xlsx(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let headers: Vec<_> = self.headers.iter().map(String::as_str).collect();
        crate::tabular::text_xlsx(&headers, &self.rows)
    }

    pub fn render(&self, path: &Path) -> Result<Vec<u8>> {
        if crate::tabular::is_xlsx(path) {
            self.xlsx()
        } else {
            ensure!(
                path.extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("csv")),
                "Choose a .csv or .xlsx filename for an internal government worksheet."
            );
            self.csv()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payroll_government::{self as g, preparation as p};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn fixture(agency: g::Agency) -> p::Preparation {
        let mut wire = g::preview().wire;
        wire.source.agency = agency;
        wire.source.company_name = "=Fictional, Café 海岸".into();
        wire.reviews.clear();
        wire.history.clear();
        for f in wire
            .source
            .facts
            .iter_mut()
            .chain(wire.effective_facts.iter_mut())
        {
            f.agency = agency;
            if agency != g::Agency::Sss {
                f.parts.ec_cents = Some(0);
            }
            if agency == g::Agency::Bir {
                f.parts.employer_cents = Some(0);
                f.contribution_month = None;
                f.received_on = Some("2026-09-30".parse().unwrap());
            }
        }
        for r in &mut wire.remittances {
            r.agency = agency;
        }
        wire.employees[0].employee_code = "000001".into();
        wire.employees[0].employee_name = "=Example, Ángela 李".into();
        let cid = wire.source.company_id;
        let identities = std::iter::once(None)
            .chain(wire.employees.iter().map(|e| Some(e.employee_id)))
            .enumerate()
            .map(|(i, employee_id)| {
                let mut details: BTreeMap<String, String> = p::keys(agency, employee_id.is_some())
                    .iter()
                    .map(|k| ((*k).into(), String::new()))
                    .collect();
                let name = if employee_id.is_some() {
                    "last_name"
                } else {
                    "registered_name"
                };
                details.insert(name.into(), "=Fictional, Café 海岸".into());
                if employee_id.is_some() {
                    details.insert("first_name".into(), "Ángela".into());
                }
                for (key, value) in [
                    (
                        "sss_number",
                        if employee_id.is_some() {
                            "0212345678"
                        } else {
                            "0123456789"
                        },
                    ),
                    ("philhealth_number", "012345678901"),
                    ("pagibig_number", "012345678901"),
                    ("bir_tin", "012345678"),
                    ("bir_branch", "0000"),
                    ("birth_date", "1990-01-02"),
                    ("sex", "F"),
                    ("nationality", "PH"),
                ] {
                    if details.contains_key(key) {
                        details.insert(key.into(), value.into());
                    }
                }
                p::Identity {
                    id: Uuid::from_u128(590001 + i as u128),
                    company_id: cid,
                    employee_id,
                    revision: 7,
                    details,
                }
            })
            .collect();
        let filter = g::Filter {
            month: wire.source.month,
            agency,
        };
        p::Preparation::from_wire(
            p::PreparationWire {
                version: "government-preparation-v1".into(),
                review: wire,
                registrations: identities,
            },
            cid,
            filter,
        )
        .unwrap()
    }

    fn workbook_rows(bytes: &[u8]) -> Vec<Vec<String>> {
        crate::tabular::read_xlsx(bytes, 100, 64)
            .unwrap()
            .into_iter()
            .map(|r| r.cells)
            .collect()
    }
    fn column(rows: &[Vec<String>], name: &str) -> usize {
        rows[0].iter().position(|v| v == name).unwrap()
    }

    #[test]
    fn four_agencies_export_literal_identifiers_and_exact_money_without_extra_private_fields() {
        for agency in [
            g::Agency::Bir,
            g::Agency::Sss,
            g::Agency::Philhealth,
            g::Agency::Pagibig,
        ] {
            let p = fixture(agency);
            let rows = workbook_rows(&p.xlsx().unwrap());
            assert_eq!(rows.len(), p.report.wire.effective_facts.len() + 2);
            assert_eq!(rows[1][column(&rows, "employee_code_current")], "000001");
            assert_eq!(rows[1][column(&rows, "known_employee_php")], "1500.00");
            assert_eq!(
                rows[1][column(&rows, "employer_registered_name")],
                "=Fictional, Café 海岸"
            );
            let key = match agency {
                g::Agency::Bir => "bir_tin",
                g::Agency::Sss => "sss_number",
                g::Agency::Philhealth => "philhealth_number",
                g::Agency::Pagibig => "pagibig_number",
            };
            assert!(rows[1][column(&rows, &format!("employer_{key}"))].starts_with('0'));
            for other in [
                "bir_tin",
                "sss_number",
                "philhealth_number",
                "pagibig_number",
            ] {
                if other != key {
                    assert!(!rows[0].iter().any(|s| s.contains(other)));
                }
            }
            let all = rows
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            for secret in [
                "FICTIONAL-SSS-RECEIPT",
                "Fictional bank receipt",
                "Fictional HR preparer",
                "Fictional coverage evidence",
                "recorded_by",
                "contributors",
                "void_reason",
            ] {
                assert!(!all.contains(secret), "{secret}");
            }
        }
    }

    #[test]
    fn reconciliation_workbook_keeps_month_allocation_and_unknown_source_totals_distinct() {
        let p = fixture(g::Agency::Sss);
        let rows = workbook_rows(&p.report.xlsx().unwrap());
        let t = column(&rows, "record_type");
        let summary = rows.iter().skip(1).find(|r| r[t] == "summary").unwrap();
        assert_eq!(summary[column(&rows, "recorded_remitted_php")], "4600.00");
        assert_eq!(
            summary[column(&rows, "complete_liability_php")],
            "UNRESOLVED"
        );
        let special = rows
            .iter()
            .skip(1)
            .find(|r| r[column(&rows, "source")] == "13th-month payout")
            .unwrap();
        assert_eq!(special[column(&rows, "included_in_liability")], "false");
        assert_eq!(special[column(&rows, "known_total_php")], "UNKNOWN");
        let receipt = rows.iter().skip(1).find(|r| r[t] == "remittance").unwrap();
        assert_eq!(receipt[column(&rows, "recorded_remitted_php")], "4600.00");
        assert!(!receipt.iter().any(|s| s == "5000.00"));
    }

    #[test]
    fn csv_retains_its_formula_prefix_while_excel_keeps_the_literal_original() {
        let p = fixture(g::Agency::Sss);
        let csv = String::from_utf8(p.csv().unwrap()).unwrap();
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains("'=Fictional, Café 海岸"));
        assert!(csv.ends_with("\r\n"));
        let rows = workbook_rows(&p.xlsx().unwrap());
        assert_eq!(
            rows[1][column(&rows, "employer_registered_name")],
            "=Fictional, Café 海岸"
        );
        assert_eq!(
            rows[1][column(&rows, "employee_name_current")],
            "=Example, Ángela 李"
        );
    }

    #[test]
    fn exporting_recomputes_summary_and_rejects_tampered_registration_scope() {
        let mut p = fixture(g::Agency::Sss);
        p.report.summary.known_parts.employee_cents = Some(999999999);
        p.report.summary.complete = true;
        let rows = workbook_rows(&p.xlsx().unwrap());
        let sum = rows.last().unwrap();
        assert_eq!(sum[column(&rows, "complete_liability_php")], "UNRESOLVED");
        assert_eq!(sum[column(&rows, "known_employee_php")], "1500.00");
        let rows = workbook_rows(&p.report.xlsx().unwrap());
        assert_eq!(
            rows[1][column(&rows, "complete_liability_php")],
            "UNRESOLVED"
        );
        p.registrations[0].company_id = Uuid::new_v4();
        assert!(p.xlsx().is_err());
        assert!(p.csv().is_err());
    }

    #[test]
    fn omitted_identity_remains_flagged_instead_of_becoming_invented_registration() {
        let mut p = fixture(g::Agency::Philhealth);
        p.registrations.pop();
        let rows = workbook_rows(&p.xlsx().unwrap());
        let row = &rows[2];
        assert_eq!(row[column(&rows, "registered_philhealth_number")], "");
        assert!(
            row[column(&rows, "preparation_review_flags")]
                .contains("missing employee philhealth_number")
        );
        assert!(row[column(&rows, "preparation_review_flags")].contains("source excluded"));
    }

    #[test]
    fn postdated_receipts_remain_dated_and_excluded_from_bir_received_liability() {
        let mut p = fixture(g::Agency::Bir);
        for f in [
            &mut p.report.wire.source.facts[0],
            &mut p.report.wire.effective_facts[0],
        ] {
            f.received_on = Some("2026-10-26".parse().unwrap());
            f.state = g::State::DatedAfterAsOf;
        }
        let rows = workbook_rows(&p.report.xlsx().unwrap());
        let row = rows
            .iter()
            .skip(1)
            .find(|r| r[column(&rows, "source")] == "Monthly salary")
            .unwrap();
        assert_eq!(row[column(&rows, "received_on")], "2026-10-26");
        assert_eq!(row[column(&rows, "included_in_liability")], "false");
        assert_eq!(
            row[column(&rows, "status")],
            "Receipt dated after report date"
        );
    }

    #[test]
    fn worksheet_format_dispatch_preserves_existing_files_and_rejects_unsupported_extensions() {
        let p = fixture(g::Agency::Sss);
        assert!(p.export(Path::new("government.xls")).is_err());
        assert!(p.report.export(Path::new("government.xlsx.exe")).is_err());
        assert!(
            p.export(Path::new("government.XLSX"))
                .unwrap()
                .starts_with(b"PK")
        );
        assert!(
            p.export(Path::new("government.CSV"))
                .unwrap()
                .starts_with(b"\xef\xbb\xbf")
        );
        let dir = std::env::temp_dir().join(format!("hris-government-excel-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("review.xlsx");
        let bytes = p.xlsx().unwrap();
        crate::reports::save_export(&file, &bytes).unwrap();
        assert!(crate::reports::save_export(&file, b"replacement").is_err());
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn oversized_or_invalid_worksheets_fail_without_partial_truncation() {
        let mut t = Table {
            headers: vec!["literal".into()],
            rows: vec![vec!["value".into()]; 50_001],
        };
        assert!(t.xlsx().is_err());
        assert!(t.csv().is_err());
        t.rows = vec![vec!["invalid\u{000b}text".into()]];
        assert!(t.xlsx().is_err());
        t.rows = vec![vec!["x".repeat(32768)]];
        assert!(t.xlsx().is_err());
        t.rows = vec![Vec::new()];
        assert!(t.xlsx().is_err());
        assert!(t.csv().is_err());
    }

    #[test]
    fn negative_linked_centavos_keep_their_sign_and_are_not_rounded_or_absolutized() {
        let mut p = fixture(g::Agency::Sss);
        for f in [
            &mut p.report.wire.source.facts[0],
            &mut p.report.wire.effective_facts[0],
        ] {
            f.kind = g::Kind::Amendment;
            f.parts = g::Parts {
                employee_cents: Some(-1),
                employer_cents: Some(-2),
                ec_cents: Some(0),
            };
        }
        let rows = workbook_rows(&p.xlsx().unwrap());
        assert_eq!(rows[1][column(&rows, "known_employee_php")], "-0.01");
        assert_eq!(rows[1][column(&rows, "known_employer_php")], "-0.02");
        assert_eq!(rows[1][column(&rows, "known_source_total_php")], "-0.03");
        let csv = String::from_utf8(p.csv().unwrap()).unwrap();
        assert!(csv.contains("'-0.01"));
        assert!(csv.contains("'-0.03"));
    }

    #[test]
    fn voided_receipts_stay_visible_and_never_reduce_or_rewrite_source_liability() {
        let mut p = fixture(g::Agency::Sss);
        let r = &mut p.report.wire.remittances[0];
        r.void_id = Some(Uuid::from_u128(599001));
        r.void_reason = Some("Private void reason not exported".into());
        r.voided_by = Some(Uuid::from_u128(599002));
        r.voided_at = Some(p.report.wire.source.generated_at);
        let rows = workbook_rows(&p.report.xlsx().unwrap());
        let sum = &rows[1];
        assert_eq!(sum[column(&rows, "known_employee_php")], "1500.00");
        assert_eq!(sum[column(&rows, "recorded_remitted_php")], "0.00");
        let receipt = rows
            .iter()
            .skip(1)
            .find(|r| r[column(&rows, "record_type")] == "remittance")
            .unwrap();
        assert_eq!(receipt[column(&rows, "status")], "Voided");
        assert_eq!(receipt[column(&rows, "recorded_remitted_php")], "0.00");
        assert!(
            !rows
                .iter()
                .flatten()
                .any(|s| s.contains("Private void reason"))
        );
    }

    #[test]
    fn mutated_wire_scope_effective_values_or_other_agency_fields_block_export() {
        for case in 0..4 {
            let mut p = fixture(g::Agency::Sss);
            match case {
                0 => p.report.wire.source.company_id = Uuid::new_v4(),
                1 => p.report.wire.effective_facts[0].parts.employee_cents = Some(17),
                2 => {
                    p.registrations[0]
                        .details
                        .insert("bir_tin".into(), "012345678".into());
                }
                3 => p
                    .report
                    .wire
                    .employees
                    .push(p.report.wire.employees[0].clone()),
                _ => unreachable!(),
            }
            assert!(p.xlsx().is_err(), "case {case}");
            assert!(p.csv().is_err(), "case {case}");
        }
    }

    #[test]
    #[ignore = "Explicit fictional XLSX/CSV evidence export to a new private folder."]
    fn write_eight_fictional_government_workbooks() {
        let dir = std::path::PathBuf::from(std::env::var("HRIS_GOVERNMENT_EXCEL_OUT").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let mut vectors = Vec::new();
        for agency in [
            g::Agency::Bir,
            g::Agency::Sss,
            g::Agency::Philhealth,
            g::Agency::Pagibig,
        ] {
            let p = fixture(agency);
            for (name, excel, csv) in [
                ("preparation", p.xlsx().unwrap(), p.csv().unwrap()),
                ("review", p.report.xlsx().unwrap(), p.report.csv().unwrap()),
            ] {
                let base = format!("{}-{name}", agency.label());
                crate::reports::save_export(&dir.join(format!("{base}.xlsx")), &excel).unwrap();
                crate::reports::save_export(&dir.join(format!("{base}.csv")), &csv).unwrap();
            }
            vectors.push(p::PreparationWire {
                version: "government-preparation-v1".into(),
                review: p.report.wire,
                registrations: p.registrations,
            });
        }
        std::fs::write(
            dir.join("fictional-vectors.json"),
            serde_json::to_vec_pretty(&vectors).unwrap(),
        )
        .unwrap();
    }

    #[test]
    #[ignore = "Decode actual raw protected preparation RPC captures and render native Excel/CSV."]
    fn raw_protected_government_workbook_vectors() {
        let path = std::env::var("HRIS_GOVERNMENT_EXCEL_VECTORS").unwrap();
        let dir = std::path::PathBuf::from(std::env::var("HRIS_GOVERNMENT_EXCEL_OUT").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let vectors: Vec<p::PreparationWire> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!((4..=16).contains(&vectors.len()));
        for (i, w) in vectors.into_iter().enumerate() {
            let filter = g::Filter {
                agency: w.review.source.agency,
                month: w.review.source.month,
            };
            let cid = w.review.source.company_id;
            let p = p::Preparation::from_wire(w, cid, filter).unwrap();
            let base = format!("{i}-{}", filter.agency.label());
            for (name, excel, csv) in [
                ("preparation", p.xlsx().unwrap(), p.csv().unwrap()),
                ("review", p.report.xlsx().unwrap(), p.report.csv().unwrap()),
            ] {
                crate::reports::save_export(&dir.join(format!("{base}-{name}.xlsx")), &excel)
                    .unwrap();
                crate::reports::save_export(&dir.join(format!("{base}-{name}.csv")), &csv).unwrap();
            }
        }
    }
}
