//! Small printable payslip using standard PDF fonts; no external runtime/service.
//! Unsupported font characters fail explicitly; UTF-8 CSV preserves every name.
use crate::payroll::{Slip, money, totals_rows};
use anyhow::{Context, Result, ensure};
use std::{io::Write, path::Path};
fn winansi(s: &str) -> Result<Vec<u8>> {
    s.chars().map(|c|Ok(match c{ '€'=>128,'‚'=>130,'ƒ'=>131,'„'=>132,'…'=>133,'†'=>134,'‡'=>135,'ˆ'=>136,'‰'=>137,'Š'=>138,'‹'=>139,'Œ'=>140,'Ž'=>142,'‘'=>145,'’'=>146,'“'=>147,'”'=>148,'•'=>149,'–'=>150,'—'=>151,'˜'=>152,'™'=>153,'š'=>154,'›'=>155,'œ'=>156,'ž'=>158,'Ÿ'=>159,_=>{ensure!((c as u32)>=32&&(c as u32)<=255&&!(127..160).contains(&(c as u32)),"This printable font cannot represent a character in the payslip. Use the UTF-8 CSV export to preserve the exact text.");c as u8}})).collect()
}
fn literal(s: &str) -> Result<String> {
    let mut out = String::from("(");
    for b in winansi(s)? {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            32..=126 => out.push(b as char),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out.push(')');
    Ok(out)
}
fn wrap(s: &str, out: &mut Vec<String>) {
    for raw in s.lines() {
        let mut line = String::new();
        for c in raw.chars() {
            if line.chars().count() >= 75 {
                out.push(std::mem::take(&mut line));
            }
            line.push(if c == '\t' { ' ' } else { c });
        }
        out.push(line);
    }
    if s.is_empty() {
        out.push(String::new());
    }
}
pub fn build(s: &Slip) -> Result<Vec<u8>> {
    build_period(s, &format!("Pay period: {}", s.month.format("%B %Y")))
}
pub fn build_cutoff(s: &crate::payroll_cutoffs::Slip) -> Result<Vec<u8>> {
    build_period(
        &s.base,
        &format!(
            "Pay period: {} to {} | Cutoff {} | Pay date: {}",
            s.starts_on, s.ends_on, s.part, s.pay_date
        ),
    )
}
fn build_period(s: &Slip, period: &str) -> Result<Vec<u8>> {
    let mut lines = vec![];
    for text in [
        s.company_name.clone(),
        period.to_string(),
        format!(
            "Employee: {} - {}",
            s.item.employee_code, s.item.employee_name
        ),
        format!("Department: {}", s.item.department),
        format!("Approved: {} UTC", s.approved_at.format("%Y-%m-%d %H:%M")),
        format!("Rule pack: {}", s.rule_version),
        String::new(),
    ] {
        wrap(&text, &mut lines);
    }
    for (label, n) in totals_rows(&s.item.totals) {
        wrap(&format!("{label:<42} {:>20}", money(n)), &mut lines);
    }
    wrap("", &mut lines);
    wrap(
        "Approved payroll calculation; payment is recorded separately.",
        &mut lines,
    );
    wrap(
        "Employer benefits and 13th-month accrual are memos, not added to net pay.",
        &mut lines,
    );
    if !s.item.profile.pay.components.is_empty() || !s.item.worksheet.adjustments.is_empty() {
        wrap("", &mut lines);
        wrap("EARNINGS / DEDUCTION COMPONENTS", &mut lines);
        for c in s
            .item
            .profile
            .pay
            .components
            .iter()
            .chain(&s.item.worksheet.adjustments)
        {
            wrap(
                &format!(
                    "{} / {}: {}",
                    c.label,
                    c.kind.replace('_', " "),
                    money(c.amount_cents)
                ),
                &mut lines,
            );
        }
    }
    let chunks: Vec<_> = lines.chunks(47).collect();
    let page_ids: Vec<_> = (0..chunks.len()).map(|i| 5 + i * 2).collect();
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!(
            "<< /Type /Pages /Count {} /Kids [{}] >>",
            chunks.len(),
            page_ids
                .iter()
                .map(|i| format!("{i} 0 R"))
                .collect::<Vec<_>>()
                .join(" ")
        )
        .into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    ];
    for (n, chunk) in chunks.iter().enumerate() {
        let page = page_ids[n];
        let mut stream = String::from(
            "0.07 0.25 0.29 rg 0 780 595 62 re f\n1 1 1 rg BT /F2 18 Tf 36 805 Td (PAYSLIP) Tj ET\n0.12 0.17 0.21 rg BT /F1 10.5 Tf 14 TL 36 756 Td\n",
        );
        for line in *chunk {
            stream.push_str(&format!("{} Tj T*\n", literal(line)?));
        }
        stream.push_str(&format!("ET\n0.4 0.45 0.5 rg BT /F1 8 Tf 36 34 Td (Private payroll copy - Page {} of {}) Tj ET\n",n+1,chunks.len()));
        let bytes = stream.into_bytes();
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",page+1).into_bytes());
        let mut obj = format!("<< /Length {} >>\nstream\n", bytes.len()).into_bytes();
        obj.extend(bytes);
        obj.extend(b"endstream");
        objects.push(obj);
    }
    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0];
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n", i + 1).as_bytes());
        pdf.extend(obj);
        pdf.extend(b"\nendobj\n");
    }
    let start = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for o in offsets.iter().skip(1) {
        pdf.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    pdf.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            start
        )
        .as_bytes(),
    );
    Ok(pdf)
}
pub fn save(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure!(
        path.extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("pdf")),
        "Use a .pdf filename."
    );
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context("Choose a new PDF filename; existing files are preserved.")?;
    if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(e).context("PDF could not be completely saved.");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pdf_literal_preserves_enye_and_escapes_syntax() {
        assert_eq!(literal(r"Peña (HR) \").unwrap(), r"(Pe\361a \(HR\) \\)");
        assert!(literal("王").is_err());
    }
}
