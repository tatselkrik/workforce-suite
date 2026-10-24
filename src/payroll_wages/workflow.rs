use super::*;
use crate::payroll::{Event, month_end};
pub fn validate_record(c: &Classification) -> Result<()> {
    ensure!(
        c.starts_on <= c.ends_on && c.starts_on.year() == 2026 && c.ends_on.year() == 2026,
        "Use a2026 effective interval."
    );
    ensure!(
        ["covered", "exempt"].contains(&c.coverage.as_str()),
        "Choose covered or an evidenced exemption; unknown coverage requires review."
    );
    crate::payroll::text(&c.workplace, 240, true)?;
    crate::payroll::text(&c.evidence, 1000, true)?;
    crate::payroll::text(&c.note, 1000, true)?;
    if c.coverage == "covered" {
        ensure!(
            catalog().rates.iter().any(|r| r.series_key == c.series_key),
            "Choose a verified regional category."
        );
    } else {
        ensure!(
            c.series_key.is_empty(),
            "An exemption must not pretend to select a wage rate."
        );
    }
    Ok(())
}
pub fn apply_demo(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    payload(a, data)?;
    let v = &a.record;
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    let op = a.operation.strip_prefix("wage_").unwrap();
    let before;
    let after;
    if op == "settings" {
        ensure!(id == data.company.id, "Company settings identity mismatch.");
        let old = data
            .payroll
            .wages
            .settings
            .iter()
            .find(|s| s.id == id)
            .cloned();
        let rev = old.as_ref().map_or(0, |s| s.revision);
        ensure!(
            v["expected_revision"].as_i64() == Some(rev),
            "Wage settings changed. Refresh."
        );
        ensure!(v["enforced"].is_boolean(), "Choose an enforcement state.");
        before = old.as_ref().map(|s| serde_json::to_value(s).unwrap());
        let s = Settings {
            id,
            company_id: id,
            enforced: v["enforced"].as_bool().unwrap(),
            revision: if old.is_some() { rev + 1 } else { 1 },
        };
        after = serde_json::to_value(&s)?;
        data.payroll.wages.settings.retain(|s| s.id != id);
        data.payroll.wages.settings.push(s);
    } else {
        let old = data
            .payroll
            .wages
            .classifications
            .iter()
            .find(|c| c.id == id)
            .cloned();
        if op == "create" && old.is_some() {
            let birth = data
                .payroll
                .events
                .iter()
                .find(|e| e.record_id == id && e.kind == "wage_create")
                .context("Creation identity already used.")?;
            ensure!(
                birth.after_record["birth"] == *v
                    && birth.after_record["birth_actor"] == json!(data.access.user_id),
                "Creation identity already used."
            );
            return Ok(());
        }
        before = old.as_ref().map(|c| serde_json::to_value(c).unwrap());
        let mut c = if op == "create" {
            ensure!(
                old.is_none() && v.get("expected_revision").is_none(),
                "New wage review has no revision."
            );
            let eid: Uuid = serde_json::from_value(v["employee_id"].clone())?;
            Classification {
                id,
                company_id: data.company.id,
                employee_id: eid,
                starts_on: serde_json::from_value(v["starts_on"].clone())?,
                ends_on: serde_json::from_value(v["ends_on"].clone())?,
                coverage: v["coverage"].as_str().unwrap_or("").into(),
                series_key: v["series_key"].as_str().unwrap_or("").into(),
                workplace: v["workplace"].as_str().unwrap_or("").trim().into(),
                evidence: v["evidence"].as_str().unwrap_or("").trim().into(),
                note: v["note"].as_str().unwrap_or("").trim().into(),
                employee_basis: employee_basis(data, eid)?,
                status: "draft".into(),
                revision: 0,
                created_by: data.access.user_id,
                prepared_by: data.access.user_id,
                approved_by: None,
                approved_at: None,
            }
        } else {
            let c = old.context("Wage review unavailable.")?;
            ensure!(
                c.company_id == data.company.id
                    && Some(c.revision) == v["expected_revision"].as_i64(),
                "Wage review changed. Refresh."
            );
            c
        };
        match op {
            "create" => validate_record(&c)?,
            "save" => {
                ensure!(c.status == "draft", "Only draft wage reviews may change.");
                c.starts_on = serde_json::from_value(v["starts_on"].clone())?;
                c.ends_on = serde_json::from_value(v["ends_on"].clone())?;
                for (field, target) in [
                    ("coverage", &mut c.coverage),
                    ("series_key", &mut c.series_key),
                    ("workplace", &mut c.workplace),
                    ("evidence", &mut c.evidence),
                    ("note", &mut c.note),
                ] {
                    *target = v[field].as_str().unwrap_or("").trim().into();
                }
                c.employee_basis = employee_basis(data, c.employee_id)?;
                c.prepared_by = data.access.user_id;
                validate_record(&c)?;
            }
            "approve" => {
                validate_record(&c)?;
                ensure!(
                    c.status == "draft"
                        && ![c.created_by, c.prepared_by].contains(&data.access.user_id),
                    "A different Administrator must approve this wage classification."
                );
                ensure!(
                    c.employee_basis == employee_basis(data, c.employee_id)?,
                    "Workplace/employment changed. Save a fresh draft review."
                );
                ensure!(
                    !data.payroll.wages.classifications.iter().any(|x| x.id != id
                        && x.company_id == c.company_id
                        && x.employee_id == c.employee_id
                        && x.status == "approved"
                        && x.starts_on <= c.ends_on
                        && x.ends_on >= c.starts_on),
                    "Approved wage classifications must not overlap."
                );
                c.status = "approved".into();
                c.approved_by = Some(data.access.user_id);
                c.approved_at = Some(Utc::now());
                let p = Pay {
                    rate_cents: 10_000_000_000,
                    ..Default::default()
                };
                let s = Source {
                    company_id: c.company_id,
                    employee_id: c.employee_id,
                    pay: BasicPay::from(&p),
                    employee_basis: c.employee_basis.clone(),
                    starts_on: c.starts_on,
                    ends_on: c.ends_on,
                    classifications: vec![c.clone()],
                };
                ensure!(
                    calculate(&s)?.blocking_days == 0,
                    "Some classification dates have no verified wage rate. Use supported dates or review the coverage."
                );
            }
            "retire" => {
                ensure!(
                    c.status == "approved",
                    "Only approved classifications can retire."
                );
                c.status = "retired".into();
            }
            "cancel" => {
                ensure!(c.status == "draft", "Only draft reviews can cancel.");
                c.status = "cancelled".into();
            }
            _ => anyhow::bail!("Unsupported wage operation."),
        }
        if op != "create" {
            c.revision += 1;
        }
        after = serde_json::to_value(&c)?;
        data.payroll.wages.classifications.retain(|x| x.id != id);
        data.payroll.wages.classifications.push(c);
    }
    let event_after = if op == "create" {
        json!({"record":after,"birth":v,"birth_actor":data.access.user_id})
    } else {
        after
    };
    let role = data.access.role;
    data.payroll.events.push(Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        record_id: id,
        kind: a.operation.clone(),
        reason: v["reason"].as_str().unwrap().trim().into(),
        actor_name: data
            .access
            .employee_id
            .map(|eid| data.employee_name(eid))
            .unwrap_or("HR staff")
            .to_string(),
        actor_role: role,
        before_record: before,
        after_record: event_after,
        recorded_at: Utc::now(),
    });
    Ok(())
}
pub fn run_assessment(data: &Snapshot, kind: &str, id: Uuid) -> Result<Value> {
    let mut rows = vec![];
    if kind == "monthly" {
        let run = data
            .payroll
            .runs
            .iter()
            .find(|r| r.id == id && r.company_id == data.company.id)
            .context("Monthly run unavailable.")?;
        for i in data
            .payroll
            .items
            .iter()
            .filter(|i| i.run_id == id && i.company_id == data.company.id)
        {
            let s = source(
                data,
                i.employee_id,
                &i.profile.pay,
                run.month,
                month_end(run.month),
            )?;
            rows.push(json!({"employee_id":i.employee_id,"employee_code":i.employee_code,"employee_name":i.employee_name,"source":s,"result":calculate(&s)?}));
        }
    } else {
        ensure!(kind == "cutoff", "Choose monthly or cutoff wage checks.");
        let run = data
            .payroll
            .cutoffs
            .runs
            .iter()
            .find(|r| r.base.id == id && r.base.company_id == data.company.id)
            .context("Cutoff run unavailable.")?;
        for i in data
            .payroll
            .cutoffs
            .items
            .iter()
            .map(|i| &i.base)
            .filter(|i| i.run_id == id && i.company_id == data.company.id)
        {
            let s = source(
                data,
                i.employee_id,
                &i.profile.pay,
                run.starts_on,
                run.ends_on,
            )?;
            rows.push(json!({"employee_id":i.employee_id,"employee_code":i.employee_code,"employee_name":i.employee_name,"source":s,"result":calculate(&s)?}));
        }
    }
    ensure!(
        rows.len() <= 500,
        "At most500recipients per wage-reviewed run."
    );
    rows.sort_by_key(|v| v["employee_id"].as_str().unwrap_or("").to_string());
    let revision = data
        .payroll
        .wages
        .settings
        .iter()
        .find(|s| s.id == data.company.id)
        .map_or(0, |s| s.revision);
    Ok(
        json!({"version":VERSION,"enforced":enabled(data),"settings_revision":revision,"employees":rows}),
    )
}
pub fn record_run_check(
    data: &mut Snapshot,
    kind: &str,
    id: Uuid,
    stage: &str,
    revision: i64,
) -> Result<()> {
    let assessment = run_assessment(data, kind, id)?;
    if enabled(data) {
        ensure!(
            assessment["employees"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["result"]["blocking_days"] == 0),
            "Resolve the regional wage flags before submitting/approving this payroll."
        );
    }
    if stage == "approved" {
        if let Some(previous) = data
            .payroll
            .wages
            .checks
            .iter()
            .filter(|c| {
                c.company_id == data.company.id
                    && c.kind == kind
                    && c.run_id == id
                    && c.stage == "submitted"
            })
            .max_by_key(|c| c.run_revision)
        {
            ensure!(
                previous.assessment == assessment,
                "Wage facts changed after submission. Return and resubmit payroll."
            );
        } else {
            ensure!(
                !enabled(data),
                "Resubmit this older payroll to record its wage checks."
            );
        }
    }
    data.payroll.wages.checks.push(Check {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        run_id: id,
        kind: kind.into(),
        stage: stage.into(),
        run_revision: revision,
        enforced: enabled(data),
        assessment,
        recorded_at: Utc::now(),
    });
    Ok(())
}
