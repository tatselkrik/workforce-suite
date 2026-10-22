use super::*;
use crate::hiring::{self, Applicant, ApplicantDraft, HiringAction, Stage};

pub(super) struct Editor {
    pub draft: ApplicantDraft,
    pub original: Option<Applicant>,
    pub reason: String,
}
impl Editor {
    fn changed(&self) -> bool {
        self.original
            .as_ref()
            .is_none_or(|r| self.draft != ApplicantDraft::existing(r))
    }
    fn action(&self) -> HiringAction {
        match &self.original {
            Some(r) => HiringAction::Edit {
                draft: self.draft.clone(),
                expected_revision: r.revision,
                reason: self.reason.clone(),
            },
            None => HiringAction::Create {
                draft: self.draft.clone(),
                reason: self.reason.clone(),
            },
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Tab {
    #[default]
    Details,
    History,
    Attachments,
    Interviews,
    Offers,
    Onboarding,
}
#[derive(Default)]
pub(super) struct State {
    pub form: Option<Editor>,
    pub pending: Option<HiringAction>,
    pub search: String,
    pub stage: Option<Stage>,
    pub include_archived: bool,
    pub page: usize,
    pub tab: Tab,
    pub next: Option<Stage>,
    pub reason: String,
    pub closing: bool,
}
impl State {
    pub fn editing(&self) -> bool {
        self.form.is_some()
    }
    pub fn open(&mut self, r: Applicant) {
        self.form = Some(Editor {
            draft: ApplicantDraft::existing(&r),
            original: Some(r),
            reason: String::new(),
        });
        self.next = None;
        self.reason.clear();
        self.closing = false;
    }
    // Only a matching saved record may replace a draft. Failed/stale responses retain it.
    pub fn accept(&mut self, data: &Snapshot) -> bool {
        let Some(action) = self.pending.take() else {
            return true;
        };
        let (id, matches) = match &action {
            HiringAction::Create { draft, .. } => (
                draft.id,
                data.hiring.records.iter().any(|r| {
                    Some(r.id) == draft.id && ApplicantDraft::existing(r) == normalized(draft)
                }),
            ),
            HiringAction::Edit {
                draft,
                expected_revision,
                ..
            } => (
                draft.id,
                data.hiring.records.iter().any(|r| {
                    Some(r.id) == draft.id
                        && r.revision == expected_revision + 1
                        && ApplicantDraft::existing(r) == normalized(draft)
                }),
            ),
            HiringAction::Stage {
                id,
                expected_revision,
                next,
                ..
            } => (
                Some(*id),
                data.hiring.records.iter().any(|r| {
                    r.id == *id && r.revision == expected_revision + 1 && r.stage == *next
                }),
            ),
            HiringAction::Archive {
                id,
                expected_revision,
                archived,
                ..
            } => (
                Some(*id),
                data.hiring.records.iter().any(|r| {
                    r.id == *id && r.revision == expected_revision + 1 && r.archived == *archived
                }),
            ),
        };
        if matches
            && let Some(r) = data
                .hiring
                .records
                .iter()
                .find(|r| Some(r.id) == id && r.company_id == data.company.id)
        {
            self.open(r.clone());
            return true;
        }
        false
    }
}
fn normalized(d: &ApplicantDraft) -> ApplicantDraft {
    ApplicantDraft {
        id: d.id,
        full_name: d.full_name.trim().into(),
        email: d.email.trim().into(),
        phone: d.phone.trim().into(),
        position: d.position.trim().into(),
        source: d.source.trim().into(),
        applied_on: d.applied_on.trim().into(),
        notes: d.notes.trim().into(),
    }
}
fn event_title(kind: &str) -> &str {
    match kind {
        "created" => "Application recorded",
        "saved" => "Details updated",
        "stage_changed" => "Stage changed",
        "archived" => "Archived",
        "restored" => "Restored",
        _ => "Application event",
    }
}
fn values(r: &Applicant) -> Vec<(&'static str, String)> {
    vec![
        ("Full name", r.full_name.clone()),
        ("Email", r.email.clone()),
        ("Phone", r.phone.clone()),
        ("Position", r.position.clone()),
        ("Source", r.source.clone()),
        ("Applied on", r.applied_on.to_string()),
        ("HR notes", r.notes.clone()),
        ("Stage", r.stage.label().into()),
        ("Archived", if r.archived { "Yes" } else { "No" }.into()),
    ]
}
fn history_fields(ui: &mut Ui, e: &hiring::Event) -> Vec<(egui::Rect, egui::Rect)> {
    let after = values(&e.after_record);
    let before = e.before_record.as_ref().map(values);
    let width = ((ui.available_width() - 155.0) / if before.is_some() { 2.0 } else { 1.0 })
        .clamp(85.0, 320.0);
    let mut measured = Vec::new();
    egui::Grid::new(("applicant_event", e.id))
        .num_columns(if before.is_some() { 3 } else { 2 })
        .striped(true)
        .max_col_width(width)
        .spacing(Vec2::new(12.0, 10.0))
        .show(ui, |ui| {
            ui.strong("Field");
            if before.is_some() {
                ui.strong("Previous");
            }
            ui.strong("Saved value");
            ui.end_row();
            for (i, (label, value)) in after.iter().enumerate() {
                if before.as_ref().is_some_and(|b| b[i].1 == *value) {
                    continue;
                }
                // All cells share a top-down layout; bare Grid labels center in tall rows.
                let label = ui.vertical(|ui| ui.label(*label)).inner;
                if let Some(b) = &before {
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.add(
                            egui::Label::new(if b[i].1.is_empty() {
                                "Not set"
                            } else {
                                &b[i].1
                            })
                            .wrap(),
                        );
                    });
                }
                let value = ui
                    .vertical(|ui| {
                        ui.set_width(width);
                        ui.add(
                            egui::Label::new(if value.is_empty() { "Not set" } else { value })
                                .wrap(),
                        )
                    })
                    .inner;
                measured.push((label.rect, value.rect));
                ui.end_row();
            }
        });
    measured
}
impl HrisApp {
    pub(super) fn hiring(&mut self, ui: &mut Ui) {
        if self.data.access.role == Role::Manager {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(
                        !self.onboarding_state.editing() && !self.recruiting_state.editing(),
                        |ui| {
                            ui.selectable_value(
                                &mut self.onboarding_state.active,
                                false,
                                "Your interviews",
                            );
                            ui.selectable_value(
                                &mut self.onboarding_state.active,
                                true,
                                "Your onboarding tasks",
                            );
                        },
                    );
                });
            });
            ui.add_space(10.0);
            if self.onboarding_state.active {
                self.onboarding(ui, None);
            } else {
                self.applicant_interviews(ui, None);
            }
            return;
        }
        if self.onboarding_state.active {
            self.onboarding(ui, None);
            return;
        }
        if self.recruiting_state.agenda {
            self.applicant_interviews(ui, None);
            return;
        }
        if hiring::authorize(&self.data.access, &self.data).is_err() {
            self.hiring_state = State::default();
            empty(
                ui,
                "Only HR staff and Administrators can access applicant records.",
            );
            return;
        }
        if self.hiring_state.form.is_some() {
            self.applicant_editor(ui);
            return;
        }
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if primary(ui, "Add applicant") {
                    self.hiring_state.form = Some(Editor {
                        draft: ApplicantDraft::new(self.data.company.today()),
                        original: None,
                        reason: "Application recorded".into(),
                    });
                    self.hiring_state.tab = Tab::Details;
                    self.hiring_state.closing = false;
                }
                if ui.button("Interview agenda").clicked() {
                    self.recruiting_state.agenda = true;
                }
                if ui.button("Onboarding").clicked() {
                    self.onboarding_state.active = true;
                }
                ui.label(RichText::new("Private to HR and Administrators").color(muted(ui)));
            });
            let old = (
                self.hiring_state.search.clone(),
                self.hiring_state.stage,
                self.hiring_state.include_archived,
            );
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.hiring_state.search)
                        .hint_text("Search name, contact, position or source")
                        .desired_width(320.0),
                );
                egui::ComboBox::from_id_salt("applicant_filter")
                    .selected_text(
                        self.hiring_state
                            .stage
                            .map(Stage::label)
                            .unwrap_or("All stages"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.hiring_state.stage, None, "All stages");
                        for stage in Stage::ALL {
                            ui.selectable_value(
                                &mut self.hiring_state.stage,
                                Some(stage),
                                stage.label(),
                            );
                        }
                    });
                ui.checkbox(&mut self.hiring_state.include_archived, "Include archived");
            });
            if old
                != (
                    self.hiring_state.search.clone(),
                    self.hiring_state.stage,
                    self.hiring_state.include_archived,
                )
            {
                self.hiring_state.page = 0;
            }
            ui.label(RichText::new("Applicants become employees only through a future hiring workflow. Recording an application creates no employee record or sign-in account.").small().color(muted(ui)));
        });
        ui.add_space(12.0);
        let needle = self.hiring_state.search.trim().to_lowercase();
        let mut rows: Vec<_> = self
            .data
            .hiring
            .records
            .iter()
            .filter(|r| {
                r.company_id == self.data.company.id
                    && (self.hiring_state.include_archived || !r.archived)
                    && self.hiring_state.stage.is_none_or(|s| s == r.stage)
                    && (needle.is_empty()
                        || format!(
                            "{} {} {} {} {}",
                            r.full_name, r.email, r.phone, r.position, r.source
                        )
                        .to_lowercase()
                        .contains(&needle))
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| {
            b.applied_on
                .cmp(&a.applied_on)
                .then(a.full_name.cmp(&b.full_name))
                .then(a.id.cmp(&b.id))
        });
        let pages = rows.len().div_ceil(25).max(1);
        self.hiring_state.page = self.hiring_state.page.min(pages - 1);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} application(s)", rows.len()));
            if ui
                .add_enabled(self.hiring_state.page > 0, egui::Button::new("Previous"))
                .clicked()
            {
                self.hiring_state.page -= 1;
            }
            ui.label(format!("Page {} of {pages}", self.hiring_state.page + 1));
            if ui
                .add_enabled(
                    self.hiring_state.page + 1 < pages,
                    egui::Button::new("Next"),
                )
                .clicked()
            {
                self.hiring_state.page += 1;
            }
        });
        if rows.is_empty() {
            empty(
                ui,
                "No applications match. Add an applicant to start the pipeline.",
            );
        }
        for r in rows.iter().skip(self.hiring_state.page * 25).take(25) {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .link(RichText::new(&r.full_name).size(18.0).strong())
                        .clicked()
                    {
                        self.hiring_state.open(r.clone());
                        self.hiring_state.tab = Tab::Details;
                    }
                    badge(ui, r.stage.label(), !r.stage.terminal());
                    if r.archived {
                        badge(ui, "Archived", false);
                    }
                });
                ui.label(format!(
                    "{} · Applied {}",
                    r.position,
                    r.applied_on.format("%b %d, %Y")
                ));
                if !r.source.is_empty() {
                    ui.label(RichText::new(format!("Source: {}", r.source)).color(muted(ui)));
                }
            });
            ui.add_space(8.0);
        }
    }
    fn applicant_editor(&mut self, ui: &mut Ui) {
        let Some(mut e) = self.hiring_state.form.take() else {
            return;
        };
        let mut close = false;
        let mut action = None;
        let current = e
            .draft
            .id
            .and_then(|id| self.data.hiring.records.iter().find(|r| r.id == id))
            .cloned();
        let stale = match (&e.original, &current) {
            (Some(a), Some(b)) => a.revision != b.revision,
            (Some(_), None) => true,
            (None, Some(_)) => true,
            _ => false,
        };
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("< All applicants").clicked() {
                    if e.changed()
                        || !self.hiring_state.reason.is_empty()
                        || self.recruiting_state.editing()
                        || self.onboarding_state.editing()
                    {
                        self.hiring_state.closing = true;
                    } else {
                        close = true;
                    }
                }
                ui.label(
                    RichText::new(if e.original.is_none() {
                        "New applicant"
                    } else {
                        &e.draft.full_name
                    })
                    .strong()
                    .size(21.0),
                );
            });
            if let Some(r) = &e.original {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, r.stage.label(), !r.stage.terminal());
                    if self
                        .data
                        .onboarding
                        .conversions
                        .iter()
                        .any(|c| c.applicant_id == r.id)
                    {
                        badge(ui, "Hired", true);
                    }
                    if r.archived {
                        badge(ui, "Archived", false);
                    }
                });
            }
            if self.hiring_state.closing {
                ui.label("Discard unsaved changes and return to the applicant list?");
                ui.horizontal(|ui| {
                    if ui.button("Keep editing").clicked() {
                        self.hiring_state.closing = false;
                    }
                    if ui.button("Discard changes").clicked() {
                        close = true;
                    }
                });
            }
            if stale {
                ui.colored_label(ui.visuals().warn_fg_color,"A newer saved record is available. Your draft is preserved. Reload the saved record before editing further.");
                if let Some(r) = &current
                    && ui.button("Reload saved record (discard draft)").clicked()
                {
                    e = Editor {
                        draft: ApplicantDraft::existing(r),
                        original: Some(r.clone()),
                        reason: String::new(),
                    };
                    self.hiring_state.next = None;
                    self.hiring_state.reason.clear();
                }
            }
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.hiring_state.tab, Tab::Details, "Details");
                ui.add_enabled_ui(e.original.is_some(), |ui| {
                    ui.selectable_value(&mut self.hiring_state.tab, Tab::History, "History");
                    ui.selectable_value(
                        &mut self.hiring_state.tab,
                        Tab::Attachments,
                        "Résumés / files",
                    );
                    ui.selectable_value(&mut self.hiring_state.tab, Tab::Interviews, "Interviews");
                    ui.selectable_value(&mut self.hiring_state.tab, Tab::Offers, "Offers");
                    ui.selectable_value(&mut self.hiring_state.tab, Tab::Onboarding, "Onboarding");
                });
            });
        });
        ui.add_space(10.0);
        if self.hiring_state.tab == Tab::Offers {
            if let Some(id) = e.draft.id {
                self.applicant_offers(ui, id);
            }
        } else if self.hiring_state.tab == Tab::Onboarding {
            if let Some(id) = e.draft.id {
                self.onboarding(ui, Some(id));
            }
        } else if self.hiring_state.tab == Tab::Attachments {
            if let Some(id) = e.draft.id {
                self.applicant_attachments(ui, id);
            }
        } else if self.hiring_state.tab == Tab::Interviews {
            if let Some(id) = e.draft.id {
                self.applicant_interviews(ui, Some(id));
            }
        } else if self.hiring_state.tab == Tab::History {
            if let Some(id) = e.draft.id {
                self.applicant_history(ui, id);
            }
        } else {
            let archived = e.original.as_ref().is_some_and(|r| r.archived);
            card(ui, |ui| {
                ui.label(RichText::new("Application details").strong().size(18.0));
                ui.add_enabled_ui(!archived && !stale, |ui| {
                    ui.columns(2, |cols| {
                        field(
                            &mut cols[0],
                            "Full name *",
                            &mut e.draft.full_name,
                            "Applicant's full name",
                        );
                        field(&mut cols[0], "Email", &mut e.draft.email, "Optional");
                        field(&mut cols[0], "Phone", &mut e.draft.phone, "Optional");
                        field(
                            &mut cols[1],
                            "Position applied for *",
                            &mut e.draft.position,
                            "e.g. Operations Associate",
                        );
                        field(
                            &mut cols[1],
                            "Source",
                            &mut e.draft.source,
                            "e.g. Referral, walk-in, job board",
                        );
                        field(
                            &mut cols[1],
                            "Application date *",
                            &mut e.draft.applied_on,
                            "YYYY-MM-DD",
                        );
                    });
                    ui.label("Private HR notes");
                    ui.add(
                        egui::TextEdit::multiline(&mut e.draft.notes)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .hint_text("Up to 2,000 characters"),
                    );
                    field(
                        ui,
                        "Save reason *",
                        &mut e.reason,
                        "Briefly describe this save",
                    );
                    let candidate = e.action();
                    let validation = candidate.payload(&self.data);
                    if let Err(err) = &validation {
                        ui.label(RichText::new(err.to_string()).small().color(muted(ui)));
                    }
                    if ui
                        .add_enabled(
                            validation.is_ok() && (e.original.is_none() || e.changed()),
                            egui::Button::new(
                                RichText::new(if e.original.is_some() {
                                    "Save changes"
                                } else {
                                    "Create applicant"
                                })
                                .color(Color32::WHITE),
                            )
                            .fill(TEAL),
                        )
                        .clicked()
                    {
                        action = Some(candidate);
                    }
                });
                if archived {
                    ui.label("This application is archived. Restore it below before editing.");
                }
            });
            if let Some(r) = &e.original {
                ui.add_space(10.0);
                card(ui, |ui| {
                    ui.label(RichText::new("Pipeline and archive").strong().size(18.0));
                    if e.changed() {
                        ui.label("Save or discard detail edits before changing the pipeline.");
                    }
                    ui.add_enabled_ui(!e.changed() && !stale, |ui| {
                        if !r.archived
                            && !self
                                .data
                                .onboarding
                                .conversions
                                .iter()
                                .any(|c| c.applicant_id == r.id)
                        {
                            egui::ComboBox::from_id_salt("applicant_next_stage")
                                .selected_text(
                                    self.hiring_state
                                        .next
                                        .map(Stage::label)
                                        .unwrap_or("Choose next stage"),
                                )
                                .show_ui(ui, |ui| {
                                    for stage in Stage::ALL {
                                        if r.stage.change(stage, "Stage choice").is_ok() {
                                            ui.selectable_value(
                                                &mut self.hiring_state.next,
                                                Some(stage),
                                                stage.label(),
                                            );
                                        }
                                    }
                                });
                        }
                        field(
                            ui,
                            "Change reason *",
                            &mut self.hiring_state.reason,
                            "Reason for stage change, archive or restore",
                        );
                        ui.horizontal_wrapped(|ui| {
                            if !r.archived {
                                let next = self.hiring_state.next;
                                if ui
                                    .add_enabled(
                                        next.is_some()
                                            && hiring::reason(&self.hiring_state.reason).is_ok(),
                                        egui::Button::new("Save stage"),
                                    )
                                    .clicked()
                                {
                                    action = Some(HiringAction::Stage {
                                        id: r.id,
                                        expected_revision: r.revision,
                                        next: next.unwrap(),
                                        reason: self.hiring_state.reason.clone(),
                                    });
                                }
                            }
                            if ui
                                .add_enabled(
                                    hiring::reason(&self.hiring_state.reason).is_ok(),
                                    egui::Button::new(if r.archived {
                                        "Restore application"
                                    } else {
                                        "Archive application"
                                    }),
                                )
                                .clicked()
                            {
                                action = Some(HiringAction::Archive {
                                    id: r.id,
                                    expected_revision: r.revision,
                                    archived: !r.archived,
                                    reason: self.hiring_state.reason.clone(),
                                });
                            }
                        });
                    });
                    ui.label(RichText::new("Archiving preserves the application and history. Offer is a pipeline stage; it does not create an employee or account.").small().color(muted(ui)));
                });
            }
        }
        if close {
            self.recruiting_state = Default::default();
            self.onboarding_state = Default::default();
            self.hiring_state.form = None;
            self.hiring_state.closing = false;
            self.hiring_state.reason.clear();
            self.hiring_state.next = None;
        } else {
            self.hiring_state.form = Some(e);
        }
        if let Some(action) = action {
            self.hiring_state.pending = Some(action.clone());
            self.send(Command::Change(Action::Hiring(action)));
        }
    }
    fn applicant_history(&self, ui: &mut Ui, id: Uuid) {
        let mut events: Vec<_> = self
            .data
            .hiring
            .events
            .iter()
            .filter(|e| e.applicant_id == id && e.company_id == self.data.company.id)
            .collect();
        events.sort_by_key(|e| std::cmp::Reverse(e.revision));
        ui.label("Saved events preserve the reason, author and before/after details. History cannot be edited or removed.");
        if events.is_empty() {
            empty(ui, "No saved events for this application.");
        }
        for e in events {
            card(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} · Version {}", event_title(&e.kind), e.revision))
                        .strong(),
                );
                ui.label(format!(
                    "{} · {} · {}",
                    e.recorded_at
                        .with_timezone(&self.data.company.tz())
                        .format("%b %d, %Y %H:%M"),
                    e.actor_name,
                    e.actor_role.label()
                ));
                ui.label(&e.reason);
                egui::CollapsingHeader::new(if e.before_record.is_some() {
                    "View saved changes"
                } else {
                    "View original application"
                })
                .id_salt(e.id)
                .default_open(e.revision == 0)
                .show(ui, |ui| {
                    history_fields(ui, e);
                });
            });
            ui.add_space(9.0);
        }
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_hiring_preview(&mut self) {
        if self.page != Page::Hiring
            || self.data.mode != Mode::Demo
            || !self.data.access.role.manages_hr()
        {
            return;
        }
        let mut data = self.data.clone();
        for (i, name) in ["Jordan Navarro", "Alexandra Grace Villanueva", "Sam Rivera"]
            .iter()
            .enumerate()
        {
            let mut draft = ApplicantDraft::new(data.company.today());
            draft.full_name = (*name).into();
            draft.position = if i == 1 {
                "Senior Human Resources Generalist"
            } else {
                "Operations Associate"
            }
            .into();
            draft.source = "Fictional referral".into();
            draft.email = format!("applicant{}@example.invalid", i + 1);
            draft.notes =
                "Fictional preview applicant.\nAvailability confirmed for next week.".into();
            hiring::apply_demo(
                &mut data,
                &HiringAction::Create {
                    draft: draft.clone(),
                    reason: "Fictional application received".into(),
                },
                chrono::Utc::now(),
            )
            .unwrap();
            if i == 0 {
                hiring::apply_demo(
                    &mut data,
                    &HiringAction::Stage {
                        id: draft.id.unwrap(),
                        expected_revision: 0,
                        next: Stage::Screening,
                        reason: "Fictional HR review started".into(),
                    },
                    chrono::Utc::now(),
                )
                .unwrap();
            }
        }
        self.data = data;
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if preview == "hiring-new" {
            let mut d = ApplicantDraft::new(self.data.company.today());
            d.full_name = "Taylor Santos".into();
            d.position = "Finance Associate".into();
            self.hiring_state.form = Some(Editor {
                draft: d,
                original: None,
                reason: "Application recorded".into(),
            });
        } else if preview == "hiring-details" || preview == "hiring-history" {
            self.hiring_state.open(self.data.hiring.records[0].clone());
            self.hiring_state.tab = if preview == "hiring-history" {
                Tab::History
            } else {
                Tab::Details
            };
        } else if preview == "hiring-archived" {
            let r = self.data.hiring.records[0].clone();
            hiring::apply_demo(
                &mut self.data,
                &HiringAction::Archive {
                    id: r.id,
                    expected_revision: r.revision,
                    archived: true,
                    reason: "Fictional archived application".into(),
                },
                chrono::Utc::now(),
            )
            .unwrap();
            self.hiring_state.open(self.data.hiring.records[0].clone());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_matching_save_replaces_the_applicant_draft() {
        let mut data = DemoStore::new().data;
        let mut draft = ApplicantDraft::new(data.company.today());
        draft.full_name = "Fictional Applicant".into();
        draft.position = "Associate".into();
        let action = HiringAction::Create {
            draft: draft.clone(),
            reason: "Application recorded".into(),
        };
        let mut state = State {
            form: Some(Editor {
                draft: draft.clone(),
                original: None,
                reason: "Application recorded".into(),
            }),
            pending: Some(action.clone()),
            ..Default::default()
        };
        assert!(!state.accept(&data));
        assert_eq!(state.form.as_ref().unwrap().draft, draft);
        state.pending = Some(action.clone());
        hiring::apply_demo(&mut data, &action, chrono::Utc::now()).unwrap();
        assert!(state.accept(&data));
        assert!(state.form.as_ref().unwrap().original.is_some());
        let mut edited = state.form.as_ref().unwrap().draft.clone();
        edited.phone = "555-0100".into();
        let edit = HiringAction::Edit {
            draft: edited.clone(),
            expected_revision: 0,
            reason: "Corrected contact".into(),
        };
        state.form.as_mut().unwrap().draft = edited.clone();
        state.pending = Some(edit.clone());
        assert!(!state.accept(&data));
        assert_eq!(state.form.as_ref().unwrap().draft, edited);
        hiring::apply_demo(&mut data, &edit, chrono::Utc::now()).unwrap();
        state.pending = Some(edit);
        assert!(state.accept(&data));
        assert_eq!(
            state
                .form
                .as_ref()
                .unwrap()
                .original
                .as_ref()
                .unwrap()
                .revision,
            1
        );
        // A delayed create response must not silently discard an older draft after another edit.
        state.form = Some(Editor {
            draft: draft.clone(),
            original: None,
            reason: "Application recorded".into(),
        });
        state.pending = Some(action);
        assert!(!state.accept(&data));
        assert_eq!(state.form.as_ref().unwrap().draft, draft);
    }
    #[test]
    fn application_history_includes_only_changed_fields_and_never_account_fields() {
        let mut data = DemoStore::new().data;
        let mut d = ApplicantDraft::new(data.company.today());
        d.full_name = "Fictional Applicant".into();
        d.position = "Associate".into();
        hiring::apply_demo(
            &mut data,
            &HiringAction::Create {
                draft: d,
                reason: "Created".into(),
            },
            chrono::Utc::now(),
        )
        .unwrap();
        let old = data.hiring.records[0].clone();
        let id = old.id;
        hiring::apply_demo(
            &mut data,
            &HiringAction::Stage {
                id,
                expected_revision: 0,
                next: Stage::Screening,
                reason: "Reviewed".into(),
            },
            chrono::Utc::now(),
        )
        .unwrap();
        let before = values(&old);
        let after = values(&data.hiring.records[0]);
        let changed: Vec<_> = before
            .iter()
            .zip(&after)
            .filter(|(a, b)| a.1 != b.1)
            .map(|(_, b)| b.0)
            .collect();
        assert_eq!(changed, vec!["Stage"]);
        assert_eq!(after.len(), 9);
    }
    #[test]
    fn applicant_history_cells_align_with_multiline_values_at_small_and_large_scale() {
        let mut data = DemoStore::new().data;
        let mut d = ApplicantDraft::new(data.company.today());
        d.full_name = "A long fictional applicant name to check wrapping".into();
        d.position = "Senior Human Resources Associate".into();
        d.notes = "First fictional line\nSecond fictional line\nThird fictional line".into();
        hiring::apply_demo(
            &mut data,
            &HiringAction::Create {
                draft: d,
                reason: "Created".into(),
            },
            chrono::Utc::now(),
        )
        .unwrap();
        for width in [450.0, 620.0, 900.0] {
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            for _ in 0..3 {
                let mut measured = Vec::new();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(width, 1200.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            measured = history_fields(ui, &data.hiring.events[0]);
                        });
                    },
                );
                output.textures_delta.clear();
                assert_eq!(measured.len(), 9);
                for (label, value) in measured {
                    assert!(
                        (label.top() - value.top()).abs() < 1.0,
                        "Misaligned applicant history: {label:?} {value:?}"
                    );
                    assert!(value.right() < width + 1.0, "History exceeds viewport");
                }
            }
        }
    }
}
