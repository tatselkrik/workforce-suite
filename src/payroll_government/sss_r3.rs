//! A bounded, integer-centavo renderer for the currently downloaded SSS R3
//! generator's fixed-width reference format. This is not a protected export API.
//! An adapter must still validate current agency semantics, declarations,
//! payment evidence and fresh HR scope before offering a file to a person.
use anyhow::{Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub const MAX_PEOPLE: usize = 10_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Remark {
    Normal,
    NewHire,
    Separated,
    NoEarnings,
}
impl Remark {
    fn code(self) -> char {
        match self {
            Self::Normal => 'N',
            Self::NewHire => '1',
            Self::Separated => '2',
            Self::NoEarnings => '3',
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub employee_id: Uuid,
    pub sss_number: String,
    pub last_name: String,
    pub first_name: String,
    /// Explicit single-character declaration; never guessed from a full name.
    pub middle_initial: String,
    pub ss_cents: i64,
    pub ec_cents: i64,
    /// Explicit HR declaration, separate from employment/account status.
    pub remark: Remark,
    pub event_on: Option<NaiveDate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub company_id: Uuid,
    pub month: NaiveDate,
    pub as_of: NaiveDate,
    pub employer_name: String,
    pub employer_sss: String,
    pub receipt_number: String,
    pub paid_on: NaiveDate,
    pub paid_cents: i64,
    pub people: Vec<Person>,
}

fn identifier(v: &str) -> Result<()> {
    ensure!(
        v.len() == 10 && v.bytes().all(|b| b.is_ascii_digit()) && v != "0000000000",
        "Supply an assigned ten-digit SSS identifier; preserve leading zeros."
    );
    Ok(())
}

fn text(v: &str, width: usize, required: bool) -> Result<String> {
    ensure!(
        !required || !v.trim().is_empty(),
        "An explicit reference field is missing."
    );
    ensure!(
        v.len() <= width && v.bytes().all(|b| (32..=126).contains(&b)),
        "Reference fields must fit the declared ASCII width; no truncation or transliteration is applied."
    );
    Ok(format!("{:<width$}", v.to_ascii_uppercase()))
}

fn money(cents: i64, width: usize, zero_pad: bool) -> Result<String> {
    ensure!(
        cents >= 0,
        "Negative adjustments need a reviewed agency correction process."
    );
    let s = format!("{}.{:02}", cents / 100, cents % 100);
    ensure!(
        s.len() <= width,
        "An amount exceeds the reference field width."
    );
    Ok(if zero_pad {
        format!("{s:0>width$}")
    } else {
        format!("{s:>width$}")
    })
}

fn slots(ss: i64, ec: i64, position: usize, total: bool) -> Result<String> {
    let mut out = String::new();
    for i in 0..3 {
        out.push_str(&money(
            if i == position { ss } else { 0 },
            if total { 11 } else { 7 },
            false,
        )?);
        out.push(' ');
    }
    // The reference writer emits these three slots as zero. Their agency
    // semantics remain an integration gate; they are not inferred source facts.
    for _ in 0..3 {
        out.push_str(&money(0, if total { 9 } else { 5 }, false)?);
        out.push(' ');
    }
    for i in 0..3 {
        out.push_str(&money(
            if i == position { ec } else { 0 },
            if total { 9 } else { 5 },
            false,
        )?);
        out.push(' ');
    }
    Ok(out)
}

impl Draft {
    /// Returns an in-memory reference candidate only. There is deliberately no
    /// filesystem, database, filing, or network operation in this renderer.
    pub fn render_reference(&self) -> Result<Vec<u8>> {
        ensure!(!self.company_id.is_nil(), "Company identity is required.");
        ensure!(
            self.month.day() == 1
                && (2025..=2026).contains(&self.month.year())
                && self.month <= self.as_of.with_day(1).unwrap(),
            "Choose a supported elapsed/current contribution month."
        );
        ensure!(
            self.paid_on <= self.as_of && self.paid_on.year() >= 2025,
            "A reference receipt must have an elapsed, supported payment date."
        );
        ensure!(
            (1..=MAX_PEOPLE).contains(&self.people.len()),
            "Choose a bounded, nonempty employee set."
        );
        identifier(&self.employer_sss)?;
        let employer = text(&self.employer_name, 30, true)?;
        let receipt = text(&self.receipt_number, 10, true)?;
        let mut ids = HashSet::new();
        let mut numbers = HashSet::new();
        let mut ss = 0i64;
        let mut ec = 0i64;
        let mut rows = Vec::with_capacity(self.people.len());
        let position = (self.month.month0() % 3) as usize;
        for p in &self.people {
            ensure!(
                !p.employee_id.is_nil() && ids.insert(p.employee_id),
                "Duplicate/missing employee identity."
            );
            identifier(&p.sss_number)?;
            ensure!(
                numbers.insert(&p.sss_number),
                "Duplicate SSS identifiers need review."
            );
            let name = format!(
                "{}{}{}",
                text(&p.last_name, 15, true)?,
                text(&p.first_name, 15, true)?,
                text(&p.middle_initial, 1, false)?
            );
            ensure!(
                (p.remark == Remark::NoEarnings) == (p.ss_cents == 0 && p.ec_cents == 0),
                "The no-earnings declaration and zero SS/EC must agree."
            );
            ensure!(
                matches!(p.remark, Remark::NewHire | Remark::Separated) == p.event_on.is_some(),
                "Declare the hire/separation date only for the matching employment event."
            );
            let event = match p.event_on {
                Some(d) => {
                    ensure!(
                        d.year() >= 1900
                            && d <= self.as_of
                            && d <= self.paid_on
                            && d.year() == self.month.year()
                            && d.month() == self.month.month(),
                        "Check the declared employment event date."
                    );
                    d.format("%m%d%Y").to_string()
                }
                None => "0       ".into(),
            };
            let row = format!(
                "20{name}{} {}     {}{event}",
                p.sss_number,
                slots(p.ss_cents, p.ec_cents, position, false)?,
                p.remark.code()
            );
            ss = ss
                .checked_add(p.ss_cents)
                .ok_or_else(|| anyhow::anyhow!("SS total overflow."))?;
            ec = ec
                .checked_add(p.ec_cents)
                .ok_or_else(|| anyhow::anyhow!("EC total overflow."))?;
            // The official TreeSet compares names only and can omit namesakes.
            // Retain every distinct person, using their SSS number as a tie-break.
            rows.push((name, p.sss_number.clone(), row));
        }
        ensure!(
            ss.checked_add(ec) == Some(self.paid_cents),
            "The declared payment and contribution totals differ."
        );
        let mut out = format!(
            "00{employer}{}{}{receipt}{}{}\r\n",
            self.month.format("%m%Y"),
            self.employer_sss,
            self.paid_on.format("%m%d%Y"),
            money(self.paid_cents, 12, true)?
        );
        rows.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        for (_, _, row) in rows {
            out.push_str(&row);
            out.push_str("\r\n");
        }
        out.push_str("99 ");
        out.push_str(&slots(ss, ec, position, true)?);
        out.push_str("                   \r\n");
        Ok(out.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    fn draft(month: u32) -> Draft {
        Draft {
            company_id: Uuid::from_u128(1),
            month: NaiveDate::from_ymd_opt(2026, month, 1).unwrap(),
            as_of: "2026-12-31".parse().unwrap(),
            employer_name: "Fictional Industries".into(),
            employer_sss: "0123456789".into(),
            receipt_number: "1234567890".into(),
            paid_on: "2026-12-31".parse().unwrap(),
            paid_cents: 579075,
            people: vec![
                Person {
                    employee_id: Uuid::from_u128(2),
                    sss_number: "0212345678".into(),
                    last_name: "Dela Cruz".into(),
                    first_name: "Alex".into(),
                    middle_initial: "M".into(),
                    ss_cents: 525025,
                    ec_cents: 3000,
                    remark: Remark::Normal,
                    event_on: None,
                },
                Person {
                    employee_id: Uuid::from_u128(3),
                    sss_number: "0312345678".into(),
                    last_name: "Santos".into(),
                    first_name: "Jamie".into(),
                    middle_initial: "".into(),
                    ss_cents: 50050,
                    ec_cents: 1000,
                    remark: Remark::Separated,
                    event_on: Some(NaiveDate::from_ymd_opt(2026, month, 15).unwrap()),
                },
            ],
        }
    }
    #[test]
    fn independent_sss_reference_matches_all_twelve_month_slots_and_exact_centavos() {
        let vectors: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../tests/fixtures/sss-r3-reference.json"))
                .unwrap();
        assert_eq!(vectors.len(), 12);
        for v in vectors {
            let mut expected = Vec::new();
            for field in std::iter::once(v["header_b64"].as_str().unwrap())
                .chain(
                    v["rows_b64"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap()),
                )
                .chain(std::iter::once(v["trailer_b64"].as_str().unwrap()))
            {
                expected.extend(STANDARD.decode(field).unwrap());
                expected.extend(b"\r\n");
            }
            assert_eq!(
                String::from_utf8(
                    draft(v["month"].as_u64().unwrap() as u32)
                        .render_reference()
                        .unwrap()
                )
                .unwrap(),
                String::from_utf8(expected).unwrap()
            );
        }
    }
    #[test]
    fn namesakes_are_never_dropped_and_source_order_does_not_change_bytes() {
        let mut d = draft(10);
        d.people[1].last_name = d.people[0].last_name.clone();
        d.people[1].first_name = d.people[0].first_name.clone();
        d.people[1].middle_initial = "M".into();
        let bytes = d.render_reference().unwrap();
        d.people.reverse();
        assert_eq!(d.render_reference().unwrap(), bytes);
        assert_eq!(std::str::from_utf8(&bytes).unwrap().lines().count(), 4);
    }
    #[test]
    fn malformed_unknown_ambiguous_and_oversize_inputs_fail_without_data_in_errors() {
        let d = draft(10);
        for field in [
            "employer",
            "person",
            "amount",
            "total",
            "duplicate",
            "future",
            "unicode",
            "newline",
            "event",
            "remark",
        ] {
            let mut bad = d.clone();
            match field {
                "employer" => bad.employer_sss = "0000000000".into(),
                "person" => bad.people[0].sss_number = "".into(),
                "amount" => bad.people[0].ss_cents = 1000000,
                "total" => bad.paid_cents += 1,
                "duplicate" => bad.people[1].employee_id = bad.people[0].employee_id,
                "future" => bad.month = "2027-01-01".parse().unwrap(),
                "unicode" => bad.people[0].last_name = "Peña".into(),
                "newline" => bad.receipt_number = "x\r\ny".into(),
                "event" => bad.people[0].event_on = Some("2027-01-01".parse().unwrap()),
                "remark" => bad.people[0].remark = Remark::NoEarnings,
                _ => unreachable!(),
            }
            let e = bad.render_reference().unwrap_err().to_string();
            assert!(!e.contains("0212345678") && !e.contains("Peña"), "{field}");
        }
    }
}
