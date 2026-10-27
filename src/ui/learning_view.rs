use super::*;
use crate::learning::{
    self, CertificateDraft, EvidenceDraft, LearningAction as Change, TrainingDraft,
};
#[derive(Default)]
pub(super) struct State {
    pub training_form: Option<TrainingDraft>,
    pub certificate_form: Option<CertificateDraft>,
    pub evidence_form: Option<EvidenceDraft>,
    pub decision: Option<Change>,
    pub pending: Option<Change>,
    employee: Option<Uuid>,
    search: String,
    resolved: bool,
    archived: bool,
    pub certificates: bool,
}
impl State {
    pub(super) fn open_notice(&mut self,data:&Snapshot,employee:Uuid,id:Uuid,certificate:bool){
        self.employee=Some(employee);self.certificates=certificate;self.archived=false;
        self.search=if certificate{data.learning.certificates.iter().find(|r|r.id==id).map(|r|r.title.clone())}
            else{data.learning.training.iter().find(|r|r.id==id).map(|r|r.title.clone())}.unwrap_or_default();
    }
    pub fn editing(&self) -> bool {
        self.training_form.is_some()
            || self.certificate_form.is_some()
            || self.evidence_form.is_some()
            || self.decision.is_some()
    }
    pub fn accept(&mut self, data: &Snapshot) -> bool {
        let Some(a) = self.pending.take() else {
            return true;
        };
        if !a.saved(data) {
            return false;
        }
        self.training_form = None;
        self.certificate_form = None;
        self.evidence_form = None;
        self.decision = None;
        true
    }
}
fn area(ui: &mut Ui, label: &str, value: &mut String, rows: usize) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
}
fn dates(ui: &mut Ui, first: (&str, &mut String), second: (&str, &mut String)) {
    if ui.available_width() >= 500.0 {
        ui.columns(2, |cols| {
            field(&mut cols[0], first.0, first.1, "YYYY-MM-DD or blank");
            field(&mut cols[1], second.0, second.1, "YYYY-MM-DD or blank");
        });
    } else {
        field(ui, first.0, first.1, "YYYY-MM-DD or blank");
        field(ui, second.0, second.1, "YYYY-MM-DD or blank");
    }
}
fn controls(ui: &mut Ui, data: &Snapshot, a: &Change, label: &str) -> (bool, bool) {
    let valid = a.payload(data);
    if let Err(e) = &valid {
        ui.label(RichText::new(e.to_string()).color(muted(ui)));
    }
    let mut out = (false, false);
    ui.horizontal_wrapped(|ui| {
        out.0 = ui
            .add_enabled(valid.is_ok(), egui::Button::new(label))
            .clicked();
        out.1 = ui.button("Discard changes").clicked();
    });
    out
}
impl HrisApp {
    fn send_learning(&mut self, a: Change) {
        self.learning_state.pending = Some(a.clone());
        self.send(Command::Change(Action::Learning(a)));
    }
    pub(super) fn learning(&mut self, ui: &mut Ui, certificates: bool) {
        self.learning_state.certificates = certificates;
        if self.learning_decision(ui) {
            return;
        }
        if self.learning_state.training_form.is_some() {
            self.learning_training_form(ui);
            return;
        }
        if self.learning_state.certificate_form.is_some() {
            self.learning_certificate_form(ui);
            return;
        }
        if self.learning_state.evidence_form.is_some() {
            self.learning_evidence_form(ui);
            return;
        }
        let hr = self.data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration);
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if hr
                    && ui
                        .button(if certificates {
                            "Add certification"
                        } else {
                            "Assign training"
                        })
                        .clicked()
                {
                    let emp = self
                        .learning_state
                        .employee
                        .filter(|id| self.data.active().any(|e| e.id == *id));
                    if certificates {
                        self.learning_state.certificate_form =
                            Some(CertificateDraft::new(emp, self.data.company.today()));
                    } else {
                        self.learning_state.training_form = Some(TrainingDraft::new(emp));
                    }
                }
                if certificates {
                    ui.checkbox(&mut self.learning_state.archived, "Include archived");
                } else {
                    ui.checkbox(
                        &mut self.learning_state.resolved,
                        "Include completed / cancelled",
                    );
                }
            });
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("learning_employee_filter")
                    .width(280.0)
                    .selected_text(
                        self.learning_state
                            .employee
                            .map(|id| self.data.employee_name(id))
                            .unwrap_or("All employees in your access"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.learning_state.employee,
                            None,
                            "All employees in your access",
                        );
                        for e in self
                            .data
                            .employees
                            .iter()
                            .filter(|e| learning::in_scope(&self.data, e.id))
                        {
                            ui.selectable_value(
                                &mut self.learning_state.employee,
                                Some(e.id),
                                format!("{} · {}", e.employee_code, e.full_name),
                            );
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut self.learning_state.search)
                        .hint_text("Search employee, title or provider")
                        .desired_width(260.0),
                );
            });
            ui.label(if certificates{"Recorded certificates and expiry dates; HR retains file evidence separately in private 201 files. "}else{"HR assigns training and records completion. Staff can record their own progress; current reporting Managers can also record team progress. Dates do not change attendance or leave."});
        });
        ui.add_space(10.0);
        let search = self.learning_state.search.to_lowercase();
        let employee = self.learning_state.employee;
        let mut found = false;
        if certificates {
            let mut rows: Vec<_> = self
                .data
                .learning
                .certificates
                .iter()
                .filter(|c| {
                    learning::in_scope(&self.data, c.employee_id)
                        && employee.is_none_or(|id| c.employee_id == id)
                        && (self.learning_state.archived || !c.archived)
                        && format!(
                            "{} {} {} {}",
                            c.employee_name, c.employee_code, c.title, c.issuer
                        )
                        .to_lowercase()
                        .contains(&search)
                })
                .cloned()
                .collect();
            rows.sort_by_key(|c| {
                (
                    c.archived,
                    c.employee_name.clone(),
                    c.expires_on,
                    c.title.clone(),
                )
            });
            for c in rows {
                found = true;
                self.learning_certificate_card(ui, &c);
                ui.add_space(10.0);
            }
        } else {
            let mut rows: Vec<_> = self
                .data
                .learning
                .training
                .iter()
                .filter(|t| {
                    learning::in_scope(&self.data, t.employee_id)
                        && employee.is_none_or(|id| t.employee_id == id)
                        && (self.learning_state.resolved
                            || !["completed", "cancelled"].contains(&t.status.as_str()))
                        && format!(
                            "{} {} {} {}",
                            t.employee_name, t.employee_code, t.title, t.provider
                        )
                        .to_lowercase()
                        .contains(&search)
                })
                .cloned()
                .collect();
            rows.sort_by_key(|t| (t.employee_name.clone(), t.due_on, t.title.clone()));
            for t in rows {
                found = true;
                self.learning_training_card(ui, &t);
                ui.add_space(10.0);
            }
        }
        if !found {
            card(ui, |ui| {
                ui.label(if certificates {
                    "No certifications match these filters."
                } else {
                    "No training matches these filters."
                });
            });
        }
    }
    fn learning_training_card(&mut self, ui: &mut Ui, t: &learning::Training) {
        let hr = self.data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration);
        card(ui, |ui| {
            ui.label(RichText::new(&t.title).strong().size(18.0));
            ui.label(format!("{} · {}", t.employee_code, t.employee_name));
            ui.label(format!(
                "{}{} · Provider: {}",
                t.status.replace('_', " "),
                if t.required { " · Required" } else { "" },
                if t.provider.is_empty() {
                    "Not set"
                } else {
                    &t.provider
                }
            ));
            ui.label(format!(
                "Start: {} · End: {} · Due: {}",
                date_or(t.starts_on),
                date_or(t.ends_on),
                date_or(t.due_on)
            ));
            if !t.progress_note.is_empty() {
                ui.label(RichText::new("Recorded progress").strong());
                ui.label(&t.progress_note);
            }
            if let Some(date) = t.completed_on {
                ui.label(format!(
                    "Completed {date} · Learning minutes: {}",
                    t.learning_minutes
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "Not recorded".into())
                ));
                ui.label(&t.outcome);
            }
            ui.horizontal_wrapped(|ui| {
                if hr && ui.button("Edit training").clicked() {
                    self.learning_state.training_form = Some(TrainingDraft::existing(t));
                }
                let p = Change::Progress {
                    id: t.id,
                    revision: t.revision,
                    note: t.progress_note.clone(),
                    reason: "Preview availability".into(),
                };
                if p.payload(&self.data).is_ok() && ui.button("Record progress").clicked() {
                    let mut a = p;
                    a.reason_mut().clear();
                    self.learning_state.decision = Some(a);
                }
                if hr {
                    for (status, label) in [
                        ("in_progress", "Start / reopen"),
                        (
                            "completed",
                            if t.status == "completed" {
                                "Correct completion"
                            } else {
                                "Record completion"
                            },
                        ),
                        ("cancelled", "Cancel training"),
                    ] {
                        if status == t.status && status != "completed" {
                            continue;
                        }
                        if status == "in_progress"
                            && !self.data.active().any(|e| e.id == t.employee_id)
                        {
                            continue;
                        }
                        if ui.button(label).clicked() {
                            self.learning_state.decision = Some(Change::TrainingStatus {
                                id: t.id,
                                revision: t.revision,
                                status: status.into(),
                                completed_on: if status == "completed" {
                                    t.completed_on
                                        .unwrap_or(self.data.company.today())
                                        .to_string()
                                } else {
                                    String::new()
                                },
                                learning_minutes: if status == "completed" {
                                    t.learning_minutes
                                } else {
                                    None
                                },
                                outcome: if status == "completed" {
                                    t.outcome.clone()
                                } else {
                                    String::new()
                                },
                                reason: String::new(),
                            });
                        }
                    }
                }
            });
            self.learning_history(ui, Some(t.id), None);
        });
    }
    fn learning_certificate_card(&mut self, ui: &mut Ui, c: &learning::Certificate) {
        let hr = self.data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration);
        card(ui, |ui| {
            ui.label(RichText::new(&c.title).strong().size(18.0));
            ui.label(format!("{} · {}", c.employee_code, c.employee_name));
            ui.label(format!(
                "{} · Issuer: {}",
                c.label(self.data.company.today()),
                if c.issuer.is_empty() {
                    "Not set"
                } else {
                    &c.issuer
                }
            ));
            ui.label(format!(
                "Issued: {} · Expires: {}",
                c.issued_on,
                date_or(c.expires_on)
            ));
            if !c.reference_number.is_empty() {
                ui.label(format!("Reference: {}", c.reference_number));
            }
            if hr || self.data.access.allows(crate::staff_roles::Capability::PersonnelDocuments) {
                ui.horizontal_wrapped(|ui| {
                    if hr && !c.archived && ui.button("Edit certification").clicked() {
                        self.learning_state.certificate_form = Some(CertificateDraft::existing(c));
                    }
                    if self.data.access.allows(crate::staff_roles::Capability::PersonnelDocuments) && !c.archived && ui.button("Private file evidence").clicked() {
                        self.learning_state.evidence_form = Some(
                            self.data
                                .learning
                                .evidence
                                .iter()
                                .find(|e| e.certificate_id == c.id)
                                .map(EvidenceDraft::existing)
                                .unwrap_or_else(|| EvidenceDraft::new(c.id)),
                        );
                    }
                    let a = Change::CertificateArchive {
                        id: c.id,
                        revision: c.revision,
                        archived: !c.archived,
                        reason: String::new(),
                    };
                    if hr && ui
                        .button(if c.archived { "Restore" } else { "Archive" })
                        .clicked()
                    {
                        self.learning_state.decision = Some(a);
                    }
                    if hr && !c.archived
                        && ui
                            .button(if c.status == "valid" {
                                "Revoke"
                            } else {
                                "Reinstate recorded validity"
                            })
                            .clicked()
                    {
                        self.learning_state.decision = Some(Change::CertificateStatus {
                            id: c.id,
                            revision: c.revision,
                            status: if c.status == "valid" {
                                "revoked"
                            } else {
                                "valid"
                            }
                            .into(),
                            reason: String::new(),
                        });
                    }
                });
                if self.data.access.allows(crate::staff_roles::Capability::PersonnelDocuments) {
                if let Some(doc) = self
                    .data
                    .learning
                    .evidence
                    .iter()
                    .find(|e| e.certificate_id == c.id)
                    .and_then(|e| e.document_id)
                    .and_then(|id| self.data.documents.iter().find(|d| d.id == id))
                {
                    ui.label(format!(
                        "Private HR evidence: {}{}",
                        doc.details.title,
                        if doc.archived {
                            " (document archived)"
                        } else {
                            ""
                        }
                    ));
                }
                if ui.button("Open employee 201 files").clicked() {
                    self.open_employee(c.employee_id);
                    self.contacts_section = ContactSection::Documents;
                }
                }
            }
            self.learning_history(ui, None, Some(c.id));
        });
    }
    fn learning_history(&self, ui: &mut Ui, training: Option<Uuid>, certificate: Option<Uuid>) {
        let events = learning::visible_events(&self.data, training, certificate);
        egui::CollapsingHeader::new(format!("Recorded history ({})", events.len()))
            .id_salt((training, certificate, "learning_history"))
            .default_open(learning_history_preview())
            .show(ui, |ui| {
                if events.is_empty() {
                    ui.label("No recorded changes.");
                }
                for e in events.iter().rev() {
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · {}",
                            e.kind.replace('_', " "),
                            e.actor_name,
                            e.recorded_at
                                .with_timezone(&self.data.company.tz())
                                .format("%b %d, %Y %H:%M")
                        ))
                        .strong(),
                    );
                    ui.label(&e.reason);
                    egui::CollapsingHeader::new("Saved change details")
                        .id_salt(e.id)
                        .default_open(learning_history_preview())
                        .show(ui, |ui| {
                            super::onboarding_view::record_change_fields(
                                ui,
                                e.id,
                                e.before_record.as_ref(),
                                &e.after_record,
                            )
                        });
                    ui.separator();
                }
            });
    }
    fn learning_training_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.learning_state.training_form.take() else {
            return;
        };
        let mut result = (false, false);
        card(ui, |ui| {
            ui.label(
                RichText::new(if d.original.is_some() {
                    "Edit training"
                } else {
                    "Assign training"
                })
                .strong()
                .size(20.0),
            );
            ui.add_enabled_ui(d.original.is_none(), |ui| {
                super::performance_view::employee_choice(
                    ui,
                    "learning_training_person",
                    &self.data,
                    &mut d.employee_id,
                    false,
                )
            });
            field(ui, "Training title *", &mut d.title, "");
            field(ui, "Provider", &mut d.provider, "");
            dates(
                ui,
                ("Starts on", &mut d.starts_on),
                ("Ends on", &mut d.ends_on),
            );
            field(ui, "Target due date", &mut d.due_on, "YYYY-MM-DD or blank");
            ui.checkbox(&mut d.required, "Required company training");
            field(ui, "Save reason *", &mut d.reason, "");
            ui.label("Reasons and progress stay with this record and may be visible to the employee and current reporting Manager. Record completion separately after saving.");
            result = controls(
                ui,
                &self.data,
                &Change::TrainingSave(d.clone()),
                "Save training",
            );
        });
        if !result.1 {
            self.learning_state.training_form = Some(d.clone());
        }
        if result.0 {
            self.send_learning(Change::TrainingSave(d));
        }
    }
    fn learning_certificate_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.learning_state.certificate_form.take() else {
            return;
        };
        let mut result = (false, false);
        card(ui, |ui| {
            ui.label(
                RichText::new(if d.original.is_some() {
                    "Edit certification"
                } else {
                    "Record certification"
                })
                .strong()
                .size(20.0),
            );
            ui.add_enabled_ui(d.original.is_none(), |ui| {
                super::performance_view::employee_choice(
                    ui,
                    "learning_certificate_person",
                    &self.data,
                    &mut d.employee_id,
                    false,
                )
            });
            field(ui, "Certificate title *", &mut d.title, "");
            field(ui, "Issuer", &mut d.issuer, "");
            field(ui, "Reference number", &mut d.reference_number, "");
            dates(
                ui,
                ("Issued on *", &mut d.issued_on),
                ("Expires on", &mut d.expires_on),
            );
            field(ui, "Save reason *", &mut d.reason, "");
            ui.label("These certificate details are visible to the employee and current reporting Manager. Add private file evidence after saving, using an existing employee 201 document.");
            result = controls(
                ui,
                &self.data,
                &Change::CertificateSave(d.clone()),
                "Save certification",
            );
        });
        if !result.1 {
            self.learning_state.certificate_form = Some(d.clone());
        }
        if result.0 {
            self.send_learning(Change::CertificateSave(d));
        }
    }
    fn learning_evidence_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.learning_state.evidence_form.take() else {
            return;
        };
        let mut result = (false, false);
        card(ui, |ui| {
            ui.label(
                RichText::new("Private certification evidence")
                    .strong()
                    .size(20.0),
            );
            ui.label("Only HR and Administrators receive the file link and its history. Upload or download originals through the employee's 201 files. Replacing/clearing a link does not delete the original document.");
            if let Ok(c) = learning::certificate(&self.data, d.certificate_id) {
                ui.label(format!("{} · {}", c.employee_name, c.title));
                egui::ComboBox::from_id_salt("certification_document")
                    .width(ui.available_width().min(500.0))
                    .selected_text(
                        d.document_id
                            .and_then(|id| self.data.documents.iter().find(|doc| doc.id == id))
                            .map(|doc| doc.details.title.as_str())
                            .unwrap_or("No linked document"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut d.document_id, None, "No linked document");
                        for doc in self.data.documents.iter().filter(|doc| {
                            doc.employee_id == c.employee_id
                                && doc.state == DocumentState::Ready
                                && !doc.archived
                        }) {
                            ui.selectable_value(
                                &mut d.document_id,
                                Some(doc.id),
                                format!("{} · {}", doc.details.title, doc.original_filename),
                            );
                        }
                    });
            }
            field(ui, "Private change reason *", &mut d.reason, "");
            result = controls(
                ui,
                &self.data,
                &Change::EvidenceSave(d.clone()),
                "Save private evidence link",
            );
        });
        if !result.1 {
            self.learning_state.evidence_form = Some(d.clone());
        }
        if result.0 {
            self.send_learning(Change::EvidenceSave(d));
        }
    }
    fn learning_decision(&mut self, ui: &mut Ui) -> bool {
        let Some(mut a) = self.learning_state.decision.take() else {
            return false;
        };
        let mut result = (false, false);
        card(ui, |ui| {
            ui.label(
                RichText::new(match &a {
                    Change::Progress { .. } => "Record learning progress",
                    Change::TrainingStatus { status, .. } if status == "completed" => {
                        "Record / correct training completion"
                    }
                    Change::TrainingStatus { .. } => "Change training status",
                    Change::CertificateArchive { archived: true, .. } => "Archive certification",
                    Change::CertificateArchive { .. } => "Restore certification",
                    Change::CertificateStatus { status, .. } if status == "revoked" => {
                        "Revoke recorded certification"
                    }
                    _ => "Reinstate recorded validity",
                })
                .strong()
                .size(20.0),
            );
            if let Change::CertificateStatus { id, .. } | Change::CertificateArchive { id, .. } = &a
            {
                if let Ok(c) = learning::certificate(&self.data, *id) {
                    ui.label(format!("{} · {}", c.employee_name, c.title));
                }
            }
            match &mut a {
                Change::Progress { id, note, .. } => {
                    if let Ok(t) = learning::training(&self.data, *id) {
                        ui.label(format!("{} · {}", t.employee_name, t.title));
                    }
                    area(ui, "Progress visible with this record", note, 4);
                }
                Change::TrainingStatus {
                    id,
                    status,
                    completed_on,
                    learning_minutes,
                    outcome,
                    ..
                } => {
                    if let Ok(t) = learning::training(&self.data, *id) {
                        ui.label(format!("{} · {}", t.employee_name, t.title));
                    }
                    if status == "completed" {
                        field(ui, "Completed on *", completed_on, "YYYY-MM-DD");
                        ui.horizontal_wrapped(|ui| {
                            let mut recorded = learning_minutes.is_some();
                            if ui
                                .checkbox(&mut recorded, "Record learning minutes")
                                .changed()
                            {
                                *learning_minutes = if recorded { Some(0) } else { None };
                            }
                            if let Some(minutes) = learning_minutes {
                                ui.add(
                                    egui::DragValue::new(minutes)
                                        .range(0..=600000)
                                        .suffix(" min"),
                                );
                            }
                        });
                        area(ui, "Outcome / completion evidence *", outcome, 4);
                    } else {
                        ui.label(format!(
                            "New status: {}. Previous completion facts remain in history.",
                            status.replace('_', " ")
                        ));
                    }
                }
                Change::CertificateStatus { .. } => {
                    ui.label("This changes the recorded validity state. The recorded expiry date stays the same.");
                }
                _ => {
                    ui.label("The certificate and its previous file/history remain retained.");
                }
            }
            field(ui, "Reason *", a.reason_mut(), "");
            result = controls(ui, &self.data, &a, "Confirm and save");
        });
        if !result.1 {
            self.learning_state.decision = Some(a.clone());
        }
        if result.0 {
            self.send_learning(a);
        }
        true
    }
}
fn date_or(d: Option<NaiveDate>) -> String {
    d.map(|d| d.to_string()).unwrap_or_else(|| "Not set".into())
}
fn learning_history_preview() -> bool {
    #[cfg(feature = "qa-screenshot")]
    {
        std::env::var("HRIS_PREVIEW_FORM").is_ok_and(|s| s == "learn-history")
    }
    #[cfg(not(feature = "qa-screenshot"))]
    {
        false
    }
}
#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_learning_preview(&mut self) {
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("learn-") || self.data.mode != Mode::Demo {
            return;
        }
        let access = self.data.access.clone();
        let mut data = self.data.clone();
        data.access.role = Role::Administrator;
        let mut employee = data.employees[0].clone();
        employee.id = Uuid::new_v4();
        employee.employee_code = "EMP-LRN-001".into();
        employee.full_name = "Fictional Learning Employee".into();
        employee.reports_to = access.employee_id;
        employee.status = EmployeeStatus::Active;
        let person = employee.id;
        data.employees.push(employee);
        let today = data.company.today();
        let now = chrono::Utc::now();
        let mut td = TrainingDraft::new(Some(person));
        td.title = "Service quality, documentation and structured handover training".into();
        td.provider = "Fictional Learning Centre".into();
        td.starts_on = (today - Duration::days(5)).to_string();
        td.ends_on = today.to_string();
        td.due_on = (today + Duration::days(7)).to_string();
        td.required = true;
        td.reason = "Fictional employee training assignment".into();
        let tid = td.id;
        learning::apply_demo(&mut data, &Change::TrainingSave(td), now).unwrap();
        learning::apply_demo(&mut data,&Change::Progress{id:tid,revision:0,note:"Fictional progress: first learning session complete; a practice handover and feedback discussion are planned next.".into(),reason:"Fictional recorded learning progress".into()},now).unwrap();
        let mut cd = CertificateDraft::new(Some(person), today);
        cd.title = "Service documentation and professional communication".into();
        cd.issuer = "Fictional Learning Centre".into();
        cd.reference_number = "FICTIONAL-LRN-2026".into();
        cd.expires_on = (today + Duration::days(25)).to_string();
        cd.reason = "Fictional certification record".into();
        let cid = cd.id;
        learning::apply_demo(&mut data, &Change::CertificateSave(cd), now).unwrap();
        let doc = Uuid::new_v4();
        data.documents.push(crate::documents::EmployeeDocument {
            id: doc,
            company_id: data.company.id,
            employee_id: person,
            details: crate::documents::DocumentDetails {
                title: "Fictional course completion certificate".into(),
                category: DocumentCategory::Certificate,
                notes: String::new(),
                issued_on: Some(today),
                expires_on: None,
                contract_start: None,
                contract_end: None,
            },
            original_filename: "fictional-certificate.pdf".into(),
            mime_type: "application/pdf".into(),
            byte_size: 100,
            sha256: "a".repeat(64),
            object_name: format!("{}/{person}/{doc}/file.pdf", data.company.id),
            state: DocumentState::Ready,
            archived: false,
            revision: 0,
            created_by: access.user_id,
            created_by_name: "Fictional HR".into(),
            created_at: now,
            updated_at: now,
        });
        let mut ed = EvidenceDraft::new(cid);
        ed.document_id = Some(doc);
        ed.reason = "Fictional private certification evidence".into();
        learning::apply_demo(&mut data, &Change::EvidenceSave(ed), now).unwrap();
        if preview == "learn-completed" || preview == "learn-history" {
            learning::apply_demo(&mut data,&Change::TrainingStatus{id:tid,revision:1,status:"completed".into(),completed_on:today.to_string(),learning_minutes:Some(90),outcome:"Fictional outcome: completed the agreed handover checklist and demonstrated clear, accurate service documentation.".into(),reason:"Fictional HR completion review".into()},now).unwrap();
            self.learning_state.resolved = true;
        }
        data.access = access;
        let certificates = preview.contains("certificate") || preview.contains("evidence");
        self.select_learning_tab(certificates);
        self.learning_state.certificates = certificates;
        self.employee_view = EmployeeView::Development;
        self.learning_state.employee = Some(person);
        self.data = data;
        match preview.as_str(){"learn-training-form"=>{let mut d=TrainingDraft::existing(&self.data.learning.training[0]);d.reason="Fictional training schedule correction".into();self.learning_state.training_form=Some(d);},"learn-certificate-form"=>{let mut d=CertificateDraft::existing(&self.data.learning.certificates[0]);d.reason="Fictional issuer/reference confirmation".into();self.learning_state.certificate_form=Some(d);},"learn-evidence"=>{let mut d=EvidenceDraft::existing(&self.data.learning.evidence[0]);d.reason="Fictional private file reference check".into();self.learning_state.evidence_form=Some(d);},"learn-progress"|"learn-manager-progress"=>self.learning_state.decision=Some(Change::Progress{id:tid,revision:1,note:"Fictional practice handover completed; scheduled feedback discussion follows.".into(),reason:"Fictional progress evidence".into()}),"learn-completion"=>self.learning_state.decision=Some(Change::TrainingStatus{id:tid,revision:1,status:"completed".into(),completed_on:today.to_string(),learning_minutes:Some(90),outcome:"Fictional completion: demonstrated the agreed handover and service-documentation checklist.".into(),reason:"Fictional HR learning completion".into()}),"learn-certificate-revoke"=>self.learning_state.decision=Some(Change::CertificateStatus{id:cid,revision:0,status:"revoked".into(),reason:"Fictional recorded revocation reference".into()}),"learn-certificate-archive"=>self.learning_state.decision=Some(Change::CertificateArchive{id:cid,revision:0,archived:true,reason:"Fictional certificate archival".into()}),_=>{}}
        if !self.data.access.allows(crate::staff_roles::Capability::PersonnelDocuments) {
            self.data.documents.clear();
            self.data.learning.evidence.clear();
            self.data
                .learning
                .events
                .retain(|e| e.evidence_id.is_none());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_refresh_preserves_learning_draft() {
        let data = DemoStore::new().data;
        let mut state = State {
            training_form: Some(TrainingDraft::new(data.access.employee_id)),
            ..Default::default()
        };
        assert!(state.accept(&data));
        assert!(state.editing());
    }
    #[test]
    fn strict_learning_receipt_preserves_a_draft_on_mismatch() {
        let mut data = DemoStore::new().data;
        let mut d = TrainingDraft::new(data.access.employee_id);
        d.title = "Synthetic learning".into();
        d.reason = "Synthetic assignment".into();
        let a = Change::TrainingSave(d.clone());
        learning::apply_demo(&mut data, &a, chrono::Utc::now()).unwrap();
        data.learning.training[0].title = "Another session changed this title".into();
        let mut state = State {
            training_form: Some(d),
            pending: Some(a),
            ..Default::default()
        };
        assert!(!state.accept(&data));
        assert!(state.training_form.is_some());
        assert!(state.pending.is_none());
    }
}
