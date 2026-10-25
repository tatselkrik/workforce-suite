use super::*;
use crate::leave::dated::{Change, WithdrawRecord};
impl HrisApp {
    pub(super) fn dated_policy_history(&mut self, ui: &mut Ui) {
        let Some(c) = self.data.leave_setup.dated.clone() else {
            return;
        };
        let admin = self.data.access.role.manages_accounts();
        if let Some(change) = self.leave_state.policy_pending.clone() {
            card(ui, |ui| {
                ui.label("Policy action awaiting verification. Refresh or retry the same action; its request identity is retained.");
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("Retry same policy action"))
                        .clicked()
                    {
                        self.send(Command::Change(Action::DatedLeave(change.clone())));
                    }
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("Refresh and revise draft"))
                        .clicked()
                    {
                        self.leave_state.policy_pending = None;
                        self.send(Command::Refresh);
                    }
                });
            });
        }
        if !c.timeline.versions.is_empty() {
            card(ui, |ui| {
                ui.label(
                    RichText::new("Policy schedule and retained history")
                        .size(18.0)
                        .strong(),
                );
                ui.label("Policies activate by company date. Earlier filed requests keep their original dates, charges and approval plan.");
                for v in c.timeline.versions.iter().rev() {
                    let state = if v.withdrawn {
                        "Withdrawn"
                    } else if v.effective_on > c.as_of {
                        "Scheduled"
                    } else {
                        "Effective"
                    };
                    egui::CollapsingHeader::new(format!(
                        "{} · revision {} · {state}",
                        v.effective_on, v.policy.revision
                    ))
                    .id_salt(("leave_policy_version", v.id))
                    .show(ui, |ui| {
                        ui.label(format!(
                            "{} dated holidays · {} active types",
                            v.policy.holidays.len(),
                            v.policy.types.iter().filter(|t| t.active).count()
                        ));
                        for t in &v.policy.types {
                            ui.label(format!(
                                "{} · {} · carry up to {} day(s)",
                                t.name,
                                t.accrual.label(),
                                crate::leave::days(t.carryover_units)
                            ));
                        }
                        if let Some(a) = c.audit.iter().find(|a| a.id == v.id) {
                            ui.label(format!(
                                "{} · {}",
                                a.created_by_name,
                                a.posted_at
                                    .with_timezone(&self.data.company.tz())
                                    .format("%Y-%m-%d %H:%M")
                            ));
                            ui.label(&a.reason);
                            if !a.reference.is_empty() {
                                ui.label(format!("Reference: {}", a.reference));
                            }
                        }
                        let active = self
                            .data
                            .leave
                            .iter()
                            .filter(|r| r.live())
                            .map(|r| r.policy.policy_revision)
                            .collect::<Vec<_>>();
                        if admin
                            && c.timeline.can_withdraw(v.id, c.as_of, &active)
                            && ui.button("Withdraw this future tail").clicked()
                        {
                            self.leave_state.withdraw_target = Some(v.id);
                            self.leave_state.policy_reason.clear();
                            self.leave_state.policy_reference.clear();
                        }
                        for w in c.withdrawals.iter().filter(|w| w.schedule_id == v.id) {
                            if let Some(a) = c.audit.iter().find(|a| a.id == w.id) {
                                ui.separator();
                                ui.label(format!("Withdrawal by {}", a.created_by_name));
                                ui.label(&a.reason);
                                if !a.reference.is_empty() {
                                    ui.label(format!("Reference: {}", a.reference));
                                }
                            }
                        }
                    });
                }
            });
            ui.add_space(10.0);
        }
        if admin && let Some(id) = self.leave_state.withdraw_target {
            let mut submit = false;
            let mut cancel = false;
            card(ui, |ui| {
                ui.label(RichText::new("Withdraw future policy").strong());
                ui.label("Only the unactivated future tail without active request references can be withdrawn. The full snapshot and withdrawal receipt remain in history.");
                ui.add_enabled_ui(
                    !self.busy && self.leave_state.policy_pending.is_none(),
                    |ui| {
                        field(
                            ui,
                            "Private withdrawal reason",
                            &mut self.leave_state.policy_reason,
                            "Explain why it should not activate",
                        );
                        field(
                            ui,
                            "Private reference (optional)",
                            &mut self.leave_state.policy_reference,
                            "Review reference",
                        );
                        ui.horizontal_wrapped(|ui| {
                            submit = ui.button("Post withdrawal").clicked();
                            cancel = ui.button("Keep scheduled policy").clicked();
                        });
                    },
                );
            });
            if cancel {
                self.leave_state.withdraw_target = None;
            }
            if submit {
                let change = Change::Withdraw(WithdrawRecord {
                    id: Uuid::new_v4(),
                    schedule_id: id,
                    expected_revision: c.revision,
                    reason: self.leave_state.policy_reason.trim().into(),
                    reference: self.leave_state.policy_reference.clone(),
                });
                match change.payload(&self.data) {
                    Ok(_) => {
                        self.leave_state.policy_pending = Some(change.clone());
                        self.send(Command::Change(Action::DatedLeave(change)));
                    }
                    Err(e) => self.notice = Some((e.to_string(), true)),
                }
            }
            ui.add_space(10.0);
        }
    }
}
