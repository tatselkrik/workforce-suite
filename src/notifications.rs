//! A live, access-scoped inbox. Read markers contain keys, never HR content.
use crate::{access::Role, domain::*};
use anyhow::{Result, ensure};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;
pub mod lifecycle;

pub const MAX_READ_KEYS: usize = 5000;
pub const MAX_MARK_BATCH: usize = 100;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReadState {
    pub user_id: Uuid,
    pub company_id: Uuid,
    pub read_keys: Vec<String>,
}
#[derive(Clone, Debug)]
pub enum ReadAction {
    Mark { keys: Vec<String>, read: bool },
    Reset,
}
impl ReadAction {
    pub fn validate(&self) -> Result<()> {
        if let Self::Mark { keys, .. } = self {
            ensure!(!keys.is_empty() && keys.len() <= MAX_MARK_BATCH, "Choose 1–100 notifications.");
            ensure!(keys.iter().all(|k| valid_key(k) || lifecycle::valid_key(k)) && keys.iter().collect::<HashSet<_>>().len() == keys.len(), "The notification keys are invalid.");
        }
        Ok(())
    }
    pub fn payload(&self, company: Uuid) -> Result<serde_json::Value> {
        self.validate()?;
        Ok(match self {
            Self::Mark { keys, read } => serde_json::json!({"p_company_id":company,"p_keys":keys,"p_read":read,"p_reset":false}),
            Self::Reset => serde_json::json!({"p_company_id":company,"p_keys":[],"p_read":false,"p_reset":true}),
        })
    }
}
pub fn valid_key(key: &str) -> bool {
    if key.len() > 130 { return false; }
    let p: Vec<_> = key.split(':').collect();
    if p.len() != 3 && p.len() != 4 { return false; }
    matches!(p[0],"leave-review"|"leave-status"|"leave-upcoming"|"correction-review"|"correction-status"|"overtime-review"|"overtime-status"|"document-expiry"|"contract-end"|"checkout")
        && Uuid::parse_str(p[1]).is_ok_and(|id| !id.is_nil() && id.to_string() == p[1])
        && !p[2].is_empty() && p[2].len() <= 19 && p[2].bytes().all(|c| c.is_ascii_digit()) && p[2].parse::<i64>().is_ok()
        && (p.len() == 3 || matches!(p[3], "soon"|"today"|"expired"))
}
impl ReadState {
    pub fn validate(&self, user: Uuid, company: Uuid) -> Result<()> {
        ensure!(self.user_id == user && self.company_id == company, "Notification history did not match your account/company.");
        ensure!(self.read_keys.len() <= MAX_READ_KEYS && self.read_keys.iter().all(|k| valid_key(k)) && self.read_keys.iter().collect::<HashSet<_>>().len() == self.read_keys.len(), "Notification history is invalid.");
        Ok(())
    }
    pub fn apply(&mut self, action: &ReadAction) -> Result<()> {
        action.validate()?;
        if let ReadAction::Mark { keys, .. } = action { ensure!(keys.iter().all(|k| valid_key(k)), "Keep lifecycle markers separate from legacy history."); }
        let mut next: HashSet<_> = self.read_keys.iter().cloned().collect();
        match action {
            ReadAction::Reset => next.clear(),
            ReadAction::Mark { keys, read } => for k in keys { if *read { next.insert(k.clone()); } else { next.remove(k); } },
        }
        ensure!(next.len() <= MAX_READ_KEYS, "Read history is full. Reset read history in Notifications before marking more items.");
        self.read_keys = next.into_iter().collect();
        self.read_keys.sort();
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category { Approvals, Updates, Reminders, Documents, Payroll, Appraisals, Learning }
impl Category {
    pub const ALL: [Self;7] = [Self::Approvals,Self::Updates,Self::Reminders,Self::Documents,Self::Payroll,Self::Appraisals,Self::Learning];
    pub fn label(self) -> &'static str { match self { Self::Approvals=>"Approvals",Self::Updates=>"My request updates",Self::Reminders=>"Reminders",Self::Documents=>"Document expiry",Self::Payroll=>"Payroll",Self::Appraisals=>"Appraisals",Self::Learning=>"Learning" } }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target { Leave(Uuid), Correction(Uuid), Overtime(Uuid), Document { employee: Uuid, title: String }, Attendance(NaiveDate), Lifecycle { page:String, record:Uuid, employee:Uuid } }
#[derive(Clone, Debug)]
pub struct Alert { pub key: String, pub category: Category, pub title: String, pub detail: String, pub date: NaiveDate, pub urgent: bool, pub target: Target }
pub fn alerts(data: &Snapshot, now: chrono::DateTime<chrono::Utc>) -> Vec<Alert> {
    if data.mode == Mode::SignedOut || !data.access.role.uses_desktop() || data.company.id != data.access.company_id { return Vec::new(); }
    let mut out = Vec::new();
    let today = now.with_timezone(&data.company.tz()).date_naive();
    let company = data.company.id;
    let own = data.access.employee_id;
    let visible = |id:Uuid| data.access.role.manages_hr() || own == Some(id) || (data.access.role == Role::Manager && data.access.direct_reports.contains(&id));
    let active = |id:Uuid| data.employees.iter().any(|e| e.id == id && e.company_id == company && e.status == EmployeeStatus::Active);
    let name = |id| data.employee_name(id).to_owned();
    let recent = |date:NaiveDate| date >= today - Duration::days(30) && date <= today;
    let key = |kind:&str,id:Uuid,revision:i64| format!("{kind}:{id}:{}",revision.max(0));
    for r in data.leave.iter().filter(|r| r.company_id == company) {
        // A protected named approver can read a leave receipt without directory
        // access to the requester. Do not silently discard that eligible task.
        let employee=data.employees.iter().find(|e|e.company_id==company && e.id==r.employee_id);
        let requester=employee.map(|e|e.full_name.as_str()).unwrap_or_else(||if r.policy.requester_name.is_empty(){"Employee"}else{&r.policy.requester_name});
        let detail = format!("{} · {} to {}",requester,r.starts_on,r.ends_on);
        if crate::leave::can_review(data,r) && employee.is_none_or(|e|e.status==EmployeeStatus::Active) {
            out.push(Alert { key:key("leave-review",r.id,r.policy.revision),category:Category::Approvals,title:"Leave awaiting your approval".into(),detail:detail.clone(),date:r.starts_on,urgent:r.starts_on <= today,target:Target::Leave(r.id) });
        }
        if own == Some(r.employee_id) && visible(r.employee_id) {
            let last = data.leave_setup.events.iter().filter(|e| e.company_id == company && e.request_id == r.id).max_by_key(|e| e.created_at);
            if r.status != LeaveStatus::Pending && last.is_some_and(|e| recent(e.created_at.with_timezone(&data.company.tz()).date_naive())) {
                out.push(Alert { key:key("leave-status",r.id,r.policy.revision),category:Category::Updates,title:format!("Your leave was {}",r.status.label().to_lowercase()),detail:detail.clone(),date:last.unwrap().created_at.with_timezone(&data.company.tz()).date_naive(),urgent:false,target:Target::Leave(r.id) });
            }
            if active(r.employee_id) && r.status == LeaveStatus::Approved && r.starts_on >= today && r.starts_on <= today + Duration::days(7) {
                out.push(Alert { key:format!("{}:{}",key("leave-upcoming",r.id,r.policy.revision),if r.starts_on == today {"today"} else {"soon"}),category:Category::Reminders,title:if r.starts_on == today {"Your approved leave starts today"} else {"Your approved leave starts soon"}.into(),detail,date:r.starts_on,urgent:r.starts_on == today,target:Target::Leave(r.id) });
            }
        }
    }
    for r in data.requests.records.iter().filter(|r| r.company_id == company && visible(r.employee_id)) {
        if crate::requests::can_review(&data.access,r) && active(r.employee_id) {
            out.push(Alert { key:key("correction-review",r.id,r.revision),category:Category::Approvals,title:"Attendance correction awaiting review".into(),detail:format!("{} · {}",name(r.employee_id),r.work_date),date:r.work_date,urgent:false,target:Target::Correction(r.id) });
        }
        if own == Some(r.employee_id) && r.status != crate::requests::Status::Pending {
            if let Some(e) = data.requests.events.iter().filter(|e| e.company_id == company && e.request_id == r.id).max_by_key(|e| e.created_at).filter(|e| recent(e.created_at.with_timezone(&data.company.tz()).date_naive())) {
                out.push(Alert { key:key("correction-status",r.id,r.revision),category:Category::Updates,title:format!("Your attendance correction was {}",r.status.label().to_lowercase()),detail:r.work_date.to_string(),date:e.created_at.with_timezone(&data.company.tz()).date_naive(),urgent:false,target:Target::Correction(r.id) });
            }
        }
    }
    for r in data.overtime.records.iter().filter(|r| r.company_id == company && visible(r.employee_id)) {
        if r.status == crate::overtime::Status::Pending && crate::overtime::can_review(&data.access,r) && active(r.employee_id) {
            out.push(Alert { key:key("overtime-review",r.id,r.revision),category:Category::Approvals,title:"Overtime awaiting review".into(),detail:format!("{} · {} · {} requested min",name(r.employee_id),r.work_date,r.requested_minutes),date:r.work_date,urgent:false,target:Target::Overtime(r.id) });
        }
        if own == Some(r.employee_id) && r.status != crate::overtime::Status::Pending {
            if let Some(e) = data.overtime.events.iter().filter(|e| e.company_id == company && e.request_id == r.id).max_by_key(|e| e.created_at).filter(|e| recent(e.created_at.with_timezone(&data.company.tz()).date_naive())) {
                out.push(Alert { key:key("overtime-status",r.id,r.revision),category:Category::Updates,title:format!("Your overtime request was {}",r.status.label().to_lowercase()),detail:format!("{} · open the request to check its current basis",r.work_date),date:e.created_at.with_timezone(&data.company.tz()).date_naive(),urgent:false,target:Target::Overtime(r.id) });
            }
        }
    }
    if data.access.role.manages_hr() {
        for d in data.documents.iter().filter(|d| d.company_id == company && active(d.employee_id) && d.state == crate::documents::DocumentState::Ready && !d.archived) {
            for (kind,label,date) in [("document-expiry","Document expiry",d.details.expires_on),("contract-end","Contract ending",d.details.contract_end)] {
                if let Some(date) = date.filter(|date| *date <= today + Duration::days(30)) {
                    let phase = if date < today {"expired"} else if date == today {"today"} else {"soon"};
                    out.push(Alert { key:format!("{}:{phase}",key(kind,d.id,d.revision)),category:Category::Documents,title:format!("{label} · {}",if phase == "expired" {"past date"} else if phase == "today" {"today"} else {"within 30 days"}),detail:format!("{} · {} · {}",name(d.employee_id),d.details.title,date),date,urgent:date <= today,target:Target::Document { employee:d.employee_id,title:d.details.title.clone() } });
                }
            }
        }
    }
    for a in data.attendance.iter().filter(|a| a.company_id == company && Some(a.employee_id) == own && active(a.employee_id) && a.checked_out_at.is_none() && a.work_date < today) {
        // Date alone must not flag a still-running overnight shift.
        if a.checked_in_at + Duration::hours(24) <= now {
            out.push(Alert { key:key("checkout",a.id,a.revision),category:Category::Reminders,title:"Review an unfinished check-out".into(),detail:format!("{} · an attendance record is still open",a.work_date),date:a.work_date,urgent:true,target:Target::Attendance(a.work_date) });
        }
    }
    if let Some(inbox) = &data.lifecycle_notifications {
        if inbox.validate(&data.access, today).is_ok() { out.extend(inbox.alerts()); }
    }
    out.sort_by(|a,b| b.urgent.cmp(&a.urgent).then(a.date.cmp(&b.date)).then(a.key.cmp(&b.key)));
    let mut seen = HashSet::new();
    out.retain(|a| seen.insert(a.key.clone()));
    out
}
pub fn is_read(data:&Snapshot,key:&str)->bool {
    data.notification_reads.read_keys.iter().any(|k|k==key)
        || data.lifecycle_notifications.as_ref().is_some_and(|s|s.read_keys.iter().any(|k|k==key))
}
pub fn read_count(data:&Snapshot)->usize {
    data.notification_reads.read_keys.len()+data.lifecycle_notifications.as_ref().map_or(0,|s|s.read_keys.len())
}
pub fn apply_reads(data:&mut Snapshot,action:&ReadAction)->Result<()> {
    action.validate()?;
    let mut legacy=data.notification_reads.clone();
    let mut life=data.lifecycle_notifications.clone();
    match action {
        ReadAction::Reset=>{legacy.apply(action)?;if let Some(s)=&mut life{s.read_keys.clear();}}
        ReadAction::Mark{keys,read}=>{
            let old:Vec<_>=keys.iter().filter(|k|valid_key(k)).cloned().collect();
            let new:Vec<_>=keys.iter().filter(|k|lifecycle::valid_key(k)).cloned().collect();
            if !old.is_empty(){legacy.apply(&ReadAction::Mark{keys:old,read:*read})?;}
            if !new.is_empty(){
                let s=life.as_mut().ok_or_else(||anyhow::anyhow!("Refresh current lifecycle notices first."))?;
                ensure!(new.iter().all(|k|s.items.iter().any(|i|&i.key==k)),"A notice changed or is unavailable.");
                let mut next:HashSet<_>=s.read_keys.iter().cloned().collect();
                for k in new{if *read{next.insert(k);}else{next.remove(&k);}}
                ensure!(next.len()<=MAX_READ_KEYS,"Lifecycle read history is full.");
                s.read_keys=next.into_iter().collect();s.read_keys.sort();
            }
        }
    }
    data.notification_reads=legacy;data.lifecycle_notifications=life;Ok(())
}
