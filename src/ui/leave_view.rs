use super::*;
use anyhow::Context;
use crate::leave::{
    self, Accrual, ApprovalRule, ApproverKind, DayPart, Holiday, LeavePolicy, LeaveType,
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum View {
    #[default]
    Requests,
    Policies,
    Credits,
    Openings,
    Delegation,
}
#[derive(Default)]
pub(super) struct State {
    pub openings:super::leave_openings_view::State,
    pub policy_pending:Option<leave::dated::Change>,
    pub policy_date:String,
    pub policy_reason:String,
    pub policy_reference:String,
    pub withdraw_target:Option<Uuid>,
    pub delegation:super::approval_delegation_view::State,
    pub credits:super::leave_credits_view::State,
    pub show_inactive_types: bool,
    pub saving_revision: Option<i64>,
    pub view: View,
    pub draft: Option<LeavePolicy>,
    pub detail: Option<Uuid>,
    pub type_index: usize,
    pub holiday_date: String,
    pub holiday_name: String,
    pub review_note: String,
    pub cancel_reason: String,
}
impl HrisApp {
    pub(super) fn leave_page(&mut self, ui: &mut Ui) {
        if self.data.mode==Mode::Demo && self.data.leave_setup.dated.is_none(){self.data.leave_setup.dated=Some(leave::dated::Context::empty(&self.data));}
        if self.leave_state.policy_pending.as_ref().is_some_and(|p|self.data.leave_setup.dated.as_ref().is_some_and(|c|p.matches(c))) {self.leave_state.policy_pending=None;self.leave_state.draft=None;self.leave_state.withdraw_target=None;self.leave_state.policy_reason.clear();self.leave_state.policy_reference.clear();}
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.leave_state.view, View::Requests, "Requests");
            ui.selectable_value(&mut self.leave_state.view, View::Policies, "Leave policies");
            ui.selectable_value(&mut self.leave_state.view, View::Credits, "Credit adjustments");
            ui.selectable_value(&mut self.leave_state.view, View::Openings, "Opening balances");
            ui.selectable_value(&mut self.leave_state.view, View::Delegation, "Approval delegation");
        });
        ui.add_space(10.0);
        match self.leave_state.view {
            View::Requests => self.leave_requests(ui),
            View::Policies => self.leave_policies(ui),
            View::Credits => self.leave_credits(ui),
            View::Openings => self.leave_openings(ui),
            View::Delegation => self.approval_delegations(ui),
        }
    }
    fn leave_requests(&mut self, ui: &mut Ui) {
        if let Some(id) = self.leave_state.detail {
            self.leave_detail(ui, id);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.leave_filter, None, "All");
            for s in [
                LeaveStatus::Pending,
                LeaveStatus::Approved,
                LeaveStatus::Rejected,
                LeaveStatus::Cancelled,
            ] {
                ui.selectable_value(&mut self.leave_filter, Some(s), s.label());
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("+ Request leave"))
                .clicked()
            {
                self.new_leave();
            }
        });
        ui.label(RichText::new("Pending requests reserve days. Only the final approval deducts leave; cancellation retains history and releases the days.").small().color(muted(ui)));
        let rows = self
            .data
            .leave
            .iter()
            .filter(|r| self.leave_filter.is_none_or(|s| s == r.status))
            .cloned()
            .collect::<Vec<_>>();
        if rows.is_empty() {
            card(ui, |ui| empty(ui, "No leave requests in this view."));
        }
        for row in rows {
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(self.leave_employee_name(&row)).strong());
                        badge(ui, row.status.label(), row.status == LeaveStatus::Approved);
                        ui.label(row.type_name());
                    });
                    ui.label(format!(
                        "{} to {} · {} day(s) · {}",
                        row.starts_on,
                        row.ends_on,
                        leave::days(row.units()),
                        row.policy.day_part.label()
                    ));
                    ui.horizontal_wrapped(|ui| {
                        if row.status == LeaveStatus::Pending {
                            let total = row.policy.approval_steps.len().max(1);
                            let done = row
                                .policy
                                .approval_steps
                                .iter()
                                .filter(|s| s.approved_by.is_some())
                                .count();
                            ui.label(
                                RichText::new(format!(
                                    "Approval step {} of {total}",
                                    (done + 1).min(total)
                                ))
                                .small()
                                .color(muted(ui)),
                            );
                        }
                        if ui
                            .button(if leave::can_review(&self.data, &row) {
                                "Review request"
                            } else {
                                "Details / history"
                            })
                            .clicked()
                        {
                            self.leave_state.detail = Some(row.id);
                            self.leave_state.review_note.clear();
                            self.leave_state.cancel_reason.clear();
                        }
                    });
                });
            });
        }
    }
    fn leave_employee_name(&self, row: &LeaveRequest) -> String {
        let name = self.data.employee_name(row.employee_id);
        if name == "Unknown employee" && !row.policy.requester_name.is_empty() {
            row.policy.requester_name.clone()
        } else {
            name.into()
        }
    }
    fn leave_detail(&mut self, ui: &mut Ui, id: Uuid) {
        self.delegated_review_history(ui,crate::approvals::Scope::Leave,id);
        if ui.button("Back to requests").clicked() {
            self.leave_state.detail = None;
            return;
        }
        let Some(row) = self.data.leave.iter().find(|r| r.id == id).cloned() else {
            empty(
                ui,
                "This request is no longer in your current access scope. Refresh the list.",
            );
            return;
        };
        card(ui, |ui| {
            ui.label(
                RichText::new(self.leave_employee_name(&row))
                    .size(20.0)
                    .strong(),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label(row.type_name());
                badge(ui, row.status.label(), row.status == LeaveStatus::Approved);
            });
            ui.label(format!(
                "{} to {} · {} day(s) · {}",
                row.starts_on,
                row.ends_on,
                leave::days(row.units()),
                row.policy.day_part.label()
            ));
            if !row.reason.is_empty() {
                ui.label(egui::RichText::new(&row.reason));
            }
            ui.label(RichText::new(format!("Saved policy revision {} · request revision {}. Saved dates and charges are retained when policies change.",row.policy.policy_revision,row.policy.revision)).small().color(muted(ui)));
            ui.add_space(8.0);
            ui.label(RichText::new("Approval chain").strong());
            if row.policy.approval_steps.is_empty() {
                ui.label("Earlier final decision; approval steps were not reconstructed.");
            }
            for (i, s) in row.policy.approval_steps.iter().enumerate() {
                let person = s
                    .employee_id
                    .map(|id| format!(" · {}", self.data.employee_name(id)))
                    .unwrap_or_default();
                ui.label(format!(
                    "{}. {}{} — {}",
                    i + 1,
                    s.kind.label(),
                    person,
                    if s.approved_by.is_some() {
                        "Approved"
                    } else if row.status == LeaveStatus::Pending {
                        "Awaiting approval"
                    } else {
                        "Not completed"
                    }
                ));
            }
            ui.add_enabled_ui(!self.busy, |ui| {
                if leave::can_review(&self.data, &row) {
                    field(
                        ui,
                        "Review note (optional)",
                        &mut self.leave_state.review_note,
                        "Up to 500 characters",
                    );
                    ui.horizontal_wrapped(|ui| {
                        if primary(ui, "Approve this step") {
                            self.send(Command::Change(Action::ReviewLeave(
                                id,
                                row.policy.revision,
                                true,
                                self.leave_state.review_note.clone(),
                            )));
                        }
                        if ui.button("Reject request").clicked() {
                            self.send(Command::Change(Action::ReviewLeave(
                                id,
                                row.policy.revision,
                                false,
                                self.leave_state.review_note.clone(),
                            )));
                        }
                    });
                }
                if row.live()
                    && (self.data.access.allows(crate::staff_roles::Capability::LeaveAdministration)
                        || (self.data.access.employee_id == Some(row.employee_id)
                            && (row.status == LeaveStatus::Pending
                                || row.starts_on >= self.data.company.today())))
                {
                    ui.separator();
                    field(
                        ui,
                        "Cancellation reason",
                        &mut self.leave_state.cancel_reason,
                        "Required; history is retained",
                    );
                    if ui
                        .add_enabled(
                            !self.leave_state.cancel_reason.trim().is_empty(),
                            egui::Button::new("Cancel leave request"),
                        )
                        .clicked()
                    {
                        self.send(Command::Change(Action::CancelLeave(
                            id,
                            row.policy.revision,
                            self.leave_state.cancel_reason.clone(),
                        )));
                    }
                }
            });
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Request history").size(18.0).strong());
            let mut events = self
                .data
                .leave_setup
                .events
                .iter()
                .filter(|e| e.request_id == id)
                .collect::<Vec<_>>();
            events.sort_by_key(|e| std::cmp::Reverse((e.created_at, e.id)));
            for e in events {
                ui.separator();
                ui.label(format!(
                    "{} · {} · {}",
                    e.created_at
                        .with_timezone(&self.data.company.tz())
                        .format("%Y-%m-%d %H:%M"),
                    e.actor_name,
                    e.action.replace('_', " ")
                ));
                if !e.note.is_empty() {
                    ui.label(&e.note);
                }
            }
        });
    }
    fn leave_policies(&mut self, ui: &mut Ui) {
        let admin = self.data.access.role.manages_accounts();
        self.dated_policy_history(ui);
        if self.leave_state.draft.is_none() {
            let p = &self.data.leave_setup.policy;
            card(ui, |ui| {
                ui.label(RichText::new("Company leave policies").size(18.0).strong());
                ui.label(format!(
                    "Revision {} · {} dated holidays · {} active leave types",
                    p.revision,
                    p.holidays.len(),
                    p.types.iter().filter(|t| t.active).count()
                ));
                let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
                ui.label(format!(
                    "Workweek: {}",
                    p.workweek
                        .iter()
                        .filter_map(|d| labels.get(*d as usize - 1).copied())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                ui.label("Monthly accrual posts at month-end for full calendar months employed, rounded down to half-days. Carryover uses unused prior-year base grant, is consumed before its expiry month ends, and is not carried twice.");
                ui.label(RichText::new(format!("Balance tracking starts in {}. A known Date joined is required for monthly accrual and prior-year carryover.",p.tracking_starts_year)).small().color(muted(ui)));
                if admin && ui.button("Schedule policy / approval-chain version").clicked() {
                    let tail=self.data.leave_setup.dated.as_ref().and_then(|c|c.timeline.versions.iter().rev().find(|v|!v.withdrawn));
                    self.leave_state.draft = Some(tail.map_or_else(||p.clone(),|v|v.policy.clone()));
                    self.leave_state.policy_date=tail.filter(|v|v.effective_on>=self.data.company.today()).map_or(self.data.company.today(),|v|v.effective_on.succ_opt().unwrap()).to_string();
                    self.leave_state.policy_reason.clear();self.leave_state.policy_reference.clear();
                }
            });
            if p.types.iter().any(|t| !t.active) {
                ui.checkbox(
                    &mut self.leave_state.show_inactive_types,
                    "Include inactive leave types",
                );
            }
            for t in p
                .types
                .iter()
                .filter(|t| t.active || self.leave_state.show_inactive_types)
            {
                ui.add_space(8.0);
                card(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&t.name).strong());
                        badge(ui, if t.active { "Active" } else { "Inactive" }, t.active);
                        ui.label(if t.paid { "Paid" } else { "Unpaid" });
                    });
                    if t.accrual == Accrual::Unlimited {
                        ui.label("No balance limit · approval required");
                    } else {
                        ui.label(format!(
                        "{} · allowance: {} · carryover up to {} day(s), expires at the end of {}",
                        t.accrual.label(),
                        if t.use_employee_allowance {
                            "employee annual allowance".into()
                        } else {
                            format!("{} day(s)", leave::days(t.annual_units))
                        },
                        leave::days(t.carryover_units),
                        month_name(t.carryover_expiry_month)
                    ));
                    }
                    ui.label(format!(
                        "Approval: {}",
                        t.approval_chain
                            .iter()
                            .map(|s| s.kind.label())
                            .collect::<Vec<_>>()
                            .join(" > ")
                    ));
                });
            }
            if !p.holidays.is_empty() {
                ui.add_space(8.0);
                card(ui, |ui| {
                    ui.label(RichText::new("Holiday calendar").strong());
                    for h in &p.holidays {
                        ui.label(format!("{} · {}", h.date, h.name));
                    }
                });
            }
            return;
        }
        if !admin {
            self.leave_state.draft = None;
            return;
        }
        let mut p = self.leave_state.draft.take().unwrap();
        let mut cancel = false;
        let mut save = false;
        ui.add_enabled_ui(!self.busy && self.leave_state.policy_pending.is_none(),|ui|{
            card(ui,|ui|{
                ui.heading("Effective-date policy version");
                field(ui,"Effective date",&mut self.leave_state.policy_date,"YYYY-MM-DD");
                ui.label("Calendar, labels and approvals can change today or later. Annual grant, accrual, carry and new finite entitlements start on a future January 1. Use Credit adjustments for current-year corrections.");
                ui.label("Existing request charges and approvers are retained. Employee allowance edits become future defaults once this company's dated workflow is enabled.");
                field(ui,"Private reason",&mut self.leave_state.policy_reason,"Explain the scheduled policy version");
                field(ui,"Private reference (optional)",&mut self.leave_state.policy_reference,"Policy review reference");
            });
            ui.add_space(10.0);
            card(ui,|ui|{
                ui.label(RichText::new("Workweek and holidays").size(18.0).strong());
                ui.label("These settings calculate new requests. Existing saved day charges stay unchanged.");
                ui.horizontal_wrapped(|ui|{for (i,name) in ["Monday","Tuesday","Wednesday","Thursday","Friday","Saturday","Sunday"].iter().enumerate(){let d=i as u32+1;let mut checked=p.workweek.contains(&d);if ui.checkbox(&mut checked,*name).changed(){if checked{p.workweek.push(d);p.workweek.sort_unstable();}else{p.workweek.retain(|x|*x!=d);}}}});
                ui.add_space(6.0);
                field(ui,"Holiday date",&mut self.leave_state.holiday_date,"YYYY-MM-DD");field(ui,"Holiday name",&mut self.leave_state.holiday_name,"Company holiday");
                if ui.button("Add holiday").clicked(){match parse_date(&self.leave_state.holiday_date){Ok(date) if !self.leave_state.holiday_name.trim().is_empty()&&!p.holidays.iter().any(|h|h.date==date)=>{p.holidays.push(Holiday{date,name:self.leave_state.holiday_name.trim().into()});p.holidays.sort_by_key(|h|h.date);self.leave_state.holiday_date.clear();self.leave_state.holiday_name.clear();},_=>self.notice=Some(("Enter a unique holiday date and a name.".into(),true))}}
                let mut remove=None;
                egui::ScrollArea::vertical().id_salt("policy_holidays").max_height(180.0).show(ui,|ui|{for (i,h) in p.holidays.iter().enumerate(){ui.horizontal_wrapped(|ui|{ui.label(format!("{} · {}",h.date,h.name));if ui.small_button("Remove").clicked(){remove=Some(i);}});}});
                if let Some(i)=remove{p.holidays.remove(i);}
            });
            ui.add_space(10.0);
            card(ui,|ui|{
                ui.label(RichText::new("Leave types and approval chains").size(18.0).strong());
                ui.horizontal_wrapped(|ui|{
                    egui::ComboBox::from_id_salt("policy_type").width(240.0).truncate().selected_text(p.types.get(self.leave_state.type_index).map(|t|t.name.as_str()).unwrap_or("Choose type")).show_ui(ui,|ui|{for (i,t) in p.types.iter().enumerate(){ui.selectable_value(&mut self.leave_state.type_index,i,&t.name);}});
                    if p.types.len()<30&&ui.button("+ Add leave type").clicked(){let mut i=p.types.len()+1;while p.types.iter().any(|t|t.code==format!("type_{i}")){i+=1;}p.types.push(LeaveType{code:format!("type_{i}"),name:"New leave type".into(),..Default::default()});self.leave_state.type_index=p.types.len()-1;}
                });
                self.leave_state.type_index=self.leave_state.type_index.min(p.types.len().saturating_sub(1));
                if let Some(t)=p.types.get_mut(self.leave_state.type_index){
                    field(ui,"Leave type name",&mut t.name,"e.g. Bereavement leave");
                    ui.horizontal_wrapped(|ui|{ui.checkbox(&mut t.active,"Available for new requests");ui.checkbox(&mut t.paid,"Paid leave");});
                    egui::ComboBox::from_id_salt("policy_accrual").selected_text(t.accrual.label()).show_ui(ui,|ui|{for a in [Accrual::Upfront,Accrual::Monthly,Accrual::Unlimited]{ui.selectable_value(&mut t.accrual,a,a.label());}});
                    if t.code=="annual"{ui.checkbox(&mut t.use_employee_allowance,"Use each employee's annual allowance");}
                    ui.add_enabled_ui(t.accrual!=Accrual::Unlimited,|ui|{
                        if !t.use_employee_allowance {units_field(ui,"Yearly allowance (days)",&mut t.annual_units);}
                        units_field(ui,"Maximum carryover (days)",&mut t.carryover_units);
                        ui.horizontal_wrapped(|ui|{ui.label("Carryover expires at end of");egui::ComboBox::from_id_salt("carry_month").selected_text(month_name(t.carryover_expiry_month)).show_ui(ui,|ui|{for m in 1..=12{ui.selectable_value(&mut t.carryover_expiry_month,m,month_name(m));}});});
                    });
                    ui.separator();ui.label(RichText::new("Ordered approval steps").strong());
                    ui.label(RichText::new("Each step must approve in order. Self-approval is blocked. Manager and specific-person assignments are saved with each request; changing this chain affects new requests.").small().color(muted(ui)));
                    let mut movement=None;let mut removal=None;
                    let chain_len=t.approval_chain.len();
                    for (i,s) in t.approval_chain.iter_mut().enumerate(){ui.push_id(i,|ui|{
                        ui.horizontal_wrapped(|ui|{
                            ui.label(format!("Step {}",i+1));
                            egui::ComboBox::from_id_salt("approver_kind").width(180.0).truncate().selected_text(s.kind.label()).show_ui(ui,|ui|{for k in [ApproverKind::Reviewer,ApproverKind::Manager,ApproverKind::Hr,ApproverKind::Administrator,ApproverKind::Person]{ui.selectable_value(&mut s.kind,k,k.label());}});
                            if ui.add_enabled(i>0,egui::Button::new("Move up").small()).clicked(){movement=Some(i);}
                            if ui.add_enabled(chain_len>1,egui::Button::new("Remove").small()).clicked(){removal=Some(i);}
                        });
                        if s.kind==ApproverKind::Person {egui::ComboBox::from_id_salt("approver_person").selected_text(s.employee_id.map(|id|self.data.employee_name(id)).unwrap_or("Choose staff member")).show_ui(ui,|ui|{if self.data.mode==Mode::Demo {for account in self.data.accounts.iter().filter(|a|a.active&&a.role.uses_desktop()){if let Some(id)=account.employee_id{ui.selectable_value(&mut s.employee_id,Some(id),self.data.employee_name(id));}}}else{for person in self.data.recruiting.interviewers.iter().filter(|p|p.allows(crate::staff_roles::Capability::Desktop)){ui.selectable_value(&mut s.employee_id,Some(person.employee_id),&person.full_name);}}});}else{s.employee_id=None;}
                    });}
                    if let Some(i)=movement{t.approval_chain.swap(i,i-1);}
                    if let Some(i)=removal{if t.approval_chain.len()>1{t.approval_chain.remove(i);}}
                    if t.approval_chain.len()<5&&ui.button("+ Add approval step").clicked(){t.approval_chain.push(ApprovalRule{kind:ApproverKind::Hr,employee_id:None});}
                }
            });
            ui.add_space(10.0);ui.horizontal_wrapped(|ui|{save=primary(ui,"Post dated policy version");cancel=ui.button("Discard policy edits").clicked();});
            if let Some((message,true))=&self.notice {ui.colored_label(ui.visuals().error_fg_color,message);}
        });
        if save {
            let change=(||->anyhow::Result<leave::dated::Change>{let c=self.data.leave_setup.dated.as_ref().context("Refresh dated leave setup.")?;
                let change=leave::dated::Change::Schedule(leave::dated::PolicyRecord{id:Uuid::new_v4(),effective_on:parse_date(&self.leave_state.policy_date)?,expected_revision:c.revision,policy:p.clone(),reason:self.leave_state.policy_reason.trim().into(),reference:self.leave_state.policy_reference.clone()});change.payload(&self.data)?;Ok(change)})();
            match change{Ok(change)=>{self.leave_state.policy_pending=Some(change.clone());self.send(Command::Change(Action::DatedLeave(change)));},Err(e)=>self.notice=Some((e.to_string(),true))}
        }
        if !cancel {
            self.leave_state.draft = Some(p);
        }
    }
    pub(super) fn leave_request_fields(&self, ui: &mut Ui, draft: &mut LeaveDraft) {
        ui.label("Employee");
        employee_picker(ui, "leave_employee", &self.data, &mut draft.employee_id);
        let code = if draft.options.type_code.is_empty() {
            match draft.kind {
                LeaveKind::Annual => "annual",
                LeaveKind::Sick => "sick",
                LeaveKind::Unpaid => "unpaid",
                LeaveKind::Custom => "custom",
            }
            .to_owned()
        } else {
            draft.options.type_code.clone()
        };
        let request_policy=leave::dated::policy_on(&self.data,parse_date(&draft.starts_on).unwrap_or(self.data.company.today()));
        let selected = request_policy
            .leave_type(&code)
            .map(|t| t.name.as_str())
            .unwrap_or("Choose a leave type");
        ui.label("Leave type");
        egui::ComboBox::from_id_salt("request_type")
            .width(240.0)
            .truncate()
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for t in request_policy.types
                    .iter()
                    .filter(|t| t.active)
                {
                    ui.selectable_value(&mut draft.options.type_code, t.code.clone(), &t.name);
                }
            });
        ui.label("Duration");
        egui::ComboBox::from_id_salt("leave_part")
            .selected_text(draft.options.day_part.label())
            .show_ui(ui, |ui| {
                for p in [DayPart::Full, DayPart::Am, DayPart::Pm] {
                    ui.selectable_value(&mut draft.options.day_part, p, p.label());
                }
            });
        field(ui, "Start date", &mut draft.starts_on, "YYYY-MM-DD");
        if draft.options.day_part == DayPart::Full {
            field(ui, "End date", &mut draft.ends_on, "YYYY-MM-DD");
        } else {
            draft.ends_on = draft.starts_on.clone();
        }
        if let (Ok(start), Ok(end)) = (parse_date(&draft.starts_on), parse_date(&draft.ends_on)) {
            match leave::dated::charges(&self.data,&code,start,end,draft.options.day_part)
            {
                Ok(charges) => {
                    let units = charges.iter().map(|d| d.units).sum();
                    ui.colored_label(
                        accent(ui),
                        format!(
                            "{} day(s), excluding holidays and non-workdays",
                            leave::days(units)
                        ),
                    );
                }
                Err(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, e.to_string());
                }
            }
            if let (Some(e), Some(t)) = (
                self.data
                    .employees
                    .iter()
                    .find(|e| Some(e.id) == draft.employee_id),
                request_policy.leave_type(&code),
            ) {
                if t.accrual != Accrual::Unlimited {
                    match leave::balance_checked(&self.data,e,t,start.year(),start,None){Ok(b)=>{ui.label(format!("Balance at {start}: {} available · {} used · {} reserved · {} carryover day(s)",leave::days(b.available_units),leave::days(b.used_units),leave::days(b.reserved_units),leave::days(b.carryover_units)));},Err(e)=>{ui.colored_label(ui.visuals().error_fg_color,e.to_string());}}
                }
                ui.label(
                    RichText::new(format!(
                        "Approval: {}",
                        t.approval_chain
                            .iter()
                            .map(|s| s.kind.label())
                            .collect::<Vec<_>>()
                            .join(" > ")
                    ))
                    .small()
                    .color(muted(ui)),
                );
            }
        }
    }
}
fn units_field(ui: &mut Ui, label: &str, value: &mut i32) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        let mut days = *value as f64 / 2.0;
        if ui
            .add(
                egui::DragValue::new(&mut days)
                    .speed(0.5)
                    .range(0.0..=366.0)
                    .fixed_decimals(1),
            )
            .changed()
        {
            *value = (days * 2.0).round() as i32;
        }
    });
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_leave_preview(&mut self) {
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("leave-") || self.data.mode != Mode::Demo {
            return;
        }
        self.page = Page::Leave;
        self.data.leave.clear();
        for e in &mut self.data.employees {
            e.joined_on = None;
        }
        let today = self.data.company.today();
        let year = today.year();
        self.data.leave_setup.policy.workweek = vec![1, 2, 3, 4, 5, 6];
        self.data.leave_setup.policy.holidays = vec![Holiday {
            date: NaiveDate::from_ymd_opt(year, 12, 25).unwrap(),
            name: "Fictional company holiday".into(),
        }];
        self.data.leave_setup.policy.types[0].carryover_units = 10;
        self.data.leave_setup.policy.types[1].accrual = Accrual::Monthly;
        self.data.leave_setup.policy.types[1].annual_units = 24;
        self.data.leave_setup.policy.types[0].approval_chain = vec![
            ApprovalRule {
                kind: ApproverKind::Hr,
                employee_id: None,
            },
            ApprovalRule {
                kind: ApproverKind::Administrator,
                employee_id: None,
            },
        ];
        self.data.leave_setup.policy.types.push(LeaveType {
            code: "bereavement".into(),
            name: "Bereavement leave".into(),
            annual_units: 10,
            carryover_units: 0,
            approval_chain: vec![
                ApprovalRule {
                    kind: ApproverKind::Manager,
                    employee_id: None,
                },
                ApprovalRule {
                    kind: ApproverKind::Hr,
                    employee_id: None,
                },
                ApprovalRule {
                    kind: ApproverKind::Administrator,
                    employee_id: None,
                },
            ],
            ..Default::default()
        });
        if matches!(
            preview.as_str(),
            "leave-policies" | "leave-calendar" | "leave-types" | "leave-approvals"
        ) {
            self.leave_state.view = View::Policies;
            if preview != "leave-policies" {
                self.leave_state.draft = Some(self.data.leave_setup.policy.clone());
            }
            if preview == "leave-types" {
                self.leave_state.type_index = 1;
            }
            if preview == "leave-approvals" {
                self.leave_state.type_index = 3;
            }
            return;
        }
        if preview == "leave-empty" {
            return;
        }
        let person = self
            .data
            .employees
            .iter()
            .find(|e| e.id.as_u128() == 103)
            .or_else(|| {
                self.data
                    .active()
                    .find(|e| Some(e.id) != self.data.access.employee_id)
            })
            .map(|e| e.id);
        let Some(person) = person else {
            return;
        };
        let date = (0..7)
            .map(|i| today + Duration::days(i))
            .find(|d| self.data.leave_setup.policy.working(*d))
            .unwrap();
        let draft = LeaveDraft {
            employee_id: Some(person),
            starts_on: date.to_string(),
            ends_on: date.to_string(),
            reason: "Fictional request for layout verification.".into(),
            options: leave::LeaveOptions {
                type_code: "annual".into(),
                day_part: DayPart::Am,
            },
            ..Default::default()
        };
        if preview == "leave-half-form" {
            self.leave_form = Some(draft);
            return;
        }
        if let Ok(row) = leave::prepare(&self.data, &draft) {
            let id = row.id;
            self.data.leave.push(row);
            leave::demo_event(&mut self.data, id, "submitted", "Fictional submission");
            if preview == "leave-cancelled" {
                let _ = leave::apply_demo(
                    &mut self.data,
                    Action::CancelLeave(id, 0, "Fictional cancellation; plans changed.".into()),
                );
            }
            if preview == "leave-reviewed" {
                let _ = leave::apply_demo(
                    &mut self.data,
                    Action::ReviewLeave(id, 0, true, "Fictional first-stage approval.".into()),
                );
            }
            if matches!(
                preview.as_str(),
                "leave-detail" | "leave-reviewed" | "leave-cancelled"
            ) {
                self.leave_state.detail = Some(id);
            }
        }
    }
}

fn month_name(month: u32) -> &'static str {
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ]
    .get(month.saturating_sub(1) as usize)
    .copied()
    .unwrap_or("Not set")
}
