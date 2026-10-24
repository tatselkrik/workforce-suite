use super::*;
use crate::payroll_government::registration as reg;
#[derive(Default)]
pub(super) struct State {
    scope: Option<(Uuid, Uuid, Role)>,
    person: bool,
    employee: Option<Uuid>,
    loaded: Option<reg::Snapshot>,
    draft: Option<reg::Save>,
    pub(super) pending: bool,
    history_page: usize,
}
impl State {
    fn bind(&mut self, data: &Snapshot) {
        let scope = Some((data.company.id, data.access.user_id, data.access.role));
        if self.scope != scope || !data.access.role.manages_hr() {
            *self = Self {
                scope,
                ..Default::default()
            };
        }
    }
    pub(super) fn editing(&self) -> bool {
        self.draft.is_some()
    }
    pub(super) fn scroll_key(&self) -> (u8, Option<Uuid>) {
        (
            10 + u8::from(self.person) + 2 * u8::from(self.editing()),
            self.draft.as_ref().map(|d| d.id).or(self.employee),
        )
    }
    fn target(&self) -> anyhow::Result<Option<Uuid>> {
        if self.person {
            Ok(Some(
                self.employee
                    .ok_or_else(|| anyhow::anyhow!("Choose an employee."))?,
            ))
        } else {
            Ok(None)
        }
    }
    pub(super) fn accept(&mut self, r: reg::Snapshot, data: &Snapshot) {
        self.bind(data);
        if data.access.role.manages_hr()
            && r.company_id == data.company.id
            && self.target().is_ok_and(|e| e == r.employee_id)
        {
            if self.pending {
                self.draft = None;
                self.pending = false;
            }
            self.loaded = Some(r);
        }
    }
    fn open(&mut self) {
        let Some(snapshot) = &self.loaded else {
            return;
        };
        let record = snapshot.record.as_ref();
        self.draft = Some(reg::Save {
            id: record.map(|r| r.id).unwrap_or_else(Uuid::new_v4),
            operation_id: Uuid::new_v4(),
            employee_id: snapshot.employee_id,
            expected_revision: record.map(|r| r.revision),
            details: record.map(|r| r.details.clone()).unwrap_or_else(|| {
                reg::fields(self.person)
                    .into_iter()
                    .map(|k| (k.into(), String::new()))
                    .collect()
            }),
            reference: record.map(|r| r.reference.clone()).unwrap_or_default(),
            note: record.map(|r| r.note.clone()).unwrap_or_default(),
            reason: String::new(),
            reviewed: false,
        });
    }
}
fn grid(ui: &mut Ui, values: &mut reg::Details, person: bool, editable: bool) -> Vec<egui::Rect> {
    let keys = reg::fields(person);
    let mut rects = vec![];
    for pair in keys.chunks(2) {
        let w = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let height = pair
            .iter()
            .map(|k| {
                ui.painter()
                    .layout(
                        reg::label(k, person).into(),
                        font.clone(),
                        ui.visuals().text_color(),
                        w,
                    )
                    .size()
                    .y
            })
            .fold(0.0_f32, f32::max);
        ui.columns(2, |c| {
            for (i, k) in pair.iter().enumerate() {
                c[i].allocate_ui_with_layout(
                    egui::vec2(w, height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_min_size(egui::vec2(w, height));
                        ui.add(egui::Label::new(reg::label(k, person)).wrap());
                    },
                );
                let value = values.entry((*k).into()).or_default();
                if editable {
                    let rect = if *k == "sex" || *k == "employer_type" {
                        egui::ComboBox::from_id_salt(("registration", *k))
                            .width(w)
                            .selected_text(if value.is_empty() {
                                "Not recorded"
                            } else {
                                value.as_str()
                            })
                            .show_ui(&mut c[i], |ui| {
                                ui.selectable_value(value, String::new(), "Not recorded");
                                let choices: &[(&str, &str)] = if *k == "sex" {
                                    &[("M", "Male (M)"), ("F", "Female (F)")]
                                } else {
                                    &[
                                        ("private", "Private"),
                                        ("government", "Government"),
                                        ("household", "Household"),
                                    ]
                                };
                                for (v, title) in choices {
                                    ui.selectable_value(value, (*v).into(), *title);
                                }
                            })
                            .response
                            .rect
                    } else {
                        c[i].add(egui::TextEdit::singleline(value).desired_width(w))
                            .rect
                    };
                    rects.push(rect);
                } else {
                    c[i].add(
                        egui::Label::new(if value.is_empty() {
                            RichText::new("Not recorded").color(muted(&c[i]))
                        } else {
                            RichText::new(value.as_str())
                        })
                        .wrap(),
                    );
                }
            }
        });
        ui.add_space(8.0);
    }
    rects
}
fn multiline(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_rows(2)
            .desired_width(ui.available_width()),
    );
}
impl HrisApp {
    pub(super) fn payroll_government_registration(&mut self, ui: &mut Ui) {
        self.payroll_state.government.registration.bind(&self.data);
        if !self.data.access.role.manages_hr() {
            return;
        }
        if let Some(command) = render(
            ui,
            &mut self.payroll_state.government.registration,
            &self.data,
            self.busy,
        ) {
            if matches!(
                &command,
                Command::GovernmentRegistration { save: Some(_), .. }
            ) {
                self.payroll_state.government.registration.pending = true;
            }
            self.send(command);
        }
    }
}
fn render(ui: &mut Ui, state: &mut State, data: &Snapshot, busy: bool) -> Option<Command> {
    ui.heading("Government registration details");
    ui.label("HR records of legal names, registered addresses and assigned identifiers. Employment, payroll coverage and company branding remain separate.");
    if state.draft.is_some() {
        return form(ui, state, data, busy);
    }
    let before = (state.person, state.employee);
    ui.add_enabled_ui(!busy, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut state.person, false, "Company registration");
            ui.selectable_value(&mut state.person, true, "Employee registration");
        });
        if state.person {
            egui::ComboBox::from_id_salt("registration_employee")
                .selected_text(
                    state
                        .employee
                        .map(|id| data.employee_name(id).to_owned())
                        .unwrap_or("Choose employee".into()),
                )
                .show_ui(ui, |ui| {
                    for e in &data.employees {
                        ui.selectable_value(
                            &mut state.employee,
                            Some(e.id),
                            format!(
                                "{} · {}{}",
                                e.employee_code,
                                e.full_name,
                                if e.status == EmployeeStatus::Archived {
                                    " · Archived"
                                } else {
                                    ""
                                }
                            ),
                        );
                    }
                });
        }
    });
    if before != (state.person, state.employee) {
        state.loaded = None;
        state.history_page = 0;
    }
    let target = state.target();
    let mut out = None;
    let connected = data.mode == Mode::Supabase;
    let preview = cfg!(feature = "qa-screenshot") && state.loaded.is_some();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                !busy && connected && target.is_ok(),
                egui::Button::new("Load / refresh registration"),
            )
            .clicked()
        {
            out = Some(Command::GovernmentRegistration {
                employee_id: target.as_ref().unwrap().to_owned(),
                save: None,
            });
        }
        if ui
            .add_enabled(
                !busy && (connected || preview) && state.loaded.is_some(),
                egui::Button::new(
                    if state.loaded.as_ref().is_some_and(|s| s.record.is_some()) {
                        "Edit registration"
                    } else {
                        "Add registration"
                    },
                ),
            )
            .clicked()
        {
            state.open();
        }
    });
    if !connected && !preview {
        ui.label("Company account sign-in is required.");
        return out;
    }
    if let Err(e) = target {
        ui.label(e.to_string());
        return out;
    }
    let Some(snapshot) = &state.loaded else {
        ui.label(
            "Load the selected record before editing. Unknown registration facts start blank.",
        );
        return out;
    };
    if let Some(r) = &snapshot.record {
        ui.label(format!(
            "Revision {} · Updated {} UTC",
            r.revision,
            r.updated_at.format("%Y-%m-%d %H:%M")
        ));
        card(ui, |ui| {
            grid(ui, &mut r.details.clone(), state.person, false);
        });
        ui.collapsing("Supporting registration record", |ui| {
            ui.add(egui::Label::new(&r.reference).wrap());
            ui.add(egui::Label::new(&r.note).wrap());
        });
    } else {
        ui.label("No registration facts have been recorded for this target.");
    }
    ui.separator();
    ui.heading("Registration history");
    let pages = snapshot.history.len().div_ceil(5).max(1);
    state.history_page = state.history_page.min(pages - 1);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(state.history_page > 0, egui::Button::new("Newer history"))
            .clicked()
        {
            state.history_page -= 1;
        }
        ui.label(format!("Page {} / {}", state.history_page + 1, pages));
        if ui
            .add_enabled(
                state.history_page + 1 < pages,
                egui::Button::new("Older history"),
            )
            .clicked()
        {
            state.history_page += 1;
        }
    });
    for e in snapshot
        .history
        .iter()
        .rev()
        .skip(state.history_page * 5)
        .take(5)
    {
        card(ui, |ui| {
            ui.label(format!(
                "Revision {} · {} UTC · {}",
                e.revision,
                e.recorded_at.format("%Y-%m-%d %H:%M"),
                e.actor_name
            ));
            ui.add(egui::Label::new(&e.reason).wrap());
            let changed: Vec<_> = reg::fields(state.person)
                .into_iter()
                .filter(|k| {
                    e.before_record.as_ref().and_then(|r| r.details.get(*k))
                        != e.after_record.details.get(*k)
                })
                .map(|k| reg::label(k, state.person))
                .collect();
            ui.add(
                egui::Label::new(if changed.is_empty() {
                    "Supporting evidence review".into()
                } else {
                    format!("Changed: {}", changed.join(", "))
                })
                .wrap(),
            );
            ui.collapsing("Saved registration snapshot", |ui| {
                grid(ui, &mut e.after_record.details.clone(), state.person, false);
                ui.add(egui::Label::new(&e.after_record.reference).wrap());
                ui.add(egui::Label::new(&e.after_record.note).wrap());
            });
        });
    }
    out
}
fn form(ui: &mut Ui, state: &mut State, data: &Snapshot, busy: bool) -> Option<Command> {
    let mut out = None;
    let mut discard = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(!busy, egui::Button::new("Discard unsaved changes"))
            .clicked()
        {
            discard = true;
        }
        if ui
            .add_enabled(!busy, egui::Button::new("Refresh stored record"))
            .clicked()
        {
            if let Ok(eid) = state.target() {
                out = Some(Command::GovernmentRegistration {
                    employee_id: eid,
                    save: None,
                });
            }
        }
    });
    let stored = state.loaded.as_ref().and_then(|s| s.record.as_ref());
    let draft = state.draft.as_mut().unwrap();
    let stale = stored
        .map(|r| r.id != draft.id || Some(r.revision) != draft.expected_revision)
        .unwrap_or(draft.expected_revision.is_some());
    ui.label(if state.person {
        draft
            .employee_id
            .map(|id| data.employee_name(id).to_owned())
            .unwrap_or("Employee".into())
    } else {
        data.company.name.clone()
    });
    ui.label("Copy facts from supporting registration documents. Keep unknown fields blank and preserve identifier leading zeroes. Name parts are entered explicitly.");
    let mut problem = None;
    ui.add_enabled_ui(!busy,|ui|{
        card(ui,|ui|{grid(ui,&mut draft.details,state.person,true);});
        multiline(ui,"Registration document / verification reference",&mut draft.reference);
        multiline(ui,"Private registration note",&mut draft.note);
        multiline(ui,"Reason for this entry or correction",&mut draft.reason);
        ui.checkbox(&mut draft.reviewed,"I reviewed these facts against the supporting registration documents.");
        let result=draft.validate(data);
        if let Err(e)=&result{problem=Some(e.to_string());}
        if stale{problem=Some("The stored registration changed. Discard this form and reopen its current revision; your unsaved input is retained until then.".into());}
        if ui.add_enabled(!stale&&result.is_ok(),egui::Button::new("Save registration")).clicked(){out=Some(Command::GovernmentRegistration{employee_id:draft.employee_id,save:result.ok()});}
    });
    if let Some(e) = problem {
        ui.add(egui::Label::new(RichText::new(e).color(muted(ui))).wrap());
    }
    if discard {
        state.draft = None;
    }
    out
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_registration_preview(&mut self, preview: &str) {
        if self.data.mode != Mode::Demo {
            return;
        }
        self.payroll_state.tab = 13;
        self.payroll_state.government.bind(&self.data);
        self.payroll_state.government.registration_open = true;
        let s = &mut self.payroll_state.government.registration;
        s.bind(&self.data);
        if !self.data.access.role.manages_hr() {
            return;
        }
        s.person = preview.contains("employee");
        s.employee = s.person.then_some(self.data.employees[1].id);
        s.loaded = Some(reg::preview(self.data.company.id, s.employee));
        if preview.ends_with("empty") {
            let r = s.loaded.as_mut().unwrap();
            r.record = None;
            r.history.clear();
        }
        if preview.ends_with("edit") || preview.ends_with("empty") {
            s.open();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_inputs_align_and_fit_with_wrapped_labels_at_supported_sizes() {
        for width in [760.0, 520.0, 420.0] {
            for scale in [1.0, 1.25] {
                let ctx = egui::Context::default();
                configure_theme(&ctx);
                ctx.set_zoom_factor(scale);
                let mut values = reg::Details::new();
                for _ in 0..2 {
                    let mut output=ctx.run_ui(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,Vec2::new(width,3000.0))),..Default::default()},|ui|{
                    let outer=ui.available_rect_before_wrap();let rects=grid(ui,&mut values,true,true);
                    for pair in rects.chunks(2){if pair.len()==2{
                        assert!((pair[0].top()-pair[1].top()).abs()<1.0,"Misaligned registration controls: {pair:?}");
                        assert!(pair[0].right()<=pair[1].left()&&pair[1].right()<=outer.right()+1.0,"Registration controls exceed their columns: {pair:?} / {outer:?}");
                    }}
                });
                    output.textures_delta.clear();
                }
            }
        }
    }
    #[test]
    fn background_refresh_preserves_registration_edits_and_new_account_clears_private_records() {
        let store = DemoStore::new();
        let mut s = State::default();
        s.bind(&store.data);
        let r = reg::preview(store.data.company.id, None);
        s.accept(r.clone(), &store.data);
        s.open();
        s.draft.as_mut().unwrap().reason = "Unsaved correction".into();
        s.accept(r.clone(), &store.data);
        assert_eq!(s.draft.as_ref().unwrap().reason, "Unsaved correction");
        s.pending = true;
        s.accept(r.clone(), &store.data);
        assert!(!s.editing());
        s.open();
        s.pending = false;
        s.accept(r.clone(), &store.data);
        assert!(s.editing());
        let mut other = store.data.clone();
        other.access.user_id = Uuid::new_v4();
        s.bind(&other);
        assert!(s.loaded.is_none() && !s.editing());
        other.access.role = Role::Manager;
        s.accept(r, &other);
        assert!(s.loaded.is_none() && !s.editing());
    }
}
