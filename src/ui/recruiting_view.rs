use super::*;
use crate::hiring;
use crate::recruiting::{
    self, FeedbackDraft, InterviewAction, InterviewDraft, InterviewStatus, Recommendation,
    ResumeAction, ResumeDraft,
};
#[derive(Default)]
pub(super) struct State {
    pub agenda: bool,
    pub resume_form: Option<ResumeDraft>,
    pub resume_pending: Option<ResumeAction>,
    pub interview_form: Option<InterviewDraft>,
    pub detail: Option<Uuid>,
    pub feedback_form: Option<FeedbackDraft>,
    pub pending: Option<InterviewAction>,
    pub status: Option<InterviewStatus>,
    pub reason: String,
    pub search: String,
    pub show_closed: bool,
    pub include_archived: bool,
}
impl State {
    pub fn editing(&self) -> bool {
        self.resume_form.is_some()
            || self.interview_form.is_some()
            || self.feedback_form.is_some()
            || self.status.is_some()
    }
    pub fn accept(&mut self, data: &Snapshot) -> bool {
        let mut matched = true;
        if let Some(action) = self.pending.take() {
            matched = match &action {
                InterviewAction::Save(d) => data.recruiting.interviews.iter().any(|r| {
                    r.id == d.id
                        && r.title == d.title.trim()
                        && r.interviewer_employee_id == d.interviewer.unwrap_or_default()
                        && r.revision == d.original.as_ref().map_or(0, |r| r.revision + 1)
                }),
                InterviewAction::Status {
                    id,
                    expected_revision,
                    status,
                    ..
                } => data.recruiting.interviews.iter().any(|r| {
                    r.id == *id && r.revision == expected_revision + 1 && r.status == *status
                }),
                InterviewAction::Feedback(d) => data.recruiting.feedback.iter().any(|f| {
                    f.interview_id == d.interview_id
                        && f.author_id == data.access.user_id
                        && f.revision == d.expected_revision + 1
                        && f.assessment == d.assessment.trim()
                }),
            };
            if matched {
                self.detail = Some(match action {
                    InterviewAction::Save(d) => d.id,
                    InterviewAction::Status { id, .. } => id,
                    InterviewAction::Feedback(d) => d.interview_id,
                });
                self.interview_form = None;
                self.feedback_form = None;
                self.status = None;
                self.reason.clear();
            }
        }
        if let Some(action) = self.resume_pending.take() {
            let good = match &action {
                ResumeAction::Upload(d) => data.recruiting.documents.iter().any(|r| {
                    r.id == d.document.id
                        && r.state == DocumentState::Ready
                        && r.details.title == d.document.title.trim()
                }),
                ResumeAction::Edit(d) => data.recruiting.documents.iter().any(|r| {
                    r.id == d.document.id
                        && r.revision == d.document.original.as_ref().map_or(0, |r| r.revision + 1)
                        && r.details.title == d.document.title.trim()
                        && r.details.notes == d.document.notes.trim()
                }),
                ResumeAction::Archive {
                    id,
                    revision,
                    archived,
                } => {
                    data.recruiting.documents.iter().any(|r| {
                        r.id == *id && r.revision == revision + 1 && r.archived == *archived
                    })
                }
                ResumeAction::Discard { id } => data
                    .recruiting
                    .documents
                    .iter()
                    .any(|r| r.id == *id && r.state == DocumentState::Discarded),
                ResumeAction::Download { .. } => true,
            };
            matched &= good;
            if good
                && !matches!(
                    action,
                    ResumeAction::Download { .. } | ResumeAction::Archive { .. }
                )
            {
                self.resume_form = None;
            }
        }
        matched
    }
}
impl HrisApp {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_recruiting_preview(&mut self) {
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("recruit-") || self.data.mode != Mode::Demo {
            return;
        }
        let access = self.data.access.clone();
        let mut data = self.data.clone();
        data.access.role = Role::Administrator;
        let own = access.employee_id.unwrap();
        if data.hiring.records.is_empty() {
            let mut d = hiring::ApplicantDraft::new(data.company.today());
            d.full_name = "Fictional Interview Candidate".into();
            d.position = "Operations Associate".into();
            hiring::apply_demo(
                &mut data,
                &hiring::HiringAction::Create {
                    draft: d,
                    reason: "Fictional application received".into(),
                },
                chrono::Utc::now(),
            )
            .unwrap();
        }
        let app = data.hiring.records[0].clone();
        data.recruiting.interviewers = vec![recruiting::Interviewer {
            employee_id: own,
            full_name: data.employee_name(own).into(),
            role: access.role,
        }];
        let mut past = InterviewDraft::new(&data, app.id);
        past.interviewer = Some(own);
        past.title = "Initial skills and experience interview".into();
        past.date = (data.company.today() - Duration::days(2)).to_string();
        past.location = "Fictional meeting room 2".into();
        past.brief="Discuss relevant experience and problem-solving skills.\nKeep feedback specific to the applicant's demonstrated skills.".into();
        recruiting::apply_demo(
            &mut data,
            &InterviewAction::Save(past.clone()),
            chrono::Utc::now(),
        )
        .unwrap();
        let mut future = past.clone();
        future.id = Uuid::new_v4();
        future.date = (data.company.today() + Duration::days(2)).to_string();
        future.title = "Follow-up interview".into();
        recruiting::apply_demo(
            &mut data,
            &InterviewAction::Save(future),
            chrono::Utc::now(),
        )
        .unwrap();
        data.access = access.clone();
        if preview == "recruit-feedback-saved" {
            let mut d = FeedbackDraft::new(&data, past.id);
            d.rating = 4;
            d.recommendation = Recommendation::Proceed;
            d.assessment="Fictional feedback: clearly explained relevant experience, demonstrated sound problem-solving, and identified training needs.".into();
            recruiting::apply_demo(&mut data, &InterviewAction::Feedback(d), chrono::Utc::now())
                .unwrap();
        }
        if access.role.manages_hr() {
            let now = chrono::Utc::now();
            let id = Uuid::new_v4();
            let mime = "application/pdf";
            let doc = recruiting::Attachment {
                id,
                company_id: data.company.id,
                applicant_id: app.id,
                details: crate::documents::DocumentDetails {
                    title: "Résumé - application copy".into(),
                    category: DocumentCategory::Other,
                    notes: "Fictional résumé submitted with the application.".into(),
                    issued_on: None,
                    expires_on: None,
                    contract_start: None,
                    contract_end: None,
                },
                original_filename: "fictional-resume.pdf".into(),
                mime_type: mime.into(),
                byte_size: 48000,
                sha256: "a".repeat(64),
                object_name: documents::object_name(data.company.id, app.id, id, mime).unwrap(),
                state: DocumentState::Ready,
                archived: false,
                revision: 1,
                created_by: data.access.user_id,
                created_by_name: data.employee_name(own).into(),
                created_at: now,
                updated_at: now,
            };
            data.recruiting
                .document_events
                .push(recruiting::AttachmentEvent {
                    id: Uuid::new_v4(),
                    sequence: 1,
                    company_id: data.company.id,
                    applicant_id: app.id,
                    document_id: id,
                    kind: "uploaded".into(),
                    actor_id: data.access.user_id,
                    actor_name: data.employee_name(own).into(),
                    actor_role: access.role,
                    before_record: None,
                    after_record: doc.clone(),
                    recorded_at: now,
                });
            data.recruiting.documents.push(doc);
        } else {
            data.hiring = Default::default();
            data.recruiting.interviewers.clear();
        }
        self.data = data;
        self.recruiting_state.agenda = true;
        if preview.starts_with("recruit-resume") && access.role.manages_hr() {
            self.recruiting_state.agenda = false;
            self.hiring_state.open(app.clone());
            self.hiring_state.tab = super::hiring_view::Tab::Attachments;
            if preview == "recruit-resume-form" {
                self.recruiting_state.resume_form = Some(ResumeDraft::new(app.id));
            }
        } else if preview == "recruit-schedule" {
            self.recruiting_state.interview_form = Some(InterviewDraft::new(&self.data, app.id));
            self.recruiting_state
                .interview_form
                .as_mut()
                .unwrap()
                .interviewer = Some(own);
        } else if preview == "recruit-feedback"
            || preview == "recruit-feedback-saved"
            || preview == "recruit-detail"
        {
            self.recruiting_state.detail = Some(past.id);
            if preview == "recruit-feedback" {
                let mut d = FeedbackDraft::new(&self.data, past.id);
                d.assessment="Fictional assessment: clear communication, relevant experience and a practical approach to solving the exercise.".into();
                self.recruiting_state.feedback_form = Some(d);
            }
        }
    }
    pub(super) fn applicant_attachments(&mut self, ui: &mut Ui, applicant: Uuid) {
        if recruiting::authorize_attachment(&self.data, applicant).is_err() {
            return;
        }
        let archived = hiring::record(&self.data, applicant).is_ok_and(|a| a.archived);
        card(ui, |ui| {
            ui.label(
                RichText::new("Résumés and application attachments")
                    .strong()
                    .size(18.0),
            );
            ui.label("Private PDF, DOCX, PNG or JPEG files, up to 10 MiB. Original files and history are retained.");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !archived && self.recruiting_state.resume_form.is_none(),
                        egui::Button::new("Add résumé / attachment"),
                    )
                    .clicked()
                {
                    self.recruiting_state.resume_form = Some(ResumeDraft::new(applicant));
                }
                ui.checkbox(
                    &mut self.recruiting_state.include_archived,
                    "Include archived files",
                );
            });
        });
        ui.add_space(10.0);
        if let Some(mut draft) = self.recruiting_state.resume_form.take() {
            let mut close = false;
            let mut action = None;
            card(ui, |ui| {
                ui.label(
                    RichText::new(
                        if draft
                            .document
                            .original
                            .as_ref()
                            .is_some_and(|d| d.state == DocumentState::Ready)
                        {
                            "Edit attachment details"
                        } else {
                            "Upload application attachment"
                        },
                    )
                    .strong(),
                );
                field(
                    ui,
                    "Title *",
                    &mut draft.document.title,
                    "e.g. Résumé, cover letter, portfolio",
                );
                ui.label("Notes");
                ui.add(
                    egui::TextEdit::multiline(&mut draft.document.notes)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                let metadata = draft
                    .document
                    .original
                    .as_ref()
                    .is_some_and(|d| d.state == DocumentState::Ready);
                if !metadata {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Choose file").clicked() {
                            match documents::choose_file(None) {
                                Ok(Some(p)) => draft.document.file = Some(p),
                                Err(e) => self.notice = Some((e.to_string(), true)),
                                _ => {}
                            }
                        }
                        ui.label(
                            draft
                                .document
                                .file
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "No file selected".into()),
                        );
                    });
                }
                let ready = if metadata {
                    draft.payload().map(|_| ())
                } else {
                    draft.prepared(&self.data).map(|_| ())
                };
                if let Err(e) = &ready {
                    ui.label(RichText::new(e.to_string()).small().color(muted(ui)));
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            ready.is_ok(),
                            egui::Button::new(if metadata {
                                "Save attachment details"
                            } else {
                                "Upload and file"
                            }),
                        )
                        .clicked()
                    {
                        action = Some(if metadata {
                            ResumeAction::Edit(draft.clone())
                        } else {
                            ResumeAction::Upload(draft.clone())
                        });
                    }
                    if ui.button("Discard attachment draft").clicked() {
                        close = true;
                    }
                });
            });
            if !close {
                self.recruiting_state.resume_form = Some(draft);
            }
            if let Some(action) = action {
                self.send_resume(action);
            }
            ui.add_space(10.0);
        }
        let mut docs: Vec<_> = self
            .data
            .recruiting
            .documents
            .iter()
            .filter(|d| {
                d.applicant_id == applicant
                    && d.company_id == self.data.company.id
                    && (self.recruiting_state.include_archived || !d.archived)
            })
            .cloned()
            .collect();
        docs.sort_by_key(|d| std::cmp::Reverse(d.created_at));
        if docs.is_empty() {
            empty(ui, "No application attachments have been filed.");
        }
        for doc in docs {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&doc.details.title).strong());
                    badge(
                        ui,
                        match doc.state {
                            DocumentState::Pending => "Incomplete upload",
                            DocumentState::Discarded => "Discarded",
                            DocumentState::Ready if doc.archived => "Archived",
                            _ => "Filed",
                        },
                        doc.state == DocumentState::Ready && !doc.archived,
                    );
                });
                ui.label(format!(
                    "{} · {:.1} KiB",
                    doc.original_filename,
                    doc.byte_size as f32 / 1024.0
                ));
                if !doc.details.notes.is_empty() {
                    ui.label(&doc.details.notes);
                }
                ui.label(
                    RichText::new(format!(
                        "Added by {} · {}",
                        doc.created_by_name,
                        doc.created_at
                            .with_timezone(&self.data.company.tz())
                            .format("%b %d, %Y %H:%M")
                    ))
                    .small()
                    .color(muted(ui)),
                );
                ui.horizontal_wrapped(|ui| {
                    if doc.state == DocumentState::Ready {
                        if ui.button("Download original").clicked() {
                            match documents::choose_file(Some(&doc.original_filename)) {
                                Ok(Some(destination)) => self.send_resume(ResumeAction::Download {
                                    id: doc.id,
                                    destination,
                                }),
                                Err(e) => self.notice = Some((e.to_string(), true)),
                                _ => {}
                            }
                        }
                        if ui
                            .add_enabled(
                                self.recruiting_state.resume_form.is_none(),
                                egui::Button::new("Edit details"),
                            )
                            .clicked()
                        {
                            match ResumeDraft::existing(&doc) {
                                Ok(d) => self.recruiting_state.resume_form = Some(d),
                                Err(e) => self.notice = Some((e.to_string(), true)),
                            }
                        }
                        if ui
                            .button(if doc.archived {
                                "Restore file"
                            } else {
                                "Archive file"
                            })
                            .clicked()
                        {
                            self.send_resume(ResumeAction::Archive {
                                id: doc.id,
                                revision: doc.revision,
                                archived: !doc.archived,
                            });
                        }
                    } else {
                        if doc.state == DocumentState::Pending
                            && ui
                                .add_enabled(
                                    !archived && self.recruiting_state.resume_form.is_none(),
                                    egui::Button::new("Retry same file"),
                                )
                                .clicked()
                        {
                            if let Ok(d) = ResumeDraft::existing(&doc) {
                                self.recruiting_state.resume_form = Some(d);
                            }
                        }
                        if ui
                            .button(if doc.state == DocumentState::Discarded {
                                "Retry unused-file cleanup"
                            } else {
                                "Discard incomplete upload"
                            })
                            .clicked()
                        {
                            self.send_resume(ResumeAction::Discard { id: doc.id });
                        }
                    }
                });
                egui::CollapsingHeader::new("File history")
                    .id_salt(("resume_history", doc.id))
                    .show(ui, |ui| {
                        let mut events: Vec<_> = self
                            .data
                            .recruiting
                            .document_events
                            .iter()
                            .filter(|e| e.document_id == doc.id)
                            .collect();
                        events.sort_by_key(|e| std::cmp::Reverse(e.sequence));
                        for e in events {
                            ui.label(format!(
                                "{} · {} · {}",
                                e.kind.replace('_', " "),
                                e.actor_name,
                                e.recorded_at
                                    .with_timezone(&self.data.company.tz())
                                    .format("%b %d, %Y %H:%M")
                            ));
                            ui.label(format!(
                                "Saved: {}{}",
                                e.after_record.details.title,
                                if e.after_record.archived {
                                    " (archived)"
                                } else {
                                    ""
                                }
                            ));
                            if let Some(old) = &e.before_record
                                && old.details != e.after_record.details
                            {
                                ui.label(format!("Previous title: {}", old.details.title));
                                ui.label(format!("Previous notes: {}", old.details.notes));
                            }
                            ui.separator();
                        }
                    });
            });
            ui.add_space(10.0);
        }
    }
    fn send_resume(&mut self, action: ResumeAction) {
        self.recruiting_state.resume_pending = Some(action.clone());
        self.send(Command::Resume(action));
    }
    pub(super) fn applicant_interviews(&mut self, ui: &mut Ui, applicant: Option<Uuid>) {
        let hr = self.data.access.role.manages_hr();
        if !self.data.access.role.uses_desktop() {
            empty(ui, "Staff access is required.");
            return;
        }
        if let Some(mut draft) = self.recruiting_state.interview_form.take() {
            let mut close = false;
            let mut save = false;
            card(ui, |ui| {
                ui.label(
                    RichText::new(if draft.original.is_some() {
                        "Reschedule interview"
                    } else {
                        "Schedule interview"
                    })
                    .strong()
                    .size(19.0),
                );
                ui.label(format!(
                    "Company time zone: {} · Use 24-hour times",
                    self.data.company.timezone
                ));
                field(
                    ui,
                    "Interview title *",
                    &mut draft.title,
                    "e.g. Initial HR interview",
                );
                ui.label("Interviewer *");
                egui::ComboBox::from_id_salt("interview_assignee")
                    .selected_text(
                        self.data
                            .recruiting
                            .interviewers
                            .iter()
                            .find(|s| Some(s.employee_id) == draft.interviewer)
                            .map(|s| s.full_name.as_str())
                            .unwrap_or("Choose active staff"),
                    )
                    .show_ui(ui, |ui| {
                        for s in &self.data.recruiting.interviewers {
                            ui.selectable_value(
                                &mut draft.interviewer,
                                Some(s.employee_id),
                                format!("{} ({})", s.full_name, s.role.label()),
                            );
                        }
                    });
                ui.columns(3, |cols| {
                    field(&mut cols[0], "Date *", &mut draft.date, "YYYY-MM-DD");
                    field(&mut cols[1], "Start *", &mut draft.start, "09:00");
                    field(&mut cols[2], "End *", &mut draft.end, "10:00");
                });
                field(
                    ui,
                    "Location / meeting details",
                    &mut draft.location,
                    "Room or meeting link",
                );
                ui.label("Brief visible to the assigned interviewer");
                ui.add(
                    egui::TextEdit::multiline(&mut draft.brief)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                field(
                    ui,
                    "Change reason *",
                    &mut draft.reason,
                    "Why this interview is being scheduled or changed",
                );
                let validation = draft.payload(&self.data);
                if let Err(e) = &validation {
                    ui.label(RichText::new(e.to_string()).small().color(muted(ui)));
                }
                ui.horizontal_wrapped(|ui| {
                    save = ui
                        .add_enabled(validation.is_ok(), egui::Button::new("Save interview"))
                        .clicked();
                    close = ui.button("Discard interview draft").clicked();
                });
            });
            if !close {
                self.recruiting_state.interview_form = Some(draft.clone());
            }
            if save {
                self.send_interview(InterviewAction::Save(draft));
            }
            return;
        }
        if let Some(id) = self.recruiting_state.detail {
            if let Ok(r) = recruiting::interview(&self.data, id).cloned() {
                self.interview_detail(ui, &r);
                return;
            }
            self.recruiting_state.detail = None;
        }
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(if hr {
                        "Interview agenda"
                    } else {
                        "Your assigned interviews"
                    })
                    .strong()
                    .size(19.0),
                );
                if hr && applicant.is_none() && ui.button("< Applicants").clicked() {
                    self.recruiting_state.agenda = false;
                }
            });
            if let Some(id) = applicant
                && hr
            {
                if ui
                    .add_enabled(
                        hiring::record(&self.data, id).is_ok_and(|a| !a.archived),
                        egui::Button::new("Schedule interview"),
                    )
                    .clicked()
                {
                    self.recruiting_state.interview_form =
                        Some(InterviewDraft::new(&self.data, id));
                }
            }
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.recruiting_state.search)
                        .hint_text("Search interview, applicant or position")
                        .desired_width(340.0),
                );
                ui.checkbox(
                    &mut self.recruiting_state.show_closed,
                    "Include completed / cancelled",
                );
            });
            if !hr {
                ui.label(RichText::new("Only your assigned interviews and your own feedback are visible. Applicant files and private HR notes stay restricted.").small().color(muted(ui)));
            }
        });
        let search = self.recruiting_state.search.trim().to_lowercase();
        let mut rows: Vec<_> = self
            .data
            .recruiting
            .interviews
            .iter()
            .filter(|i| {
                recruiting::interview(&self.data, i.id).is_ok()
                    && applicant.is_none_or(|a| a == i.applicant_id)
                    && (self.recruiting_state.show_closed || i.status == InterviewStatus::Scheduled)
                    && (search.is_empty()
                        || format!("{} {} {}", i.title, i.applicant_name, i.position)
                            .to_lowercase()
                            .contains(&search))
            })
            .cloned()
            .collect();
        rows.sort_by_key(|i| (i.starts_at, i.id));
        if rows.is_empty() {
            empty(ui, "No interviews match this view.");
        }
        for r in rows {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .link(RichText::new(&r.title).strong().size(18.0))
                        .clicked()
                    {
                        self.recruiting_state.detail = Some(r.id);
                    }
                    badge(ui, r.status.label(), r.status == InterviewStatus::Scheduled);
                });
                ui.label(format!("{} · {}", r.applicant_name, r.position));
                interview_time(ui, &r);
                ui.label(format!(
                    "Interviewer: {}",
                    self.interviewer_name(r.interviewer_employee_id)
                ));
                if r.status != InterviewStatus::Cancelled
                    && r.starts_at <= chrono::Utc::now()
                    && !self
                        .data
                        .recruiting
                        .feedback
                        .iter()
                        .any(|f| f.interview_id == r.id && f.author_id == self.data.access.user_id)
                    && self.data.access.employee_id == Some(r.interviewer_employee_id)
                {
                    ui.label(RichText::new("Your feedback is pending").color(accent(ui)));
                }
            });
        }
    }
    fn interviewer_name(&self, id: Uuid) -> String {
        self.data
            .recruiting
            .interviewers
            .iter()
            .find(|s| s.employee_id == id)
            .map(|s| s.full_name.clone())
            .or_else(|| {
                self.data
                    .employees
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.full_name.clone())
            })
            .unwrap_or_else(|| "Assigned staff".into())
    }
    fn send_interview(&mut self, action: InterviewAction) {
        self.recruiting_state.pending = Some(action.clone());
        self.send(Command::Change(Action::Interview(action)));
    }
    fn interview_detail(&mut self, ui: &mut Ui, r: &recruiting::Interview) {
        let hr = self.data.access.role.manages_hr();
        card(ui, |ui| {
            if ui
                .add_enabled(
                    self.recruiting_state.feedback_form.is_none()
                        && self.recruiting_state.status.is_none(),
                    egui::Button::new("< Interview agenda"),
                )
                .clicked()
            {
                self.recruiting_state.detail = None;
                self.recruiting_state.feedback_form = None;
                self.recruiting_state.status = None;
                self.recruiting_state.reason.clear();
            }
            ui.label(RichText::new(&r.title).strong().size(21.0));
            badge(ui, r.status.label(), r.status == InterviewStatus::Scheduled);
            ui.label(format!("{} · {}", r.applicant_name, r.position));
            interview_time(ui, r);
            ui.label(format!(
                "Interviewer: {}",
                self.interviewer_name(r.interviewer_employee_id)
            ));
            if !r.location.is_empty() {
                ui.label(format!("Location: {}", r.location));
            }
            if !r.brief.is_empty() {
                ui.label(&r.brief);
            }
            if hr && r.status == InterviewStatus::Scheduled {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Reschedule / change interviewer").clicked() {
                        self.recruiting_state.interview_form =
                            Some(InterviewDraft::existing(&self.data, r));
                    }
                    if ui
                        .add_enabled(
                            r.starts_at <= chrono::Utc::now(),
                            egui::Button::new("Mark completed"),
                        )
                        .clicked()
                    {
                        self.recruiting_state.status = Some(InterviewStatus::Completed);
                    }
                    if ui.button("Cancel interview").clicked() {
                        self.recruiting_state.status = Some(InterviewStatus::Cancelled);
                    }
                });
            }
            if let Some(status) = self.recruiting_state.status {
                field(
                    ui,
                    "Change reason *",
                    &mut self.recruiting_state.reason,
                    "Why this interview is being completed or cancelled",
                );
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            hiring::reason(&self.recruiting_state.reason).is_ok(),
                            egui::Button::new(format!("Confirm {}", status.label().to_lowercase())),
                        )
                        .clicked()
                    {
                        self.send_interview(InterviewAction::Status {
                            id: r.id,
                            expected_revision: r.revision,
                            status,
                            reason: self.recruiting_state.reason.clone(),
                        });
                    }
                    if ui.button("Keep current status").clicked() {
                        self.recruiting_state.status = None;
                        self.recruiting_state.reason.clear();
                    }
                });
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Interview feedback").strong().size(18.0));
            if self.data.access.employee_id == Some(r.interviewer_employee_id)
                && r.status != InterviewStatus::Cancelled
                && r.starts_at <= chrono::Utc::now()
                && self.recruiting_state.feedback_form.is_none()
            {
                if ui
                    .button(
                        if self.data.recruiting.feedback.iter().any(|f| {
                            f.interview_id == r.id && f.author_id == self.data.access.user_id
                        }) {
                            "Edit your feedback"
                        } else {
                            "Write your feedback"
                        },
                    )
                    .clicked()
                {
                    self.recruiting_state.feedback_form =
                        Some(FeedbackDraft::new(&self.data, r.id));
                }
            }
            if let Some(mut d) = self.recruiting_state.feedback_form.take() {
                let mut close = false;
                let mut save = false;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Rating (1–5)");
                    ui.add(egui::DragValue::new(&mut d.rating).range(1..=5));
                    egui::ComboBox::from_id_salt("interview_recommendation")
                        .selected_text(d.recommendation.label())
                        .show_ui(ui, |ui| {
                            for v in Recommendation::ALL {
                                ui.selectable_value(&mut d.recommendation, v, v.label());
                            }
                        });
                });
                ui.label("Written assessment *");
                ui.add(
                    egui::TextEdit::multiline(&mut d.assessment)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
                field(
                    ui,
                    "Save reason *",
                    &mut d.reason,
                    "Reason for this assessment or correction",
                );
                let action = InterviewAction::Feedback(d.clone());
                let valid = action.payload(&self.data, chrono::Utc::now());
                if let Err(e) = &valid {
                    ui.label(RichText::new(e.to_string()).small().color(muted(ui)));
                }
                ui.horizontal_wrapped(|ui| {
                    save = ui
                        .add_enabled(valid.is_ok(), egui::Button::new("Save feedback"))
                        .clicked();
                    close = ui.button("Discard feedback draft").clicked();
                });
                if !close {
                    self.recruiting_state.feedback_form = Some(d);
                }
                if save {
                    self.send_interview(action);
                }
            }
            let rows: Vec<_> = self
                .data
                .recruiting
                .feedback
                .iter()
                .filter(|f| {
                    f.interview_id == r.id && (hr || f.author_id == self.data.access.user_id)
                })
                .collect();
            if rows.is_empty() {
                ui.label(RichText::new("No feedback submitted in your view.").color(muted(ui)));
            }
            for f in rows {
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "{} · {}/5 · {}",
                        f.author_name,
                        f.rating,
                        f.recommendation.label()
                    ))
                    .strong(),
                );
                ui.label(&f.assessment);
                ui.label(
                    RichText::new(format!(
                        "Saved {} · Version {}",
                        f.updated_at
                            .with_timezone(&self.data.company.tz())
                            .format("%b %d, %Y %H:%M"),
                        f.revision
                    ))
                    .small()
                    .color(muted(ui)),
                );
            }
        });
        ui.add_space(10.0);
        egui::CollapsingHeader::new("Interview history")
            .id_salt(("interview_history", r.id))
            .show(ui, |ui| {
                let mut events: Vec<_> = self
                    .data
                    .recruiting
                    .events
                    .iter()
                    .filter(|e| {
                        e.interview_id == r.id
                            && (hr
                                || e.kind != "feedback_saved"
                                || e.actor_id == self.data.access.user_id)
                    })
                    .collect();
                events.sort_by_key(|e| std::cmp::Reverse(e.recorded_at));
                for e in events {
                    card(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                e.kind.replace('_', " "),
                                e.actor_name
                            ))
                            .strong(),
                        );
                        ui.label(format!(
                            "{} · {}",
                            e.recorded_at
                                .with_timezone(&self.data.company.tz())
                                .format("%b %d, %Y %H:%M"),
                            e.actor_role.label()
                        ));
                        ui.label(&e.reason);
                        for key in if e.kind == "feedback_saved" {
                            vec!["rating", "recommendation", "assessment"]
                        } else {
                            vec!["starts_at", "ends_at", "location", "brief", "status"]
                        } {
                            let old = e.before_record.as_ref().and_then(|v| v.get(key));
                            let new = e.after_record.get(key);
                            if new != old
                                && let Some(value) = new
                            {
                                ui.label(format!(
                                    "{}: {}",
                                    key.replace('_', " "),
                                    value
                                        .as_str()
                                        .map(str::to_owned)
                                        .unwrap_or_else(|| value.to_string())
                                ));
                                if let Some(value) = old {
                                    ui.label(
                                        RichText::new(format!(
                                            "Previous: {}",
                                            value
                                                .as_str()
                                                .map(str::to_owned)
                                                .unwrap_or_else(|| value.to_string())
                                        ))
                                        .small()
                                        .color(muted(ui)),
                                    );
                                }
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            });
    }
}
fn interview_time(ui: &mut Ui, r: &recruiting::Interview) {
    let tz = r
        .timezone
        .parse::<chrono_tz::Tz>()
        .unwrap_or(chrono_tz::UTC);
    ui.label(format!(
        "{}–{} · {}",
        r.starts_at.with_timezone(&tz).format("%b %d, %Y %H:%M"),
        r.ends_at.with_timezone(&tz).format("%H:%M"),
        r.timezone
    ));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unmatched_schedule_or_feedback_response_preserves_the_open_editor() {
        let mut data = DemoStore::new().data;
        let id = Uuid::new_v4();
        let draft = InterviewDraft::new(&data, id);
        let mut state = State {
            interview_form: Some(draft.clone()),
            pending: Some(InterviewAction::Save(draft)),
            ..Default::default()
        };
        assert!(!state.accept(&data));
        assert!(state.interview_form.is_some());
        let f = FeedbackDraft {
            interview_id: id,
            expected_revision: 0,
            rating: 4,
            recommendation: Recommendation::Proceed,
            assessment: "Fictional notes".into(),
            reason: "Assessed".into(),
        };
        state.feedback_form = Some(f.clone());
        state.pending = Some(InterviewAction::Feedback(f));
        assert!(!state.accept(&data));
        assert!(state.feedback_form.is_some());
        state.resume_form = Some(ResumeDraft::new(id));
        state.resume_pending = Some(ResumeAction::Upload(
            state.resume_form.as_ref().unwrap().clone(),
        ));
        assert!(!state.accept(&data));
        assert!(state.resume_form.is_some());
        data.mode = Mode::SignedOut;
    }
}
