use super::*;
use crate::leave::{
    self,
    credits::{Context, Record},
};
#[derive(Default)]
pub(super) struct State {
    scope: Option<crate::access::Access>,
    person: Option<Uuid>,
    code: String,
    date: String,
    delta: String,
    reason: String,
    reference: String,
    pending: Option<Record>,
    reverse: Option<Uuid>,
    error: Option<String>,
    page: usize,
    posted: bool,
}
impl State {
    fn reconcile(&mut self, data: &Snapshot) {
        if self.scope.as_ref() != Some(&data.access) {
            *self = Self {
                scope: Some(data.access.clone()),
                ..Default::default()
            };
        }
        if let Some(p) = &self.pending {
            if let Some(e) = data
                .leave_setup
                .credits
                .as_ref()
                .and_then(|c| c.entries.iter().find(|e| e.id == p.id))
            {
                if p.matches_entry(e) {
                    self.pending = None;
                    self.delta.clear();
                    self.reason.clear();
                    self.reference.clear();
                    self.reverse = None;
                    self.posted = true;
                    self.error = None;
                } else {
                    self.error=Some("This request identity has a different stored adjustment. Keep the draft and review its history.".into());
                }
            }
        }
    }
}
impl HrisApp {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_leave_credits_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("leave-credits") || self.data.mode != Mode::Demo {
            return;
        }
        self.data.leave.clear();
        self.data.leave_setup.credits = Some(Context::empty(&self.data));
        let own = self.data.employees[0].id;
        let today = self.data.company.today();
        let first=Record{id:Uuid::from_u128(947001),employee_id:own,type_code:"annual".into(),effective_on:today,units:3,reverses_id:None,expected_revision:0,reason:"Fictional reviewed correction to a base entitlement. This example retains a dated reason and the original entry.".into(),reference:"Fictional review reference".into()};
        leave::credits::apply_demo(&mut self.data, &first).unwrap();
        let reverse = Record {
            id: Uuid::from_u128(947002),
            units: -3,
            reverses_id: Some(first.id),
            expected_revision: 1,
            reason: "Fictional full reversal; original history retained.".into(),
            ..first.clone()
        };
        leave::credits::apply_demo(&mut self.data, &reverse).unwrap();
        let scheduled = Record {
            id: Uuid::from_u128(947003),
            units: 1,
            effective_on: today + Duration::days(2),
            expected_revision: 2,
            reason: "Fictional scheduled half-day credit.".into(),
            ..first
        };
        leave::credits::apply_demo(&mut self.data, &scheduled).unwrap();
        if form.contains("manager") {
            self.data.access.role = Role::Manager;
            let c = self.data.leave_setup.credits.as_mut().unwrap();
            c.role = Role::Manager;
            c.audit.clear();
        }
        let mut state = State {
            scope: Some(self.data.access.clone()),
            person: Some(own),
            code: "annual".into(),
            date: today.to_string(),
            ..Default::default()
        };
        if form.contains("draft") {
            state.delta = "2.5".into();
            state.reason =
                "Fictional credit review for checking the entry form and private reference layout."
                    .into();
            state.reference = "Fictional source only".into();
        }
        self.leave_state.credits = state;
        self.leave_state.view = super::leave_view::View::Credits;
        self.page = Page::Leave;
    }
    pub(super) fn leave_credits(&mut self, ui: &mut Ui) {
        if self.data.mode == Mode::Demo && self.data.leave_setup.credits.is_none() {
            self.data.leave_setup.credits = Some(Context::empty(&self.data));
        }
        let mut state = std::mem::take(&mut self.leave_state.credits);
        state.reconcile(&self.data);
        let Some(context) = self.data.leave_setup.credits.as_ref() else {
            empty(ui, "Refresh to load credit history.");
            self.leave_state.credits = state;
            return;
        };
        let data = &self.data;
        let admin = data.access.role.manages_accounts();
        let today = data.company.today();
        let mut command = None;
        if state
            .person
            .is_none_or(|id| !data.employees.iter().any(|e| e.id == id))
        {
            state.person = data
                .access
                .employee_id
                .or_else(|| data.employees.first().map(|e| e.id));
        }
        if state.code.is_empty() {
            state.code = "annual".into();
        }
        if state.date.is_empty() {
            state.date = today.to_string();
        }
        card(ui, |ui| {
            ui.label(
                RichText::new("Leave credit adjustments")
                    .size(18.0)
                    .strong(),
            );
            ui.label("Dated additions or deductions to base entitlement, in whole or half days. These are separate from annual grants and imported opening balances.");
            ui.label(RichText::new("Original entries remain in history. A full linked reversal offsets an original entry once; it does not delete it or approve leave.").small().color(muted(ui)));
            ui.add_enabled_ui(state.pending.is_none() && !self.busy, |ui| {
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("credit-person")
                        .selected_text(
                            state
                                .person
                                .map(|id| data.employee_name(id))
                                .unwrap_or("Choose employee"),
                        )
                        .show_ui(ui, |ui| {
                            for e in &data.employees {
                                if ui
                                    .selectable_value(
                                        &mut state.person,
                                        Some(e.id),
                                        format!("{} · {}", e.employee_code, e.full_name),
                                    )
                                    .changed()
                                {
                                    state.page = 0;
                                }
                            }
                        });
                    egui::ComboBox::from_id_salt("credit-type")
                        .selected_text(
                            data.leave_setup
                                .policy
                                .leave_type(&state.code)
                                .map(|t| t.name.as_str())
                                .unwrap_or("Choose finite leave type"),
                        )
                        .show_ui(ui, |ui| {
                            for t in data
                                .leave_setup
                                .policy
                                .types
                                .iter()
                                .filter(|t| t.accrual != leave::Accrual::Unlimited)
                            {
                                if ui
                                    .selectable_value(&mut state.code, t.code.clone(), &t.name)
                                    .changed()
                                {
                                    state.page = 0;
                                }
                            }
                        });
                });
            });
            if let (Some(person), Some(kind)) = (
                state
                    .person
                    .and_then(|id| data.employees.iter().find(|e| e.id == id)),
                data.leave_setup.policy.leave_type(&state.code),
            ) {
                let b = leave::balance(data, person, kind, today.year(), today, None);
                let adjustments = context.units(person.id, &kind.code, today.year(), today);
                ui.label(format!(
                    "Today: {} available · {} used · {} reserved · {} carryover day(s)",
                    leave::days(b.available_units),
                    leave::days(b.used_units),
                    leave::days(b.reserved_units),
                    leave::days(b.carryover_units)
                ));
                ui.label(format!(
                    "Applied base adjustments this year: {} day(s)",
                    leave::days(adjustments)
                ));
            }
            if state.posted {
                ui.colored_label(
                    accent(ui),
                    "Adjustment recorded. The original entry is retained.",
                );
            }
            if admin {
                if let Some(pending) = state.pending.clone() {
                    ui.label("Keep this draft until its result is confirmed. Retrying keeps the same request identity.");
                    if context.revision != pending.expected_revision {
                        ui.label("Credit history changed. Review the balance above before retrying with the current revision.");
                    }
                    if ui
                        .add_enabled(
                            !self.busy,
                            egui::Button::new(if context.revision == pending.expected_revision {
                                "Retry same adjustment"
                            } else {
                                "Review current balance and retry"
                            }),
                        )
                        .clicked()
                    {
                        let mut retry = pending;
                        retry.expected_revision = context.revision;
                        state.pending = Some(retry.clone());
                        command = Some(retry);
                    }
                } else {
                    ui.add_enabled_ui(!self.busy,|ui|{
                        ui.horizontal_wrapped(|ui|{ui.label("Effective date");ui.add(egui::TextEdit::singleline(&mut state.date).desired_width(130.0).hint_text("YYYY-MM-DD"));ui.label("Days to add or deduct");ui.add(egui::TextEdit::singleline(&mut state.delta).desired_width(115.0).hint_text("2 or -0.5"));});
                        if state.reverse.is_some(){ui.label("This draft reverses the full selected original adjustment. Keep its employee, type and opposite number of days.");}
                        ui.label("Reason (required)");ui.add(egui::TextEdit::multiline(&mut state.reason).desired_rows(2).desired_width(f32::INFINITY));
                        ui.label("Supporting reference (optional, private to HR/Admin)");ui.add(egui::TextEdit::singleline(&mut state.reference).desired_width(f32::INFINITY));
                        ui.horizontal_wrapped(|ui|{
                            if ui.button("Post adjustment").clicked(){
                                let result=(||->anyhow::Result<Record>{let r=Record{id:Uuid::new_v4(),employee_id:state.person.ok_or_else(||anyhow::anyhow!("Choose an employee."))?,type_code:state.code.clone(),effective_on:parse_date(&state.date)?,units:leave::credits::parse_delta(&state.delta)?,reverses_id:state.reverse,expected_revision:context.revision,reason:state.reason.trim().into(),reference:state.reference.trim().into()};r.validate(data)?;Ok(r)})();
                                match result{Ok(r)=>{state.pending=Some(r.clone());state.error=None;state.posted=false;command=Some(r);},Err(e)=>state.error=Some(e.to_string())}
                            }
                            if ui.button("Discard unsaved draft").clicked(){state.delta.clear();state.reason.clear();state.reference.clear();state.reverse=None;state.error=None;}
                        });
                    });
                }
            } else {
                ui.label(RichText::new("Administrators record adjustments. This view shows only the employee records within your current access.").small().color(muted(ui)));
            }
            if let Some(error) = &state.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
        let mut rows = context
            .entries
            .iter()
            .filter(|e| Some(e.employee_id) == state.person && e.type_code == state.code)
            .collect::<Vec<_>>();
        rows.reverse();
        let pages = rows.len().div_ceil(30).max(1);
        state.page = state.page.min(pages - 1);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} retained entries · Page {} of {}",
                rows.len(),
                state.page + 1,
                pages
            ));
            if ui
                .add_enabled(state.page > 0, egui::Button::new("Previous"))
                .clicked()
            {
                state.page -= 1;
            }
            if ui
                .add_enabled(state.page + 1 < pages, egui::Button::new("Next"))
                .clicked()
            {
                state.page += 1;
            }
        });
        for e in rows.into_iter().skip(state.page * 30).take(30) {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}{} day(s) · Effective {}",
                            if e.units > 0 { "+" } else { "" },
                            leave::days(e.units),
                            e.effective_on
                        ))
                        .strong(),
                    );
                    badge(
                        ui,
                        if e.effective_on > today {
                            "Scheduled"
                        } else {
                            "Applied"
                        },
                        e.effective_on <= today,
                    );
                });
                if e.reverses_id.is_some() {
                    ui.label("Linked full reversal of an earlier adjustment.");
                }
                if let Some(a) = context.audit.iter().find(|a| a.id == e.id) {
                    ui.label(format!(
                        "{} · {}",
                        a.created_by_name,
                        a.posted_at
                            .with_timezone(&data.company.tz())
                            .format("%Y-%m-%d %H:%M")
                    ));
                    ui.label(&a.reason);
                    if !a.reference.is_empty() {
                        ui.label(format!("Reference: {}", a.reference));
                    }
                }
                let reversed = context.entries.iter().any(|r| r.reverses_id == Some(e.id));
                if reversed {
                    ui.label("A linked reversal is retained in history.");
                }
                if admin
                    && e.reverses_id.is_none()
                    && !reversed
                    && e.effective_on.year() >= today.year()
                {
                    let draft_empty = state.pending.is_none()
                        && state.delta.is_empty()
                        && state.reason.is_empty()
                        && state.reference.is_empty();
                    if ui
                        .add_enabled(
                            !self.busy && draft_empty,
                            egui::Button::new("Prepare full reversal"),
                        )
                        .clicked()
                    {
                        state.delta = leave::days(-e.units);
                        state.date = today.max(e.effective_on).to_string();
                        state.reverse = Some(e.id);
                        state.reason.clear();
                        state.reference.clear();
                        state.posted = false;
                    }
                }
            });
        }
        self.leave_state.credits = state;
        if let Some(record) = command {
            self.send(Command::Change(Action::LeaveCredit(record)));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn draft(data: &Snapshot) -> Record {
        Record {
            id: Uuid::from_u128(947100),
            employee_id: data.employees[0].id,
            type_code: "annual".into(),
            effective_on: data.company.today(),
            units: 1,
            reverses_id: None,
            expected_revision: 0,
            reason: "Fictional private draft".into(),
            reference: "Fictional reference".into(),
        }
    }
    #[test]
    fn unrelated_refresh_retains_pending_identity_and_only_matching_source_acknowledges_it() {
        let mut data = DemoStore::new().data;
        data.leave_setup.credits = Some(Context::empty(&data));
        let r = draft(&data);
        let mut s = State {
            scope: Some(data.access.clone()),
            delta: "0.5".into(),
            reason: r.reason.clone(),
            pending: Some(r.clone()),
            ..Default::default()
        };
        s.reconcile(&data);
        assert_eq!(s.pending.as_ref().unwrap().id, r.id);
        assert_eq!(s.reason, r.reason);
        let mut wrong = crate::leave::credits::Entry {
            id: r.id,
            employee_id: r.employee_id,
            type_code: r.type_code.clone(),
            effective_on: r.effective_on,
            units: 2,
            reverses_id: None,
        };
        data.leave_setup
            .credits
            .as_mut()
            .unwrap()
            .entries
            .push(wrong.clone());
        s.reconcile(&data);
        assert!(s.pending.is_some());
        assert!(s.error.is_some());
        wrong.units = 1;
        data.leave_setup.credits.as_mut().unwrap().entries[0] = wrong;
        s.reconcile(&data);
        assert!(s.pending.is_none());
        assert!(s.reason.is_empty());
        assert!(s.posted);
    }
    #[test]
    fn role_change_clears_private_credit_draft() {
        let mut data = DemoStore::new().data;
        let mut s = State {
            scope: Some(data.access.clone()),
            reason: "Fictional private reason".into(),
            pending: Some(draft(&data)),
            ..Default::default()
        };
        data.access.role = Role::Manager;
        s.reconcile(&data);
        assert!(s.reason.is_empty());
        assert!(s.pending.is_none());
        assert_eq!(s.scope, Some(data.access));
    }
}
