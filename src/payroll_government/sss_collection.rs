//! Bind an R3 reference draft to fresh, reconciled source and registration facts.
//! Preparing a local draft never records a remittance or files a collection list.
use super::{
    preparation::{Preparation, PreparationWire},
    sss_r3, *,
};
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) fn save_reference(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    ensure!(
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("txt")),
        "Use a .txt filename for the reference draft."
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context(
            "Choose a new filename in an existing writable folder; existing files are preserved.",
        )?;
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(e).context("The reference draft could not be saved completely.");
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct PersonBasis {
    pub employee_id: Uuid,
    pub employee_code: String,
    pub employee_name: String,
    pub legal: BTreeMap<String, String>,
    pub ss_cents: i64,
    pub ec_cents: i64,
}
#[derive(Clone, Debug)]
pub struct Basis {
    pub company_id: Uuid,
    pub month: NaiveDate,
    pub as_of: NaiveDate,
    pub fingerprint: String,
    pub employer_name: String,
    pub employer_sss: String,
    pub receipt: Receipt,
    pub people: Vec<PersonBasis>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub employee_id: Uuid,
    pub last_name: String,
    pub first_name: String,
    pub middle_initial: String,
    pub remark: sss_r3::Remark,
    pub event_on: Option<NaiveDate>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub expected_fingerprint: String,
    pub receipt_id: Uuid,
    pub employer_name: String,
    pub declarations: Vec<Declaration>,
    pub reviewed: bool,
}

impl Basis {
    pub fn from_preparation(prepared: &Preparation) -> Result<Self> {
        let filter = prepared.report.filter();
        let cid = prepared.report.wire.source.company_id;
        ensure!(
            filter.agency == Agency::Sss,
            "Choose SSS for a collection-list draft."
        );
        // Do not trust a caller-mutated cached Summary or an unchecked identity.
        let p = Preparation::from_wire(
            PreparationWire {
                version: "government-preparation-v1".into(),
                review: prepared.report.wire.clone(),
                registrations: prepared.registrations.clone(),
            },
            cid,
            filter,
        )?;
        let report = &p.report;
        let summary = &report.summary;
        ensure!(
            summary.complete && summary.source_count > 0 && summary.difference_cents == Some(0),
            "Complete and reconcile this month's contribution coverage/shares and actual remittance before preparing R3."
        );
        let receipts: Vec<_> = report
            .wire
            .remittances
            .iter()
            .filter(|r| r.void_id.is_none())
            .collect();
        ensure!(
            receipts.len() == 1,
            "This increment supports one active full-month SSS receipt; split remittances need a separate reviewed allocation."
        );
        let receipt = receipts[0];
        ensure!(
            receipt.allocations.len() == 1
                && receipt.allocations[0].month == filter.month
                && receipt.covered_cents(filter.month) == receipt.amount_cents
                && Some(receipt.amount_cents) == summary.known_parts.total(),
            "Use a single-month receipt matching the full known contribution total."
        );
        ensure!(
            receipt.reference.len() <= 10
                && receipt.reference.bytes().all(|b| (32..=126).contains(&b)),
            "The recorded TR/SBR number must fit the legacy ten-character field. Do not truncate a PRN or bank reference."
        );
        let employer = p
            .registrations
            .iter()
            .find(|r| r.employee_id.is_none())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Record the employer's legal name and assigned SSS identifier first."
                )
            })?;
        ensure!(
            !employer.details["registered_name"].trim().is_empty()
                && !employer.details["sss_number"].is_empty(),
            "Employer registration is incomplete."
        );
        let mut grouped: BTreeMap<Uuid, [i64; 2]> = BTreeMap::new();
        for f in report
            .wire
            .effective_facts
            .iter()
            .filter(|f| report.included(f))
        {
            let parts = f.parts;
            let values = grouped.entry(f.employee_id).or_default();
            values[0] = add(
                values[0],
                add(
                    parts
                        .employee_cents
                        .context("Employee share is unresolved.")?,
                    parts
                        .employer_cents
                        .context("Employer share is unresolved.")?,
                )?,
            )?;
            values[1] = add(values[1], parts.ec_cents.context("EC is unresolved.")?)?;
        }
        ensure!(
            (1..=sss_r3::MAX_PEOPLE).contains(&grouped.len()),
            "Choose a supported employee set."
        );
        let mut people = Vec::with_capacity(grouped.len());
        for (employee_id, [ss_cents, ec_cents]) in grouped {
            ensure!(
                ss_cents >= 0 && ec_cents >= 0 && ss_cents + ec_cents > 0,
                "Zero/no-earnings or negative employee balances require separate evidenced handling; they are not inferred from contributions."
            );
            let registration = p
                .registrations
                .iter()
                .find(|r| r.employee_id == Some(employee_id))
                .ok_or_else(|| {
                    anyhow::anyhow!("A contributing employee lacks recorded legal/SSS details.")
                })?;
            ensure!(
                ["last_name", "first_name", "sss_number"]
                    .iter()
                    .all(|k| !registration.details[*k].trim().is_empty()),
                "A contributing employee's legal name or assigned SSS identifier is missing."
            );
            let label = report
                .employee(employee_id)
                .context("A contributing employee label is missing.")?;
            people.push(PersonBasis {
                employee_id,
                employee_code: label.employee_code.clone(),
                employee_name: label.employee_name.clone(),
                legal: registration.details.clone(),
                ss_cents,
                ec_cents,
            });
        }
        let mut identities = p.registrations.clone();
        identities.sort_by_key(|r| r.id);
        let mut facts = report.wire.effective_facts.clone();
        facts.sort_by_key(|f| f.id);
        let raw = serde_json::to_vec(
            &serde_json::json!({"company_id":cid,"month":filter.month,"as_of":report.wire.source.as_of,
            "facts":facts,"identities":identities,"receipt":receipt}),
        )?;
        let fingerprint = Sha256::digest(raw)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self {
            company_id: cid,
            month: filter.month,
            as_of: report.wire.source.as_of,
            fingerprint,
            employer_name: employer.details["registered_name"].clone(),
            employer_sss: employer.details["sss_number"].clone(),
            receipt: receipt.clone(),
            people,
        })
    }
    pub fn render(&self, request: &Request) -> Result<Vec<u8>> {
        ensure!(
            request.reviewed,
            "Review the recorded receipt, identifiers, file-name representations and explicit employment declarations."
        );
        ensure!(
            request.expected_fingerprint == self.fingerprint
                && request.receipt_id == self.receipt.id,
            "Source or registration facts changed. Refresh this form before exporting; unsaved input remains available."
        );
        ensure!(
            request.declarations.len() == self.people.len(),
            "Declare each contributing employee exactly once."
        );
        let mut seen = HashSet::new();
        let mut people = Vec::with_capacity(self.people.len());
        for d in &request.declarations {
            ensure!(
                seen.insert(d.employee_id),
                "Duplicate employee declaration."
            );
            let p = self
                .people
                .iter()
                .find(|p| p.employee_id == d.employee_id)
                .context("A declaration is outside this source set.")?;
            people.push(sss_r3::Person {
                employee_id: p.employee_id,
                sss_number: p.legal["sss_number"].clone(),
                last_name: d.last_name.clone(),
                first_name: d.first_name.clone(),
                middle_initial: d.middle_initial.clone(),
                ss_cents: p.ss_cents,
                ec_cents: p.ec_cents,
                remark: d.remark,
                event_on: d.event_on,
            });
        }
        sss_r3::Draft {
            company_id: self.company_id,
            month: self.month,
            as_of: self.as_of,
            employer_name: request.employer_name.clone(),
            employer_sss: self.employer_sss.clone(),
            receipt_number: self.receipt.reference.clone(),
            paid_on: self.receipt.paid_on,
            paid_cents: self.receipt.amount_cents,
            people,
        }
        .render_reference()
    }
}

#[cfg(any(test, feature = "qa-screenshot"))]
pub(crate) fn fixture() -> Preparation {
    use super::preparation::Identity;
    let id = Uuid::from_u128;
    let cid = id(1);
    let month: NaiveDate = "2026-09-01".parse().unwrap();
    let at: DateTime<Utc> = "2026-10-25T01:00:00Z".parse().unwrap();
    let f = Fact {
        company_id: cid,
        employee_id: id(101),
        id: id(199001),
        agency: Agency::Sss,
        kind: Kind::Monthly,
        state: State::Received,
        starts_on: month,
        ends_on: "2026-09-30".parse().unwrap(),
        contribution_month: Some(month),
        received_on: Some("2026-10-01".parse().unwrap()),
        parts: Parts {
            employee_cents: Some(150000),
            employer_cents: Some(300000),
            ec_cents: Some(3000),
        },
    };
    let receipt = Receipt {
        id: id(199010),
        company_id: cid,
        agency: Agency::Sss,
        paid_on: "2026-10-02".parse().unwrap(),
        amount_cents: 453000,
        allocations: vec![ReceiptAllocation {
            month,
            amount_cents: 453000,
        }],
        reference: "QA-SBR-01".into(),
        evidence: "Fictional receipt only".into(),
        note: "".into(),
        replaces_id: None,
        recorded_by: id(7),
        recorded_at: at,
        void_id: None,
        void_reason: None,
        voided_by: None,
        voided_at: None,
        replacement_id: None,
    };
    let source = Wire {
        version: "government-payroll-sources-v1".into(),
        company_id: cid,
        company_name: "Fictional Company".into(),
        generated_at: at,
        as_of: at.date_naive(),
        month,
        agency: Agency::Sss,
        facts: vec![f.clone()],
    };
    let review = ReviewWire {
        version: "government-reconciliation-v1".into(),
        source,
        effective_facts: vec![f],
        reviews: vec![],
        remittances: vec![receipt],
        history: vec![],
        employees: vec![EmployeeLabel {
            employee_id: id(101),
            employee_code: "TEST-SSS".into(),
            employee_name: "Fictional Staff".into(),
        }],
    };
    let registrations = vec![
        Identity {
            id: id(199020),
            company_id: cid,
            employee_id: None,
            revision: 0,
            details: [
                ("registered_name".into(), "Fictional Company".into()),
                ("sss_number".into(), "0123456789".into()),
            ]
            .into(),
        },
        Identity {
            id: id(199021),
            company_id: cid,
            employee_id: Some(id(101)),
            revision: 0,
            details: [
                ("last_name".into(), "Dela Cruz".into()),
                ("first_name".into(), "Alex".into()),
                ("middle_name".into(), "M".into()),
                ("name_suffix".into(), "".into()),
                ("sss_number".into(), "0212345678".into()),
            ]
            .into(),
        },
    ];
    Preparation::from_wire(
        PreparationWire {
            version: "government-preparation-v1".into(),
            review,
            registrations,
        },
        cid,
        Filter {
            month,
            agency: Agency::Sss,
        },
    )
    .unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_save_keeps_existing_files_and_requires_text_extension() {
        let folder = std::env::temp_dir().join(format!("hris-reference-{}", Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("reference.txt");
        save_reference(&path, b"review draft\r\n").unwrap();
        assert!(save_reference(&path, b"replacement").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"review draft\r\n");
        assert!(save_reference(&folder.join("wrong.csv"), b"data").is_err());
        assert!(!folder.join("wrong.csv").exists());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    fn request(b: &Basis) -> Request {
        Request {
            expected_fingerprint: b.fingerprint.clone(),
            receipt_id: b.receipt.id,
            employer_name: b.employer_name.clone(),
            reviewed: true,
            declarations: vec![Declaration {
                employee_id: b.people[0].employee_id,
                last_name: "Dela Cruz".into(),
                first_name: "Alex".into(),
                middle_initial: "M".into(),
                remark: sss_r3::Remark::Normal,
                event_on: None,
            }],
        }
    }
    #[test]
    fn draft_binds_uneditable_identifiers_amounts_and_receipt_to_complete_source() {
        let p = fixture();
        let b = Basis::from_preparation(&p).unwrap();
        let r = request(&b);
        let bytes = b.render(&r).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("0123456789QA-SBR-01 "));
        assert!(text.contains("0212345678"));
        assert!(text.contains("4500.00"));
        assert_eq!(text.lines().count(), 3);
        let mut later = p.clone();
        later.report.wire.source.generated_at += chrono::Duration::seconds(1);
        assert_eq!(
            Basis::from_preparation(&later).unwrap().fingerprint,
            b.fingerprint
        );
    }
    #[test]
    fn changed_registration_revision_or_source_rejects_old_declarations() {
        let p = fixture();
        let b = Basis::from_preparation(&p).unwrap();
        let r = request(&b);
        let mut changed = p.clone();
        changed.registrations[1].revision += 1;
        let fresh = Basis::from_preparation(&changed).unwrap();
        assert!(fresh.render(&r).is_err());
        let mut changed = p.clone();
        changed.report.wire.source.as_of += chrono::Duration::days(1);
        assert!(
            Basis::from_preparation(&changed)
                .unwrap()
                .render(&r)
                .is_err()
        );
        let mut bad = r.clone();
        bad.reviewed = false;
        assert!(b.render(&bad).is_err());
        let mut bad = r.clone();
        bad.receipt_id = Uuid::new_v4();
        assert!(b.render(&bad).is_err());
        let mut bad = r.clone();
        bad.expected_fingerprint = "0".repeat(64);
        assert!(b.render(&bad).is_err());
        let mut bad = r;
        bad.employer_name.clear();
        assert!(b.render(&bad).is_err());
    }
    #[test]
    fn unresolved_missing_registration_split_void_and_wrong_declarations_fail_closed() {
        let p = fixture();
        let b = Basis::from_preparation(&p).unwrap();
        for case in 0..7 {
            let mut changed = p.clone();
            match case {
                0 => changed.registrations.clear(),
                1 => changed.report.wire.remittances.clear(),
                2 => {
                    changed.report.wire.source.facts[0].parts.employer_cents = None;
                    changed.report.wire.effective_facts[0].parts.employer_cents = None;
                }
                3 => {
                    changed.report.wire.remittances[0].reference = "LONG-PRN-DO-NOT-TRUNCATE".into()
                }
                4 => {
                    changed.report.wire.remittances[0].amount_cents /= 2;
                    changed.report.wire.remittances[0].allocations[0].amount_cents /= 2;
                    let mut second = changed.report.wire.remittances[0].clone();
                    second.id = Uuid::new_v4();
                    second.reference = "OTHER".into();
                    changed.report.wire.remittances.push(second);
                }
                5 => {
                    let r = &mut changed.report.wire.remittances[0];
                    r.void_id = Some(Uuid::new_v4());
                    r.void_reason = Some("Synthetic void".into());
                    r.voided_by = Some(Uuid::from_u128(7));
                    r.voided_at = Some(Utc::now());
                }
                6 => {
                    let r = &mut changed.report.wire.remittances[0];
                    r.allocations.push(ReceiptAllocation {
                        month: "2026-10-01".parse().unwrap(),
                        amount_cents: 100,
                    });
                    r.amount_cents += 100;
                }
                _ => unreachable!(),
            }
            assert!(Basis::from_preparation(&changed).is_err(), "{case}");
        }
        let mut r = request(&b);
        r.declarations[0].employee_id = Uuid::new_v4();
        assert!(b.render(&r).is_err());
        let mut r = request(&b);
        r.declarations.push(r.declarations[0].clone());
        assert!(b.render(&r).is_err());
        let mut r = request(&b);
        r.declarations[0].remark = sss_r3::Remark::NoEarnings;
        assert!(b.render(&r).is_err());
    }
}
