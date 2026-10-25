use super::*;
use crate::leave::opening_import::{BatchPreview, Import, Post};
use crate::leave::{
    self,
    dated::{Change, OpeningRecord, Preview},
};
use anyhow::Context;

#[derive(Default, PartialEq, Eq)]
enum View {
    #[default]
    Entry,
    Import,
    History,
}

#[derive(Default)]
pub(super) struct State {
    view: View,
    scope: Option<crate::access::Access>,
    person: Option<Uuid>,
    code: String,
    target: String,
    carry: String,
    reason: String,
    reference: String,
    reviewed: bool,
    record: Option<OpeningRecord>,
    preview: Option<Preview>,
    pending: Option<Change>,
    error: Option<String>,
    posted: bool,
    page: usize,
    import: Option<Import>,
    import_preview: Option<BatchPreview>,
    import_reviewed: bool,
}
impl State {
    pub(super) fn has_draft(&self) -> bool {
        self.record.is_some()
            || self.pending.is_some()
            || self.import.is_some()
            || !self.target.trim().is_empty()
            || !self.carry.trim().is_empty()
            || !self.reason.trim().is_empty()
            || !self.reference.trim().is_empty()
    }
    pub(super) fn accept(&mut self, p: Preview) {
        if self.record.as_ref() == Some(&p.record) {
            self.preview = Some(p);
            self.error = None;
        }
    }
    pub(super) fn accept_import(&mut self, p: BatchPreview, data: &Snapshot) {
        if let Some(source) = &mut self.import {
            let records = source
                .rows
                .iter()
                .filter_map(|r| r.record.clone())
                .collect::<Vec<_>>();
            if records == p.records() {
                for row in &mut source.rows {
                    if let Some(r) = &row.record {
                        if !data.employees.iter().any(|e| {
                            e.id == r.employee_id
                                && e.employee_code.eq_ignore_ascii_case(&row.employee_code)
                        }) {
                            row.errors
                                .push("Employee code changed; reload and review the file.".into());
                        }
                    }
                }
                self.import_preview = Some(p);
                self.error = None;
            }
        }
    }
    fn reconcile(&mut self, data: &Snapshot) {
        if self.scope.as_ref() != Some(&data.access) {
            *self = Self {
                scope: Some(data.access.clone()),
                ..Default::default()
            };
        }
        if self.pending.as_ref().is_some_and(|p| {
            data.leave_setup
                .dated
                .as_ref()
                .is_some_and(|c| p.matches(c))
        }) {
            self.pending = None;
            self.preview = None;
            self.record = None;
            self.target.clear();
            self.carry.clear();
            self.reason.clear();
            self.reference.clear();
            self.reviewed = false;
            self.posted = true;
            self.error = None;
            self.import = None;
            self.import_preview = None;
            self.import_reviewed = false;
        }
    }
}
fn amount(text: &str) -> anyhow::Result<i32> {
    leave::opening_import::quantity(text)
}
fn balance_grid(ui: &mut Ui, id: &str, b: &leave::Balance) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([24.0, 6.0])
        .show(ui, |ui| {
            for (label, value) in [
                ("Base (including credits)", b.earned_units),
                ("Carry before expiry", b.carryover_units),
                ("Approved use", b.used_units),
                ("Pending reservations", b.reserved_units),
                ("Available", b.available_units),
            ] {
                ui.label(label);
                ui.label(RichText::new(format!("{} day(s)", leave::days(value))).strong());
                ui.end_row();
            }
        });
}
impl HrisApp {
    pub(super) fn leave_openings(&mut self, ui: &mut Ui) {
        let mut state = std::mem::take(&mut self.leave_state.openings);
        state.reconcile(&self.data);
        let Some(context) = self.data.leave_setup.dated.as_ref() else {
            empty(ui, "Refresh to load opening balances.");
            self.leave_state.openings = state;
            return;
        };
        let context = context.clone();
        let hr = self.data.access.role.manages_hr();
        let admin = self.data.access.role.manages_accounts();
        if state.posted {
            ui.colored_label(accent(ui),"Opening posted. Its original basis is retained; later corrections use Credit adjustments.");
        }
        card(ui, |ui| {
            ui.heading("Opening balances");
            ui.label("Take over a reviewed available balance today without adding a second annual grant. The target already includes the HRIS requests shown in the preview. Imported carry is part of that target and keeps its expiry.");
            ui.label(RichText::new("One immutable opening per employee, leave type and year. Earlier balances are not reconstructed. Later changes use audited credit adjustments.").small().color(muted(ui)));
        });
        if hr {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut state.view, View::Entry, "Enter opening");
                ui.selectable_value(&mut state.view, View::Import, "Import CSV");
                ui.selectable_value(&mut state.view, View::History, "History");
            });
        }
        if hr && state.view == View::Entry {
            ui.add_space(10.0);
            let pending = state.pending.is_some();
            let mut preview_command = None;
            let mut post_command = None;
            card(ui, |ui| {
                ui.label(RichText::new("Review an opening").size(18.0).strong());
                ui.add_enabled_ui(!self.busy&&!pending,|ui|{
                    ui.label("Employee");employee_picker(ui,"opening_employee",&self.data,&mut state.person);
                    let types=context.timeline.policy_on(NaiveDate::from_ymd_opt(context.as_of.year(),1,1).unwrap()).types.iter().filter(|t|t.accrual!=leave::Accrual::Unlimited).collect::<Vec<_>>();
                    if !types.iter().any(|t|t.code==state.code) {state.code=types.first().map_or(String::new(),|t|t.code.clone());}
                    ui.label("Leave type");egui::ComboBox::from_id_salt("opening_type").width(240.0).truncate().selected_text(types.iter().find(|t|t.code==state.code).map_or("Choose finite type",|t|t.name.as_str())).show_ui(ui,|ui|{for t in types{ui.selectable_value(&mut state.code,t.code.clone(),&t.name);}});
                    ui.label(format!("Takeover date: {} · company time zone",context.as_of));
                    field(ui,"Target available after HRIS reservations (days)",&mut state.target,"e.g. 12.5");
                    field(ui,"Of that target, remaining expiring carry (days)",&mut state.carry,"0 if none");
                    field(ui,"Private HR reason / source review",&mut state.reason,"Explain the reviewed takeover");
                    field(ui,"Private source reference (optional)",&mut state.reference,"No attachment or password required");
                    ui.label(RichText::new("Record still-active requests from the source system here before takeover, so future approvals/cancellations remain traceable. Later-year reservations must be resolved before posting an opening.").small().color(muted(ui)));
                    ui.checkbox(&mut state.reviewed,"I reviewed source-system requests and their representation in HRIS reservations");
                    let candidate=(||->anyhow::Result<OpeningRecord>{Ok(OpeningRecord{id:state.record.as_ref().map_or_else(Uuid::new_v4,|r|r.id),employee_id:state.person.context("Choose an employee.")?,type_code:state.code.clone(),effective_on:context.as_of,
                        target_available_units:amount(&state.target)?,remaining_carry_units:amount(if state.carry.trim().is_empty(){"0"}else{&state.carry})?,source_requests_reviewed:state.reviewed,
                        expected_revision:state.record.as_ref().map_or(context.revision,|r|r.expected_revision),reason:state.reason.trim().into(),reference:state.reference.clone()})})();
                    if candidate.as_ref().ok().is_some_and(|r|state.record.as_ref().is_some_and(|old|old!=r)){state.preview=None;state.record=None;}
                    if ui.button("Preview opening — makes no changes").clicked(){match candidate.and_then(|r|{r.validate(&self.data)?;Ok(r)}){Ok(r)=>{state.record=Some(r.clone());state.preview=None;state.error=None;preview_command=Some(r);},Err(e)=>state.error=Some(e.to_string())}}
                });
                if let Some(p) = &state.preview {
                    ui.separator();
                    ui.label(RichText::new("Protected preview").strong());
                    ui.label(format!(
                        "{} · {} · carry expires {}",
                        self.data.employee_name(p.record.employee_id),
                        p.type_snapshot.name,
                        p.opening.carry_expires_on
                    ));
                    ui.push_id("opening_comparison", |ui| {
                        if ui.available_width() > 630.0 {
                            ui.columns(2, |cols| {
                                cols[0].label(RichText::new("Before takeover").strong());
                                balance_grid(&mut cols[0], "opening_before", &p.before);
                                cols[1].label(RichText::new("After takeover").strong());
                                balance_grid(&mut cols[1], "opening_after", &p.after);
                            });
                        } else {
                            ui.label(RichText::new("Before takeover").strong());
                            balance_grid(ui, "opening_before", &p.before);
                            ui.add_space(8.0);
                            ui.label(RichText::new("After takeover").strong());
                            balance_grid(ui, "opening_after", &p.after);
                        }
                    });
                    ui.label(format!("{} saved active request(s) retained. Frozen annual rate: {} day(s). Imported carry remaining: {} day(s).",p.opening.baseline.len(),leave::days(p.basis.rate_units),leave::days(p.opening.remaining_carry_units)));
                    if admin
                        && ui
                            .add_enabled(
                                !self.busy,
                                egui::Button::new(if pending {
                                    "Retry same opening"
                                } else {
                                    "Post reviewed opening"
                                }),
                            )
                            .clicked()
                    {
                        post_command = Some(Change::Opening(Box::new(p.clone())));
                    }
                    if !admin {
                        ui.label("An Administrator posts the reviewed opening.");
                    }
                }
                if pending {
                    ui.label("Posting outcome is pending verification. Keep this draft; Refresh or retry the same opening.");
                    if ui
                        .add_enabled(
                            !self.busy,
                            egui::Button::new("Refresh and revise this draft"),
                        )
                        .clicked()
                    {
                        state.pending = None;
                        state.preview = None;
                        state.record = None;
                        self.send(Command::Refresh);
                    }
                }
                if let Some(e) = &state.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            });
            if let Some(record) = preview_command {
                self.send(Command::PreviewLeaveOpening(record));
            }
            if let Some(change) = post_command {
                state.pending = Some(change.clone());
                self.send(Command::Change(Action::DatedLeave(change)));
            }
        }
        if hr && state.view == View::Import {
            ui.add_space(10.0);
            self.opening_import_card(ui, &mut state);
        }
        if hr && state.view != View::History {
            self.leave_state.openings = state;
            return;
        }
        ui.add_space(10.0);
        ui.heading(if hr {
            "Opening history"
        } else {
            "Own / current team openings"
        });
        if context.openings.is_empty() {
            card(ui, |ui| {
                empty(
                    ui,
                    "No opening balances have been posted in your current scope.",
                )
            });
        }
        let mut rows = context.openings.iter().collect::<Vec<_>>();
        rows.sort_by_key(|o| std::cmp::Reverse((o.effective_on, o.id)));
        let pages = rows.len().div_ceil(12).max(1);
        state.page = state.page.min(pages - 1);
        for o in rows.into_iter().skip(state.page * 12).take(12) {
            ui.add_space(7.0);
            ui.push_id(o.id, |ui| {
                card(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            self.data.employee_name(o.employee_id),
                            o.type_code
                        ))
                        .strong(),
                    );
                    ui.label(format!(
                        "{} · target {} day(s) · included unspent carry {} · expiry {}",
                        o.effective_on,
                        leave::days(o.target_available_units),
                        leave::days(o.remaining_carry_units),
                        o.carry_expires_on
                    ));
                    if let Some(a) = context.audit.iter().find(|a| a.id == o.id) {
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
                })
            });
        }
        if pages > 1 {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(state.page > 0, egui::Button::new("Previous"))
                    .clicked()
                {
                    state.page -= 1;
                }
                ui.label(format!("Page {} of {pages}", state.page + 1));
                if ui
                    .add_enabled(state.page + 1 < pages, egui::Button::new("Next"))
                    .clicked()
                {
                    state.page += 1;
                }
            });
        }
        self.leave_state.openings = state;
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_dated_leave_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if self.data.mode != Mode::Demo
            || !(form.starts_with("leave-opening") || form.starts_with("leave-policy"))
        {
            return;
        }
        self.leave_state = super::leave_view::State::default();
        self.data.access.role = Role::Administrator;
        self.data.leave.clear();
        let today = self.data.company.today();
        for e in &mut self.data.employees {
            e.joined_on = Some(NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap());
            e.annual_leave_days = 15;
        }
        self.data.leave_setup.policy.tracking_starts_year = today.year() - 1;
        self.data.leave_setup.policy.types[0].carryover_units = 12;
        self.data.leave_setup.policy.types[0].carryover_expiry_month = 12;
        self.data.leave_setup.credits = Some(leave::credits::Context::empty(&self.data));
        self.data.leave_setup.dated = Some(leave::dated::Context::empty(&self.data));
        let first = self.data.employees[0].id;
        let second = self.data.employees[1].id;
        let record=OpeningRecord{id:Uuid::from_u128(490900),employee_id:first,type_code:"annual".into(),effective_on:today,target_available_units:25,remaining_carry_units:5,source_requests_reviewed:true,expected_revision:0,reason:"Fictional takeover reviewed against the source system and current HRIS reservations.".into(),reference:"Fictional source register only".into()};
        let p = leave::dated::preview_demo(&self.data, &record).unwrap();
        leave::dated::apply_demo(&mut self.data, &Change::Opening(Box::new(p))).unwrap();
        let mut state = State {
            scope: Some(self.data.access.clone()),
            person: Some(second),
            code: "annual".into(),
            ..Default::default()
        };
        if form.contains("draft") {
            let r = OpeningRecord {
                id: Uuid::from_u128(490901),
                employee_id: second,
                target_available_units: 18,
                remaining_carry_units: 2,
                expected_revision: 1,
                ..record.clone()
            };
            state.target = "9".into();
            state.carry = "1".into();
            state.reason = r.reason.clone();
            state.reference = r.reference.clone();
            state.reviewed = true;
            state.record = Some(r.clone());
            state.preview = Some(leave::dated::preview_demo(&self.data, &r).unwrap());
        } else if form.contains("import") {
            state.view = View::Import;
            let headers = leave::opening_import::HEADERS.join(",");
            let text = format!(
                "{headers}\n{},annual,9,1,Reviewed fictional opening,Register row 2\nUNKNOWN,annual,2.25,0,Needs source review,Register row 3\n",
                self.data.employees[1].employee_code
            );
            state.import =
                Some(Import::parse(&self.data, "fictional-opening-balances.csv", &text).unwrap());
        } else {
            state.view = View::History;
        }
        self.leave_state.openings = state;
        self.leave_state.view = super::leave_view::View::Openings;
        self.page = Page::Leave;
        if form.starts_with("leave-policy") {
            let mut policy = self.data.leave_setup.policy.clone();
            policy.types[0].use_employee_allowance = false;
            policy.types[0].annual_units = 32;
            let record=leave::dated::PolicyRecord{id:Uuid::from_u128(490902),effective_on:NaiveDate::from_ymd_opt(today.year()+1,1,1).unwrap(),expected_revision:1,policy:policy.clone(),reason:"Fictional next-year policy with retained prior entitlements and request snapshots.".into(),reference:"Fictional policy review".into()};
            leave::dated::apply_demo(&mut self.data, &Change::Schedule(record)).unwrap();
            self.leave_state.view = super::leave_view::View::Policies;
            if form.contains("edit") {
                self.leave_state.draft = Some(policy);
                self.leave_state.policy_date = NaiveDate::from_ymd_opt(today.year() + 2, 1, 1)
                    .unwrap()
                    .to_string();
                self.leave_state.policy_reason = "Fictional upcoming policy review".into();
            }
        }
    }
    fn opening_import_card(&mut self, ui: &mut Ui, state: &mut State) {
        let mut preview = None;
        let mut post = None;
        let pending = state.pending.is_some();
        card(ui, |ui| {
            ui.label(
                RichText::new("Import opening balances from CSV")
                    .size(18.0)
                    .strong(),
            );
            ui.label("Use existing employee codes and leave type codes. All rows take over today's reviewed balance. Up to 100 rows are previewed together; posting applies all rows or none.");
            if let Some(c) = &self.data.leave_setup.dated {
                ui.label(format!(
                    "Finite type codes: {}",
                    c.timeline
                        .policy_on(NaiveDate::from_ymd_opt(c.as_of.year(), 1, 1).unwrap())
                        .types
                        .iter()
                        .filter(|t| t.accrual != leave::Accrual::Unlimited)
                        .map(|t| format!("{} ({})", t.code, t.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            ui.add_enabled_ui(!self.busy && !pending, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Save opening CSV template").clicked() {
                        match crate::reports::choose_export("hris-opening-balances.csv").and_then(
                            |p| {
                                p.map(|p| {
                                    leave::opening_import::template()
                                        .and_then(|b| crate::reports::save_csv(&p, &b))
                                })
                                .transpose()
                            },
                        ) {
                            Ok(Some(())) => {
                                self.notice = Some(("Opening template saved.".into(), false))
                            }
                            Err(e) => state.error = Some(e.to_string()),
                            _ => {}
                        }
                    }
                    if ui.button("Choose opening CSV").clicked() {
                        match crate::imports::choose_csv_source("Preview opening balances CSV")
                            .and_then(|p| p.map(|p| Import::load(&self.data, &p)).transpose())
                        {
                            Ok(Some(source)) => {
                                state.import = Some(source);
                                state.import_preview = None;
                                state.import_reviewed = false;
                                state.error = None;
                            }
                            Err(e) => state.error = Some(e.to_string()),
                            _ => {}
                        }
                    }
                });
            });
            if let Some(source) = &state.import {
                ui.label(format!(
                    "{} · {} row(s) · takeover {}",
                    source.filename,
                    source.rows.len(),
                    self.data.company.today()
                ));
                egui::ScrollArea::both()
                    .id_salt("opening_import_rows")
                    .max_height(320.0)
                    .show(ui, |ui| {
                        egui::Grid::new("opening_import_grid")
                            .num_columns(5)
                            .striped(true)
                            .spacing([16.0, 9.0])
                            .show(ui, |ui| {
                                for name in [
                                    "Source line",
                                    "Employee / type",
                                    "Available days",
                                    "Carry days",
                                    "Review",
                                ] {
                                    ui.strong(name);
                                }
                                ui.end_row();
                                for (n, row) in source.rows.iter().enumerate() {
                                    ui.label(row.line.to_string());
                                    ui.label(format!(
                                        "{} / {}",
                                        row.employee_code,
                                        row.cells.get(1).map_or("", |s| s.as_str())
                                    ));
                                    ui.label(row.cells.get(2).map_or("", |s| s.as_str()));
                                    ui.label(row.cells.get(3).map_or("0", |s| s.as_str()));
                                    ui.vertical(|ui| {
                                        if row.errors.is_empty() {
                                            if let Some(server) = state
                                                .import_preview
                                                .as_ref()
                                                .and_then(|p| p.rows.get(n))
                                            {
                                                if let Some(error) = &server.error {
                                                    ui.colored_label(
                                                        ui.visuals().error_fg_color,
                                                        error,
                                                    );
                                                } else if let Some(p) = &server.preview {
                                                    ui.label(format!(
                                                        "{} -> {} day(s); {} request(s) retained",
                                                        leave::days(p.before.available_units),
                                                        leave::days(p.after.available_units),
                                                        p.opening.baseline.len()
                                                    ));
                                                }
                                            } else {
                                                ui.label("Ready for protected preview");
                                            }
                                        }
                                        for e in &row.errors {
                                            ui.colored_label(ui.visuals().error_fg_color, e);
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
                ui.add_enabled_ui(!self.busy&&!pending,|ui|{
                    ui.checkbox(&mut state.import_reviewed,"I reviewed source requests and their representation in HRIS for every imported row");
                    if ui.button("Preview every CSV row — no changes").clicked(){match source.records(&self.data,state.import_reviewed){Ok(records)=>{preview=Some(records);state.import_preview=None;state.error=None;},Err(e)=>state.error=Some(e.to_string())}}
                    if ui.button("Export row review / errors CSV").clicked(){
                        let mut report=source.clone();if let Some(p)=&state.import_preview{for(row,server)in report.rows.iter_mut().zip(&p.rows){if let Some(e)=&server.error{row.errors.push(e.clone());}}}
                        match crate::reports::choose_export("hris-opening-row-review.csv"){Ok(Some(destination))=>self.send(Command::OpeningImportErrors{report,destination}),Err(e)=>state.error=Some(e.to_string()),_=>{}}
                    }
                });
                if let Some(p) = &state.import_preview {
                    if self.data.access.role.manages_accounts()
                        && ui
                            .add_enabled(
                                !self.busy
                                    && state.import_reviewed
                                    && p.ready()
                                    && source.rows.iter().all(|r| r.errors.is_empty()),
                                egui::Button::new(if pending {
                                    "Retry same opening import"
                                } else {
                                    "Post all reviewed opening rows"
                                }),
                            )
                            .clicked()
                    {
                        post = Some(Change::OpeningBatch(Box::new(Post {
                            id: source.id,
                            preview: p.clone(),
                        })));
                    }
                    if !p.ready() {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            "Import blocked: resolve every row error. No rows have been applied.",
                        );
                    }
                }
                if pending
                    && ui
                        .add_enabled(!self.busy, egui::Button::new("Refresh and revise import"))
                        .clicked()
                {
                    state.pending = None;
                    state.import_preview = None;
                    self.send(Command::Refresh);
                }
            }
            if let Some(e) = &state.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
        });
        if let Some(records) = preview {
            self.send(Command::PreviewLeaveOpeningImport(records));
        }
        if let Some(change) = post {
            match change.payload(&self.data) {
                Ok(_) => {
                    state.pending = Some(change.clone());
                    self.send(Command::Change(Action::DatedLeave(change)));
                }
                Err(e) => state.error = Some(e.to_string()),
            }
        }
    }
}
