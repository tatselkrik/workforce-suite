use super::*;
use crate::shifts::{self, ShiftAssignmentDraft, ShiftTemplateDraft};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Tab {
    #[default]
    Daily,
    Shifts,
    Assignments,
    History,
    Calculations,
    Requests,
    Overtime,
}
#[derive(Default)]
pub(super) struct State {
    pub tab: Tab,
    pub template: Option<ShiftTemplateDraft>,
    pub assignment: Option<ShiftAssignmentDraft>,
    pub show_inactive: bool,
    pub search: String,
    pub history_employee: Option<Uuid>,
    pub history_template: Option<Uuid>,
    pub saving: bool,
}

impl HrisApp {
    pub(super) fn attendance_navigation(&mut self, ui: &mut Ui) -> bool {
        let tabs_per_row = if ui.available_width() >= 700.0 {
            5
        } else if ui.available_width() >= 420.0 {
            3
        } else {
            2
        };
        let tabs = [
            (Tab::Daily, "Daily attendance"),
            (Tab::Shifts, "Shift schedules"),
            (Tab::Assignments, "Assignments"),
            (Tab::History, "Schedule history"),
            (Tab::Calculations, "Time calculations"),
            (Tab::Requests, "Correction requests"),
            (Tab::Overtime, "Overtime approvals"),
        ];
        for row in tabs.chunks(tabs_per_row) {
            ui.horizontal_wrapped(|ui| {
                for &(tab, label) in row {
                    ui.add_enabled_ui(
                        self.shift_state.template.is_none()
                            && self.shift_state.assignment.is_none()
                            && self.break_state.form.is_none()
                            && self.break_state.void_target.is_none()
                            && !self.calculation_state.editing()
                            && !self.request_state.editing()
                            && !self.overtime_state.editing(),
                        |ui| {
                            ui.selectable_value(&mut self.shift_state.tab, tab, label);
                        },
                    );
                }
            });
        }
        ui.add_space(10.0);
        if self.shift_state.tab == Tab::Daily && self.attendance_date > self.data.company.today() {
            self.attendance_date = self.data.company.today();
            self.calendar_month = month_start(self.attendance_date);
        }
        if self.shift_state.tab != Tab::Daily {
            self.break_state.detail = None;
        }
        if let Some(id) = self.break_state.detail {
            self.attendance_breaks(ui, id);
            return false;
        }
        if self.shift_state.template.is_some() {
            self.shift_template_editor(ui);
            return false;
        }
        if self.shift_state.assignment.is_some() {
            self.shift_assignment_editor(ui);
            return false;
        }
        match self.shift_state.tab {
            Tab::Overtime => {
                self.overtime_approvals(ui);
                false
            }
            Tab::Requests => {
                self.attendance_requests(ui);
                false
            }
            Tab::Daily => true,
            Tab::Shifts => {
                self.shift_templates(ui);
                false
            }
            Tab::Assignments => {
                self.shift_assignments(ui);
                false
            }
            Tab::History => {
                self.shift_history(ui);
                false
            }
            Tab::Calculations => {
                self.time_calculations(ui);
                false
            }
        }
    }
    fn shift_templates(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            if self.data.access.allows(crate::staff_roles::Capability::AttendanceAdministration) && primary(ui, "+ Add shift") {
                self.shift_state.template = Some(Default::default());
            }
            ui.checkbox(&mut self.shift_state.show_inactive, "Show inactive shifts");
        });
        ui.label(RichText::new("Create a shift, then assign it to employees from an effective date. Times use the company time zone when assigned.").color(muted(ui)));
        let rows: Vec<_> = self
            .data
            .shifts
            .templates
            .iter()
            .filter(|t| t.active || self.shift_state.show_inactive)
            .cloned()
            .collect();
        if rows.is_empty() {
            empty(
                ui,
                "No shift schedules yet. HR or an Administrator can add the first shift.",
            );
        }
        for t in rows {
            card(ui, |ui| {
                ui.label(RichText::new(&t.name).size(18.0).strong());
                ui.label(format!(
                    "{} · {}",
                    shifts::hours(t.start_minute, t.end_minute),
                    shifts::day_names(&t.days)
                ));
                breaks_view::rules_summary(ui, &t.break_rules);
                ui.horizontal_wrapped(|ui| {
                    badge(ui, if t.active { "Active" } else { "Inactive" }, t.active);
                    ui.label(if t.works_on_holidays {
                        "Scheduled on company holidays"
                    } else {
                        "Company holidays are days off"
                    });
                    if self.data.access.allows(crate::staff_roles::Capability::AttendanceAdministration) {
                        if ui.button("Edit shift").clicked() {
                            self.shift_state.template = Some(ShiftTemplateDraft::from(&t));
                        }
                        if ui.button("Version history").clicked() {
                            self.shift_state.history_template = Some(t.id);
                            self.shift_state.history_employee = None;
                            self.shift_state.tab = Tab::History;
                        }
                    }
                });
            });
            ui.add_space(8.0);
        }
        ui.label(RichText::new("Editing or deactivating a shift keeps existing assignment snapshots. Assign the revised shift to apply its new hours. Set break rules when editing the shift. Lateness, undertime and overtime calculations follow separately.").small().color(muted(ui)));
    }
    fn shift_template_editor(&mut self, ui: &mut Ui) {
        let Some(mut draft) = self.shift_state.template.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.heading(if draft.id.is_some() {
                "Edit shift schedule"
            } else {
                "New shift schedule"
            });
            ui.add_enabled_ui(!self.busy,|ui| {
                field(ui,"Shift name *",&mut draft.name,"For example, Day shift or Night shift");
                shift_hours_fields(ui, &mut draft.start, &mut draft.end);
                ui.label(RichText::new("24-hour time. An end before the start finishes the following day. Equal times are not a valid shift.").small().color(muted(ui)));
                ui.add_space(10.0);ui.label(RichText::new("Working days *").strong());
                ui.horizontal_wrapped(|ui| {for day in 1..=7 {let mut checked=draft.days.contains(&day);if ui.checkbox(&mut checked,shifts::DAY_NAMES[(day-1) as usize]).changed() {draft.days.retain(|d| *d!=day);if checked {draft.days.push(day);}}}});
                breaks_view::rules_editor(ui,&mut draft.break_rules);
                ui.checkbox(&mut draft.works_on_holidays,"Schedule work on company holidays");
                ui.checkbox(&mut draft.active,"Active — available for new assignments");
                ui.add_space(8.0);ui.label(format!("Assignment time zone: {}",self.data.company.timezone));
                ui.label(RichText::new("Existing assignments retain their saved name, days, hours and time zone. This save creates a retained template version.").color(muted(ui)).small());
                ui.separator();ui.horizontal_wrapped(|ui| {save=primary(ui,"Save shift");cancel=ui.button("Cancel").clicked();});
            });
        });
        if save {
            match draft.template(&self.data) {
                Ok(_) => {
                    self.shift_state.saving = true;
                    self.send(Command::Change(Action::SaveShiftTemplate(draft.clone())));
                }
                Err(e) => self.notice = Some((e.to_string(), true)),
            }
        }
        if !cancel {
            self.shift_state.template = Some(draft);
        }
    }
    fn shift_assignments(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Find employee");
            ui.add(
                egui::TextEdit::singleline(&mut self.shift_state.search)
                    .hint_text("Name or employee code")
                    .desired_width(250.0),
            );
        });

        ui.horizontal_wrapped(|ui| {
            ui.label("Schedule date");
            let response = ui.button(self.attendance_date.to_string());
            egui::Popup::menu(&response)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    if calendar_control(
                        ui,
                        &mut self.attendance_date,
                        &mut self.calendar_month,
                        self.data.company.today(),
                        NaiveDate::from_ymd_opt(2200, 12, 31).unwrap(),
                        &self.data,
                    ) {
                        ui.close();
                    }
                });
            if ui.button("Today").clicked() {
                self.attendance_date = self.data.company.today();
                self.calendar_month = month_start(self.attendance_date);
            }
        });

        let search = self.shift_state.search.trim().to_lowercase();
        let employees: Vec<_> = self
            .data
            .employees
            .iter()
            .filter(|e| {
                e.status == EmployeeStatus::Active
                    && (search.is_empty()
                        || e.full_name.to_lowercase().contains(&search)
                        || e.employee_code.to_lowercase().contains(&search))
            })
            .cloned()
            .collect();
        if employees.is_empty() {
            empty(ui, "No employees match your search.");
        }
        for e in employees {
            let assignment = self
                .data
                .shifts
                .assignment(e.id, self.attendance_date)
                .cloned();
            card(ui, |ui| {
                ui.label(RichText::new(format!("{} · {}", e.full_name, e.employee_code)).strong());
                if let Some(p) = assignment.as_ref().and_then(|a| a.plan.as_ref()) {
                    ui.label(format!("{} · {}", p.name, p.hours()));
                    ui.label(format!("{} · {}", shifts::day_names(&p.days), p.timezone));
                    ui.label(
                        RichText::new(if p.working(&self.data, self.attendance_date) {
                            "Scheduled workday"
                        } else {
                            "Day off / company holiday"
                        })
                        .color(muted(ui)),
                    );
                } else {
                    ui.label("Company calendar · no individual shift hours");
                }
                ui.horizontal_wrapped(|ui| {
                    if let Some(a) = &assignment {
                        ui.label(
                            RichText::new(format!(
                                "Effective {} · change #{}",
                                a.effective_on, a.sequence
                            ))
                            .small()
                            .color(muted(ui)),
                        );
                    }
                    if self.data.access.allows(crate::staff_roles::Capability::AttendanceAdministration) && ui.button("Assign / change").clicked()
                    {
                        self.shift_state.assignment = Some(ShiftAssignmentDraft::new(
                            &self.data,
                            e.id,
                            self.attendance_date,
                        ));
                    }
                    if ui.button("Assignment history").clicked() {
                        self.shift_state.history_employee = Some(e.id);
                        self.shift_state.history_template = None;
                        self.shift_state.tab = Tab::History;
                    }
                });
            });
            ui.add_space(8.0);
        }
    }
    fn shift_assignment_editor(&mut self, ui: &mut Ui) {
        let Some(mut draft) = self.shift_state.assignment.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.heading("Assign employee schedule");
            ui.label(RichText::new(self.data.employee_name(draft.employee_id)).strong());
            ui.add_enabled_ui(!self.busy,|ui| {
                ui.label("Shift schedule *");
                let previous_template=draft.template_id;
                egui::ComboBox::from_id_salt("assign_shift_template").width(320.0).selected_text(draft.template_id.and_then(|id|self.data.shifts.templates.iter().find(|t|t.id==id)).map(|t|t.name.as_str()).unwrap_or("Company calendar — no shift hours")).show_ui(ui,|ui| {
                    ui.selectable_value(&mut draft.template_id,None,"Company calendar — no shift hours");
                    for t in self.data.shifts.templates.iter().filter(|t|t.active){ui.selectable_value(&mut draft.template_id,Some(t.id),format!("{} · {}",t.name,shifts::hours(t.start_minute,t.end_minute)));}
                });
                if previous_template!=draft.template_id {draft.expected_template_revision=draft.template_id.and_then(|id|self.data.shifts.templates.iter().find(|t|t.id==id).map(|t|t.revision));}
                field(ui,"Effective date *",&mut draft.effective_on,"YYYY-MM-DD");
                field(ui,"Reason *",&mut draft.reason,"Initial assignment, schedule change or correction (up to 500 characters)");
                if let Some(t)=draft.template_id.and_then(|id|self.data.shifts.templates.iter().find(|t|t.id==id)) {ui.label(format!("{} · {} · {}",shifts::hours(t.start_minute,t.end_minute),shifts::day_names(&t.days),self.data.company.timezone));}
                ui.add_space(10.0);ui.label(RichText::new("Applies from this date until the next dated assignment. A later save for the same date takes precedence; every earlier change stays in history. Past dates correct the schedule shown for that period. Recorded attendance and filed leave keep their saved snapshots.").color(muted(ui)));
                ui.separator();ui.horizontal_wrapped(|ui| {save=primary(ui,"Save assignment");cancel=ui.button("Cancel").clicked();});
            });
        });
        if save {
            match draft.assignment(&self.data) {
                Ok(_) => {
                    self.shift_state.saving = true;
                    self.send(Command::Change(Action::AssignShift(draft.clone())));
                }
                Err(e) => self.notice = Some((e.to_string(), true)),
            }
        }
        if !cancel {
            self.shift_state.assignment = Some(draft);
        }
    }
    fn shift_history(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            if let Some(id) = self.shift_state.history_employee {
                ui.label(RichText::new(self.data.employee_name(id)).strong());
            }
            if self.shift_state.history_template.is_some() {
                ui.label(RichText::new("Shift template versions").strong());
            }
            if ui.button("Show all assignments").clicked() {
                self.shift_state.history_employee = None;
                self.shift_state.history_template = None;
            }
        });
        if let Some(id) = self.shift_state.history_template {
            let mut versions: Vec<_> = self
                .data
                .shifts
                .versions
                .iter()
                .filter(|v| v.template_id == id)
                .cloned()
                .collect();
            versions.sort_by_key(|v| std::cmp::Reverse(v.revision));
            for v in versions {
                card(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{} · version {}", v.snapshot.name, v.revision + 1))
                            .strong(),
                    );
                    ui.label(format!(
                        "{} · {}",
                        shifts::hours(v.snapshot.start_minute, v.snapshot.end_minute),
                        shifts::day_names(&v.snapshot.days)
                    ));
                    ui.label(format!(
                        "{} · {} · {}",
                        if v.snapshot.active {
                            "Active"
                        } else {
                            "Inactive"
                        },
                        if v.snapshot.works_on_holidays {
                            "Works on holidays"
                        } else {
                            "Holidays off"
                        },
                        v.actor_name
                    ));
                    ui.label(
                        v.created_at
                            .with_timezone(&self.data.company.tz())
                            .format("%Y-%m-%d %H:%M:%S %Z")
                            .to_string(),
                    );
                });
                ui.add_space(8.0);
            }
            return;
        }
        let mut rows: Vec<_> = self
            .data
            .shifts
            .assignments
            .iter()
            .filter(|a| {
                self.shift_state
                    .history_employee
                    .is_none_or(|id| a.employee_id == id)
            })
            .cloned()
            .collect();
        rows.sort_by_key(|a| std::cmp::Reverse((a.created_at, a.sequence)));
        if rows.is_empty() {
            empty(
                ui,
                "No schedule assignments recorded. Earlier schedules were not reconstructed.",
            );
        }
        for a in rows {
            card(ui, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · effective {}",
                        self.data.employee_name(a.employee_id),
                        a.effective_on
                    ))
                    .strong(),
                );
                if let Some(p) = &a.plan {
                    ui.label(format!(
                        "{} · {} · {} · {}",
                        p.name,
                        p.hours(),
                        shifts::day_names(&p.days),
                        p.timezone
                    ));
                } else {
                    ui.label("Company calendar — no individual shift hours");
                }
                if self.data.shifts.assignments.iter().any(|other| {
                    other.employee_id == a.employee_id
                        && other.effective_on == a.effective_on
                        && other.sequence > a.sequence
                }) {
                    ui.label(
                        RichText::new("Replaced by a later change for this effective date")
                            .small()
                            .color(muted(ui)),
                    );
                }
                ui.label(&a.reason);
                ui.label(
                    RichText::new(format!(
                        "Change #{} · {} · {}",
                        a.sequence,
                        a.actor_name,
                        a.created_at
                            .with_timezone(&self.data.company.tz())
                            .format("%Y-%m-%d %H:%M:%S %Z")
                    ))
                    .small()
                    .color(muted(ui)),
                );
            });
            ui.add_space(8.0);
        }
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_shift_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("shift-") {
            return;
        }
        let role = self.data.access.role;
        self.data.access.role = Role::Administrator;
        let mut day = ShiftTemplateDraft::default();
        day.name = "Office day shift".into();
        let _ = shifts::save_demo_template(&mut self.data, day);
        let mut night = ShiftTemplateDraft::default();
        night.name = "Night operations — service and support".into();
        night.start = "22:00".into();
        night.end = "06:00".into();
        night.days = vec![1, 2, 3, 4, 5, 6];
        night.works_on_holidays = true;
        let _ = shifts::save_demo_template(&mut self.data, night);
        let employee = self.data.employees[0].id;
        let template = self
            .data
            .shifts
            .templates
            .iter()
            .find(|t| t.start_minute == 1320)
            .unwrap()
            .id;
        let date = self.data.company.today();
        let mut a = ShiftAssignmentDraft::new(&self.data, employee, date);
        a.template_id = Some(template);
        a.expected_template_revision = Some(0);
        a.reason = "Synthetic schedule preview".into();
        if let Ok(a) = a.assignment(&self.data) {
            self.data.shifts.assignments.push(a);
        }
        self.data.access.role = role;
        self.page = Page::Attendance;
        self.shift_state.tab = match form.as_str() {
            "shift-assignments" | "shift-assign-form" => Tab::Assignments,
            "shift-history" | "shift-version-history" => Tab::History,
            "shift-daily" => Tab::Daily,
            _ => Tab::Shifts,
        };
        match form.as_str() {
            "shift-template-form" => {
                self.shift_state.template = Some(ShiftTemplateDraft::from(
                    self.data
                        .shifts
                        .templates
                        .iter()
                        .find(|t| t.id == template)
                        .unwrap(),
                ))
            }
            "shift-assign-form" => {
                self.shift_state.assignment =
                    Some(ShiftAssignmentDraft::new(&self.data, employee, date))
            }
            "shift-history" => self.shift_state.history_employee = Some(employee),
            "shift-version-history" => self.shift_state.history_template = Some(template),
            "shift-empty" => self.data.shifts = Default::default(),
            _ => {}
        }
    }
}

fn shift_hours_fields(ui: &mut Ui, start: &mut String, end: &mut String) -> [egui::Rect; 2] {
    ui.columns(2, |columns| {
        let left = columns[0]
            .vertical(|ui| field(ui, "Start time *", start, "08:00"))
            .response
            .rect;
        let right = columns[1]
            .vertical(|ui| field(ui, "End time *", end, "17:00"))
            .response
            .rect;
        [left, right]
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_shift_time_fields_fit_and_align_at_normal_and_enlarged_display_sizes() {
        for width in [730.0, 520.0, 420.0] {
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            let mut start = "22:00".to_owned();
            let mut end = "06:00".to_owned();
            for _ in 0..2 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(width, 600.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        let outer = ui.available_rect_before_wrap();
                        let cells = shift_hours_fields(ui, &mut start, &mut end);
                        assert!(
                            (cells[0].top() - cells[1].top()).abs() < 1.0,
                            "Misaligned time fields: {cells:?}"
                        );
                        assert!(
                            cells[0].right() <= cells[1].left(),
                            "Time fields overlap: {cells:?}"
                        );
                        assert!(
                            cells[1].right() <= outer.right() + 1.0,
                            "End-time field exceeds viewport: {cells:?} / {outer:?}"
                        );
                    },
                );
                output.textures_delta.clear();
            }
        }
    }
}
