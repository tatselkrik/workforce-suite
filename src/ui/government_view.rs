use super::*;
use crate::payroll_government as g;
use serde_json::json;
#[derive(Default)]
pub(super) struct State {
    pub(super) registration: super::government_registration_view::State,
    pub(super) registration_open: bool,
    pub(super) collection: Option<super::government_collection_view::State>,
    pub(super) collection_export_pending: bool,
    pub(super) bir_open: bool,
    pub(super) bir: super::bir_file_view::State,
    pub(super) builder_open: bool,
    pub(super) builder: super::bir_builder_view::State,
    pub(super) annual_open: bool,
    pub(super) annual: super::government_annual_view::State,
    pub(super) declarations_open: bool,
    pub(super) declarations: super::government_declarations_view::State,
    pub(super) identities_open: bool,
    pub(super) identities: super::government_identity_view::State,
    scope: Option<(Uuid, Uuid, Role)>,
    month: String,
    agency: Option<g::Agency>,
    report: Option<g::Reconciliation>,
    form: Option<Form>,
    pub(super) pending: Option<g::Action>,
    history: Option<Uuid>,
    search: String,
    page: usize,
    receipt_page: usize,
    export_excel: bool,
}
enum Form {
    Allocation(AllocationForm),
    Decision(DecisionForm),
    Remittance(RemittanceForm),
    Void(VoidForm),
}
struct AllocationForm {
    id: Uuid,
    revision: Option<i64>,
    basis: g::Basis,
    previous: Option<Uuid>,
    rows: Vec<AllocationRow>,
    reference: String,
    note: String,
}
#[derive(Clone)]
struct AllocationRow {
    month: String,
    employee: String,
    employer: String,
    ec: String,
}
struct DecisionForm {
    record: g::Review,
    operation: String,
    reason: String,
    reviewed: bool,
}
struct RemittanceForm {
    id: Uuid,
    agency: g::Agency,
    date: String,
    rows: Vec<ReceiptRow>,
    amount: String,
    reference: String,
    evidence: String,
    reason: String,
    replaces: Option<Uuid>,
    confirmed: bool,
}
struct ReceiptRow {
    month: String,
    amount: String,
}
struct VoidForm {
    receipt: g::Receipt,
    id: Uuid,
    reason: String,
    confirmed: bool,
}
enum Intent {
    Read,
    Save(g::Action),
    Export,
    ExportPreparation,
    PrepareSss,
    CheckBir,
    BuildBir,
    Annual,
    Identities,
    Declarations,
}
impl State {
    pub(super) fn bind(&mut self, data: &Snapshot) {
        let scope = Some((data.company.id, data.access.user_id, data.access.role));
        if self.scope != scope
            || !data
                .access
                .allows(crate::staff_roles::Capability::GovernmentPreparation)
        {
            *self = Self {
                scope,
                month: crate::payroll::month_start(data.company.today()).to_string(),
                agency: Some(g::Agency::Sss),
                ..Default::default()
            };
        }
    }
    pub(super) fn editing(&self) -> bool {
        self.form.is_some() || self.registration.editing() || self.collection.is_some()
    }
    pub(super) fn scroll_key(&self) -> (u8, Option<Uuid>) {
        if self.builder_open {
            return (25, None);
        }
        if self.declarations_open {
            return (24, None);
        }
        if self.identities_open {
            return (23, None);
        }
        if self.annual_open {
            return (22, None);
        }
        if self.bir_open {
            return (21, None);
        }
        if let Some(s) = &self.collection {
            return (20, Some(s.basis.receipt.id));
        }
        if self.registration_open {
            return self.registration.scroll_key();
        }
        match &self.form {
            None => (0, None),
            Some(Form::Allocation(f)) => (1, Some(f.id)),
            Some(Form::Decision(f)) => (2, Some(f.record.id)),
            Some(Form::Remittance(f)) => (3, Some(f.id)),
            Some(Form::Void(f)) => (4, Some(f.id)),
        }
    }
    pub(super) fn accept_registration(&mut self, r: g::registration::Snapshot, data: &Snapshot) {
        self.bind(data);
        self.registration.accept(r, data);
    }
    pub(super) fn accept_bir(&mut self, r: g::bir_file::Report, data: &Snapshot) {
        self.bind(data);
        if self.bir_open {
            self.bir.accept(r, data);
        }
    }
    pub(super) fn accept_bir_build(&mut self, r: g::bir_file::builder::Preview, data: &Snapshot) {
        self.bind(data);
        if self.builder_open {
            self.builder.accept(r, data);
        }
    }
    pub(super) fn accept_annual(&mut self, r: g::annual::Ledger, data: &Snapshot) {
        self.bind(data);
        if self.annual_open {
            self.annual.accept(r, data);
        }
    }
    pub(super) fn accept_annual_pay(&mut self, r: g::annual_pay::Report, data: &Snapshot) {
        self.bind(data);
        if self.annual_open {
            self.annual.accept_pay(r, data);
        }
    }
    pub(super) fn accept_declarations(&mut self, r: g::bir_declarations::Report, data: &Snapshot) {
        self.bind(data);
        if self.declarations_open {
            self.declarations.accept(r, data);
        }
    }
    pub(super) fn accept_identities(&mut self, r: g::bir_identity::Report, data: &Snapshot) {
        self.bind(data);
        if self.identities_open {
            self.identities.accept(r, data);
        }
    }
    pub(super) fn accept_collection(&mut self, b: g::sss_collection::Basis, data: &Snapshot) {
        self.bind(data);
        if data
            .access
            .allows(crate::staff_roles::Capability::GovernmentPreparation)
            && b.company_id == data.company.id
            && self
                .filter()
                .is_ok_and(|f| f.agency == g::Agency::Sss && f.month == b.month)
        {
            if let Some(form) = &mut self.collection {
                form.fresh = Some(b);
            } else {
                self.collection = Some(super::government_collection_view::State::new(b));
            }
        }
    }
    pub(super) fn accept(&mut self, r: g::Reconciliation, data: &Snapshot) {
        self.bind(data);
        if self.filter().is_ok_and(|f| r.filter() == f)
            && r.wire.source.company_id == data.company.id
            && data
                .access
                .allows(crate::staff_roles::Capability::GovernmentPreparation)
        {
            if self.collection_export_pending {
                self.collection = None;
                self.collection_export_pending = false;
            }
            if self.pending.take().is_some() {
                self.form = None;
            }
            self.report = Some(r);
        }
    }
    pub(super) fn filter(&self) -> anyhow::Result<g::Filter> {
        Ok(g::Filter {
            month: self
                .month
                .parse()
                .map_err(|_| anyhow::anyhow!("Choose a report month."))?,
            agency: self.agency.unwrap_or(g::Agency::Sss),
        })
    }
    fn current(&self) -> Option<&g::Reconciliation> {
        self.report
            .as_ref()
            .filter(|r| self.filter().is_ok_and(|f| r.filter() == f))
    }
}
fn entry(ui: &mut Ui, label: &str, value: impl Into<String>) {
    ui.horizontal_top(|ui| {
        let width = (ui.available_width() * 0.58).clamp(150.0, 350.0);
        ui.allocate_ui_with_layout(egui::vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            ui.add(egui::Label::new(label).wrap());
        });
        ui.add(egui::Label::new(value.into()).wrap());
    });
}
fn field(ui: &mut Ui, label: &str, value: &mut String) {
    ui.add(egui::Label::new(label).wrap());
    ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width()));
}
fn note(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_rows(3)
            .desired_width(ui.available_width()),
    );
}
fn month(ui: &mut Ui, id: &str, value: &mut String, lo: NaiveDate, hi: NaiveDate) {
    let text = value
        .parse::<NaiveDate>()
        .map(|d| d.format("%B %Y").to_string())
        .unwrap_or("Choose month".into());
    egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .show_ui(ui, |ui| {
            let mut d = crate::payroll::month_start(lo);
            for _ in 0..2112 {
                if d > hi {
                    break;
                }
                ui.selectable_value(value, d.to_string(), d.format("%B %Y").to_string());
                let Some(next) = d.checked_add_months(chrono::Months::new(1)) else {
                    break;
                };
                d = next;
            }
        });
}
fn label(report: &g::Reconciliation, eid: Uuid) -> String {
    report
        .employee(eid)
        .map(|e| format!("{} · {}", e.employee_code, e.employee_name))
        .unwrap_or("Employee".into())
}
fn part(n: Option<i64>) -> String {
    n.map(crate::payroll::money)
        .unwrap_or("Not reviewed".into())
}
fn allocation_form(
    f: &g::Fact,
    record: Option<&g::Review>,
    previous: Option<&g::Review>,
) -> AllocationForm {
    let rows = record
        .or(previous)
        .map(|r| {
            r.allocations
                .iter()
                .map(|a| AllocationRow {
                    month: a.month.to_string(),
                    employee: g::decimal(a.employee_cents),
                    employer: g::decimal(a.employer_cents),
                    ec: g::decimal(a.ec_cents),
                })
                .collect()
        })
        .unwrap_or_else(|| {
            vec![AllocationRow {
                month: f
                    .contribution_month
                    .map(|d| d.to_string())
                    .unwrap_or_default(),
                employee: f.parts.employee_cents.map(g::decimal).unwrap_or_default(),
                employer: String::new(),
                ec: if f.agency == g::Agency::Sss {
                    String::new()
                } else {
                    "0.00".into()
                },
            }]
        });
    AllocationForm {
        id: record.map(|r| r.id).unwrap_or_else(Uuid::new_v4),
        revision: record.map(|r| r.revision),
        basis: g::Basis::from(f),
        previous: record
            .and_then(|r| r.previous_id)
            .or(previous.map(|r| r.id)),
        rows,
        reference: record.map(|r| r.reference.clone()).unwrap_or_default(),
        note: record.map(|r| r.note.clone()).unwrap_or_default(),
    }
}
fn allocation_action(f: &AllocationForm, today: NaiveDate) -> anyhow::Result<g::Action> {
    let rows = f
        .rows
        .iter()
        .map(|a| {
            Ok(g::Allocation {
                month: a
                    .month
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Choose each covered month."))?,
                employee_cents: g::parse_cents(&a.employee)?,
                employer_cents: g::parse_cents(&a.employer)?,
                ec_cents: g::parse_cents(&a.ec)?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    g::validate_plan(&f.basis, &rows, today)?;
    anyhow::ensure!(
        !f.reference.trim().is_empty() && !f.note.trim().is_empty(),
        "Enter the supporting reference and review note."
    );
    let mut v = json!({"id":f.id,"expected_revision":f.revision,"allocations":rows,"reference":f.reference,"note":f.note,"reason":f.note});
    if f.revision.is_none() {
        v["source_kind"] = json!(f.basis.kind);
        v["source_id"] = json!(f.basis.id);
        v["agency"] = json!(f.basis.agency);
        v["previous_id"] = json!(f.previous);
        v["source_basis"] = json!(f.basis);
    }
    Ok(g::Action {
        operation: if f.revision.is_some() {
            "allocation_save"
        } else {
            "allocation_create"
        }
        .into(),
        record: v,
    })
}
fn active_source(report: &g::Reconciliation, basis: &g::Basis) -> bool {
    report.wire.source.facts.iter().any(|f| {
        g::Basis::from(f) == *basis
            && !matches!(f.state, g::State::Withdrawn | g::State::ReviewOnly)
    })
}
fn allocation_stale(report: Option<&g::Reconciliation>, form: &AllocationForm) -> bool {
    report.is_none_or(|r| {
        !active_source(r, &form.basis)
            || r.wire
                .reviews
                .iter()
                .find(|r| r.id == form.id)
                .map(|r| Some(r.revision) != form.revision || r.status != "draft")
                .unwrap_or(form.revision.is_some())
    })
}
fn decision_stale(report: Option<&g::Reconciliation>, form: &DecisionForm) -> bool {
    report.is_none_or(|r| {
        r.wire
            .reviews
            .iter()
            .find(|r| r.id == form.record.id)
            .is_none_or(|r| r.revision != form.record.revision || r.status != form.record.status)
            || (matches!(form.operation.as_str(), "submit" | "approve")
                && !active_source(r, &form.record.source_basis))
    })
}
impl HrisApp {
    pub(super) fn payroll_government(&mut self, ui: &mut Ui) {
        self.payroll_state.government.bind(&self.data);
        if self.payroll_state.government.builder_open {
            self.payroll_bir_builder(ui);
            return;
        }
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::GovernmentPreparation)
        {
            return;
        }
        if self.payroll_state.government.declarations_open {
            self.payroll_bir_declarations(ui);
            return;
        }
        if self.payroll_state.government.identities_open {
            self.payroll_government_identities(ui);
            return;
        }
        if self.payroll_state.government.annual_open {
            self.payroll_government_annual(ui);
            return;
        }
        if self.payroll_state.government.bir_open {
            self.payroll_bir_file(ui);
            return;
        }
        if self.payroll_state.government.collection.is_some() {
            self.payroll_sss_collection(ui);
            return;
        }
        if !self.payroll_state.government.editing() {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(
                    &mut self.payroll_state.government.registration_open,
                    false,
                    "Reconciliation",
                );
                ui.selectable_value(
                    &mut self.payroll_state.government.registration_open,
                    true,
                    "Registration details",
                );
            });
        }
        if self.payroll_state.government.registration_open {
            self.payroll_government_registration(ui);
            return;
        }
        let intent = if self.payroll_state.government.form.is_some() {
            forms(
                ui,
                &mut self.payroll_state.government,
                &self.data,
                self.busy,
            )
        } else {
            overview(
                ui,
                &mut self.payroll_state.government,
                &self.data,
                self.busy,
            )
        };
        if let Some(intent) = intent {
            let filter = match self.payroll_state.government.filter() {
                Ok(f) => f,
                Err(e) => {
                    self.notice = Some((e.to_string(), true));
                    return;
                }
            };
            match intent {
                Intent::Annual => {
                    self.payroll_state.government.annual_open = true;
                }
                Intent::Declarations => {
                    self.payroll_state.government.declarations_open = true;
                }
                Intent::Identities => {
                    self.payroll_state.government.identities_open = true;
                }
                Intent::CheckBir => {
                    self.payroll_state.government.bir_open = true;
                }
                Intent::BuildBir => {
                    self.payroll_state.government.builder =
                        super::bir_builder_view::State::new(self.data.company.today().year());
                    self.payroll_state.government.builder_open = true;
                }
                Intent::PrepareSss => self.send(Command::GovernmentSssPrepare { filter }),
                Intent::Read => self.send(Command::Government {
                    filter,
                    action: None,
                    destination: None,
                }),
                Intent::Save(a) => {
                    self.payroll_state.government.pending = Some(a.clone());
                    self.send(Command::Government {
                        filter,
                        action: Some(a),
                        destination: None,
                    });
                }
                Intent::Export => match crate::reports::choose_export(&format!(
                    "HRIS-government-{}-{}.{}",
                    filter.agency.label(),
                    filter.month,
                    if self.payroll_state.government.export_excel {
                        "xlsx"
                    } else {
                        "csv"
                    }
                )) {
                    Ok(Some(path)) => self.send(Command::Government {
                        filter,
                        action: None,
                        destination: Some(path),
                    }),
                    Ok(None) => {}
                    Err(e) => self.notice = Some((e.to_string(), true)),
                },
                Intent::ExportPreparation => match crate::reports::choose_export(&format!(
                    "HRIS-{}-preparation-{}.{}",
                    filter.agency.label(),
                    filter.month,
                    if self.payroll_state.government.export_excel {
                        "xlsx"
                    } else {
                        "csv"
                    }
                )) {
                    Ok(Some(path)) => self.send(Command::GovernmentPreparation {
                        filter,
                        destination: path,
                    }),
                    Ok(None) => {}
                    Err(e) => self.notice = Some((e.to_string(), true)),
                },
            }
        }
    }
}
fn overview(ui: &mut Ui, state: &mut State, data: &Snapshot, busy: bool) -> Option<Intent> {
    ui.heading("Government reconciliation");
    ui.label("Review contribution coverage and compensation withholding against recorded government remittances. Agency filing and actual payment remain separate.");
    let today = data.company.today();
    let lo = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let mut out = None;
    ui.horizontal_wrapped(|ui| {
        let old = (state.month.clone(), state.agency);
        month(ui, "government_period", &mut state.month, lo, today);
        egui::ComboBox::from_id_salt("government_agency")
            .selected_text(state.agency.unwrap_or(g::Agency::Sss).label())
            .show_ui(ui, |ui| {
                for a in [
                    g::Agency::Sss,
                    g::Agency::Philhealth,
                    g::Agency::Pagibig,
                    g::Agency::Bir,
                ] {
                    ui.selectable_value(&mut state.agency, Some(a), a.label());
                }
            });
        if old != (state.month.clone(), state.agency) {
            state.page = 0;
            state.receipt_page = 0;
            state.history = None;
        }
    });
    let connected = data.mode == Mode::Supabase;
    let preview = cfg!(feature = "qa-screenshot") && state.current().is_some();
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(!busy, |ui| {
            egui::ComboBox::from_id_salt("government_export_format")
                .selected_text(if state.export_excel {
                    "Excel (.xlsx)"
                } else {
                    "CSV (.csv)"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.export_excel, false, "CSV (.csv)");
                    ui.selectable_value(&mut state.export_excel, true, "Excel (.xlsx)");
                });
        });
        if state.agency == Some(g::Agency::Bir)
            && ui
                .add_enabled(
                    !busy && connected,
                    egui::Button::new("Check BIR 1604C file"),
                )
                .clicked()
        {
            out = Some(Intent::CheckBir);
        }
        if state.agency == Some(g::Agency::Bir)
            && ui
                .add_enabled(
                    !busy && connected,
                    egui::Button::new("Build BIR 1604C draft"),
                )
                .clicked()
        {
            out = Some(Intent::BuildBir);
        }
        if state.agency == Some(g::Agency::Bir)
            && ui
                .add_enabled(
                    !busy && connected,
                    egui::Button::new("Annual recorded sources"),
                )
                .clicked()
        {
            out = Some(Intent::Annual);
        }
        if state.agency == Some(g::Agency::Bir)
            && ui
                .add_enabled(
                    !busy && connected,
                    egui::Button::new("Review BIR file identities"),
                )
                .clicked()
        {
            out = Some(Intent::Identities);
        }
        if state.agency == Some(g::Agency::Bir)
            && ui
                .add_enabled(
                    !busy && connected,
                    egui::Button::new("Review declarations and sources"),
                )
                .clicked()
        {
            out = Some(Intent::Declarations);
        }
        if ui
            .add_enabled(!busy && connected, egui::Button::new("Generate / refresh"))
            .clicked()
        {
            out = Some(Intent::Read);
        }
        if ui
            .add_enabled(
                !busy && connected && state.current().is_some(),
                egui::Button::new("Export fresh review"),
            )
            .clicked()
        {
            out = Some(Intent::Export);
        }
        if ui
            .add_enabled(
                !busy && connected && state.current().is_some(),
                egui::Button::new("Export agency preparation"),
            )
            .clicked()
        {
            out = Some(Intent::ExportPreparation);
        }
        if ui
            .add_enabled(
                !busy
                    && connected
                    && state.current().is_some()
                    && state.agency == Some(g::Agency::Sss),
                egui::Button::new("Prepare SSS R3 reference draft"),
            )
            .clicked()
        {
            out = Some(Intent::PrepareSss);
        }
        if ui
            .add_enabled(
                !busy && connected && data.access.role == Role::Administrator,
                egui::Button::new("Record remittance"),
            )
            .clicked()
        {
            state.form = Some(Form::Remittance(RemittanceForm {
                id: Uuid::new_v4(),
                agency: state.agency.unwrap_or(g::Agency::Sss),
                date: today.to_string(),
                rows: vec![ReceiptRow {
                    month: state.month.clone(),
                    amount: String::new(),
                }],
                amount: String::new(),
                reference: String::new(),
                evidence: String::new(),
                reason: String::new(),
                replaces: None,
                confirmed: false,
            }));
        }
    });
    ui.add(egui::Label::new("Agency preparation contains the selected agency's registered identifiers and legal names. Keep it private to HR; it is an internal review worksheet. Excel preserves identifiers and exact PHP amounts as text; totals are captured values and do not recalculate when edited.").wrap());
    if !connected && !preview {
        ui.label("Company account sign-in is required for government reconciliation.");
        return out;
    }
    let Some(owned) = state.report.take() else {
        ui.label("Generate the selected agency/month to review its current facts.");
        return out;
    };
    if state.filter().map_or(true, |f| f != owned.filter()) {
        state.report = Some(owned);
        ui.label("Generate the selected agency/month to review its current facts.");
        return out;
    }
    let report = &owned;
    let total = &report.summary;
    ui.label(format!(
        "As of {} · Generated {}",
        report.wire.source.as_of,
        report.wire.source.generated_at.format("%Y-%m-%d %H:%M UTC")
    ));
    card(ui, |ui| {
        entry(
            ui,
            "Known employee share",
            part(total.known_parts.employee_cents),
        );
        entry(
            ui,
            "Known employer share",
            part(total.known_parts.employer_cents),
        );
        if report.filter().agency == g::Agency::Sss {
            entry(
                ui,
                "Known employer-only EC",
                part(total.known_parts.ec_cents),
            );
        }
        entry(
            ui,
            "Complete source liability",
            if total.complete {
                part(total.known_parts.total())
            } else {
                "Unresolved — review source issues".into()
            },
        );
        entry(
            ui,
            "Recorded remittances for this month",
            crate::payroll::money(total.remitted_cents),
        );
        entry(
            ui,
            "Liability minus recorded remittances",
            part(total.difference_cents),
        );
    });
    if !total.complete {
        ui.label(RichText::new(format!("Needs review: {} unallocated source(s), {} missing individual receipt(s), {} postdated receipt(s), {} negative person balance(s). Unknown shares remain unresolved.",total.unallocated_count,total.missing_receipt_count,total.dated_after_as_of_count,total.negative_balance_count)).color(muted(ui)));
    }
    ui.label(if report.filter().agency==g::Agency::Bir{"Withholding follows the person's actual received date. Unpaid approvals and tax advice do not establish a payment."}else{"Contributions follow reviewed original coverage. Special payouts require explicit employer/EC shares; an excess is a discrepancy for review, not an automatic agency credit."});
    ui.separator();
    ui.heading("Payroll sources");
    #[cfg(feature = "qa-screenshot")]
    if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("government-sources") {
        ui.scroll_to_cursor(Some(Align::TOP));
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Find employee");
        if ui.text_edit_singleline(&mut state.search).changed() {
            state.page = 0;
        }
    });
    let query = state.search.trim().to_lowercase();
    let rows: Vec<_> = report
        .wire
        .source
        .facts
        .iter()
        .filter(|f| {
            query.is_empty() || label(report, f.employee_id).to_lowercase().contains(&query)
        })
        .collect();
    let mut form = None;
    let mut selected_history = None;
    let pages = rows.len().div_ceil(20).max(1);
    state.page = state.page.min(pages - 1);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(state.page > 0, egui::Button::new("Previous sources"))
            .clicked()
        {
            state.page -= 1;
        }
        ui.label(format!(
            "Page {} / {} · {} sources",
            state.page + 1,
            pages,
            rows.len()
        ));
        if ui
            .add_enabled(state.page + 1 < pages, egui::Button::new("Next sources"))
            .clicked()
        {
            state.page += 1;
        }
    });
    for f in rows.into_iter().skip(state.page * 20).take(20) {
        ui.push_id((f.kind, f.id), |ui| {
            card(ui, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(label(report, f.employee_id)).strong()).wrap(),
                );
                ui.label(format!(
                    "{} · {} to {} · {}",
                    f.kind.label(),
                    f.starts_on,
                    f.ends_on,
                    f.state.label()
                ));
                let approved = report.approved(f);
                let pending = report.pending(f);
                entry(
                    ui,
                    "Issued employee share / adjustment",
                    part(f.parts.employee_cents),
                );
                if let Some(ef)=report.wire.effective_facts.iter().find(|ef|ef.id==f.id&&ef.kind==f.kind).filter(|ef|report.included(ef)){
                    entry(ui,"Selected month employee share",part(ef.parts.employee_cents));
                    if f.agency!=g::Agency::Bir{entry(ui,"Selected month employer share",part(ef.parts.employer_cents));}
                    if f.agency==g::Agency::Sss{entry(ui,"Selected month EC",part(ef.parts.ec_cents));}
                }else{ui.label(RichText::new("Not included in this month's liability; review coverage, receipt date or source status.").color(muted(ui)));}
                entry(
                    ui,
                    "Coverage",
                    approved
                        .map(|r| {
                            r.allocations
                                .iter()
                                .map(|a| a.month.format("%b %Y").to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .or(f.contribution_month.map(|d| d.format("%B %Y").to_string()))
                        .unwrap_or("Not allocated".into()),
                );
                if let Some(r) = approved {
                    entry(
                        ui,
                        "Approved coverage version",
                        r.approved_at
                            .map(|d| d.format("%d %b %Y %H:%M UTC").to_string())
                            .unwrap_or_default(),
                    );
                    if ui.button("Coverage history").clicked() {
                        selected_history = Some(r.id);
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if let Some(r) = pending {
                        let available=!matches!(f.state,g::State::Withdrawn|g::State::ReviewOnly);
                        ui.label(format!("{} review", r.status));
                        if r.status == "draft"
                            && ui
                                .add_enabled(!busy&&available, egui::Button::new("Edit draft"))
                                .clicked()
                        {
                            form = Some(Form::Allocation(allocation_form(f, Some(r), None)));
                        }
                        if r.status == "draft"
                            && ui.add_enabled(!busy&&available, egui::Button::new("Submit")).clicked()
                        {
                            form = Some(Form::Decision(DecisionForm {
                                record: r.clone(),
                                operation: "submit".into(),
                                reason: String::new(),
                                reviewed: false,
                            }));
                        }
                        if r.status == "submitted" {
                            let can = available&&data.access.role == Role::Administrator
                                && !r.contributors.contains(&data.access.user_id)
                                && data.access.employee_id != Some(r.employee_id);
                            if ui
                                .add_enabled(!busy && can, egui::Button::new("Approve"))
                                .clicked()
                            {
                                form = Some(Form::Decision(DecisionForm {
                                    record: r.clone(),
                                    operation: "approve".into(),
                                    reason: String::new(),
                                    reviewed: false,
                                }));
                            }
                            if !can {
                                ui.label("Independent Administrator required");
                            }
                            if ui
                                .add_enabled(!busy, egui::Button::new("Return for editing"))
                                .clicked()
                            {
                                form = Some(Form::Decision(DecisionForm {
                                    record: r.clone(),
                                    operation: "return".into(),
                                    reason: String::new(),
                                    reviewed: false,
                                }));
                            }
                        }
                        if ui
                            .add_enabled(!busy, egui::Button::new("Cancel review"))
                            .clicked()
                        {
                            form = Some(Form::Decision(DecisionForm {
                                record: r.clone(),
                                operation: "cancel".into(),
                                reason: String::new(),
                                reviewed: false,
                            }));
                        }
                        if ui.button("Review history").clicked() {
                            selected_history = Some(r.id);
                        }
                    } else if matches!(
                        f.kind,
                        g::Kind::Thirteenth | g::Kind::Amendment | g::Kind::FinalContribution
                    ) && !matches!(f.state, g::State::Withdrawn | g::State::ReviewOnly)
                        && f.agency != g::Agency::Bir
                    {
                        if ui
                            .add_enabled(
                                !busy,
                                egui::Button::new(if approved.is_some() {
                                    "Revise coverage"
                                } else {
                                    "Review coverage / shares"
                                }),
                            )
                            .clicked()
                        {
                            form = Some(Form::Allocation(allocation_form(f, None, approved)));
                        }
                    }
                });
            })
        });
    }
    ui.separator();
    ui.heading("Recorded government remittances");
    let receipts = &report.wire.remittances;
    let pages = receipts.len().div_ceil(20).max(1);
    state.receipt_page = state.receipt_page.min(pages - 1);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                state.receipt_page > 0,
                egui::Button::new("Previous receipts"),
            )
            .clicked()
        {
            state.receipt_page -= 1;
        }
        ui.label(format!(
            "Page {} / {} · {} records",
            state.receipt_page + 1,
            pages,
            receipts.len()
        ));
        if ui
            .add_enabled(
                state.receipt_page + 1 < pages,
                egui::Button::new("Next receipts"),
            )
            .clicked()
        {
            state.receipt_page += 1;
        }
    });
    if report.wire.remittances.is_empty() {
        ui.label("No government remittance is recorded for this agency/month.");
    }
    for r in report
        .wire
        .remittances
        .iter()
        .rev()
        .skip(state.receipt_page * 20)
        .take(20)
    {
        card(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(&r.reference).strong()).wrap());
            entry(ui, "Actual payment date", r.paid_on.to_string());
            entry(
                ui,
                "Whole receipt amount",
                crate::payroll::money(r.amount_cents),
            );
            entry(
                ui,
                "Selected month coverage",
                crate::payroll::money(r.covered_cents(report.filter().month)),
            );
            entry(
                ui,
                "Status",
                if r.void_id.is_some() {
                    "Voided — excluded"
                } else {
                    "Recorded"
                },
            );
            ui.collapsing("Supporting record", |ui| {
                ui.add(egui::Label::new(&r.evidence).wrap());
                ui.add(egui::Label::new(&r.note).wrap());
                if let Some(why) = &r.void_reason {
                    ui.add(egui::Label::new(format!("Void reason: {why}")).wrap());
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Receipt history").clicked() {
                    selected_history = Some(r.id);
                }
                if data.access.role == Role::Administrator {
                    if r.void_id.is_none()
                        && ui
                            .add_enabled(!busy, egui::Button::new("Void incorrect record"))
                            .clicked()
                    {
                        form = Some(Form::Void(VoidForm {
                            receipt: r.clone(),
                            id: Uuid::new_v4(),
                            reason: String::new(),
                            confirmed: false,
                        }));
                    }
                    if r.void_id.is_some()
                        && r.replacement_id.is_none()
                        && ui
                            .add_enabled(!busy, egui::Button::new("Correct / replace"))
                            .clicked()
                    {
                        form = Some(Form::Remittance(RemittanceForm {
                            id: Uuid::new_v4(),
                            agency: r.agency,
                            date: r.paid_on.to_string(),
                            rows: r
                                .allocations
                                .iter()
                                .map(|a| ReceiptRow {
                                    month: a.month.to_string(),
                                    amount: g::decimal(a.amount_cents),
                                })
                                .collect(),
                            amount: g::decimal(r.amount_cents),
                            reference: r.reference.clone(),
                            evidence: r.evidence.clone(),
                            reason: String::new(),
                            replaces: Some(r.id),
                            confirmed: false,
                        }));
                    }
                }
            });
        });
    }
    if let Some(id) = selected_history {
        state.history = Some(id);
    }
    if let Some(id) = state.history {
        ui.separator();
        ui.heading("Record history");
        if ui.button("Close history").clicked() {
            state.history = None;
        }
        for e in report.wire.history.iter().filter(|e| e.record_id == id) {
            card(ui, |ui| {
                ui.label(format!(
                    "{} · {} · {}",
                    e.recorded_at.format("%d %b %Y %H:%M UTC"),
                    e.actor_name,
                    e.kind.replace('_', " ")
                ));
                ui.add(egui::Label::new(&e.reason).wrap());
            });
        }
    }
    if form.is_some() {
        state.form = form;
    }
    state.report = Some(owned);
    out
}

// Form controls are isolated from background snapshots; failed saves retain input.
fn forms(ui: &mut Ui, state: &mut State, data: &Snapshot, busy: bool) -> Option<Intent> {
    let today = data.company.today();
    let mut out = None;
    let mut discard = false;
    ui.heading("Government records");
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(!busy, egui::Button::new("Discard unsaved changes"))
            .clicked()
        {
            discard = true;
        }
        if ui
            .add_enabled(!busy, egui::Button::new("Refresh stored records"))
            .clicked()
        {
            out = Some(Intent::Read);
        }
    });
    let mut action = None;
    let mut problem = None;
    let current = state.report.as_ref();
    ui.add_enabled_ui(!busy,|ui|match state.form.as_mut().unwrap(){
  Form::Allocation(f)=>{
   ui.heading(if f.revision.is_some(){"Edit contribution review"}else if f.previous.is_some(){"Revise approved coverage"}else{"Review contribution coverage"});
   ui.label(current.map(|r|label(r,f.basis.employee_id)).unwrap_or("Employee".into()));entry(ui,"Issued employee amount (unchanged)",part(f.basis.parts.employee_cents));
   ui.label("Enter explicit employer and EC amounts, including reviewed zero. Employee allocations must retain the issued total. An independent Administrator approves the saved review.");
   let mut remove=None;
   for(i,row)in f.rows.iter_mut().enumerate(){ui.push_id(i,|ui|card(ui,|ui|{
    ui.label(format!("Coverage {}",i+1));ui.columns(2,|c|{c[0].label("Covered month");month(&mut c[0],"allocation_month",&mut row.month,f.basis.starts_on.max(NaiveDate::from_ymd_opt(2025,1,1).unwrap()),f.basis.ends_on.min(today));field(&mut c[1],"Employee share (PHP)",&mut row.employee);});
    ui.columns(2,|c|{field(&mut c[0],"Employer share (PHP)",&mut row.employer);if f.basis.agency==g::Agency::Sss{field(&mut c[1],"Employer-only EC (PHP)",&mut row.ec);}else{c[1].label("EC does not apply to this agency.");}});
    if ui.button("Remove month").clicked(){remove=Some(i);}
   }));}
   if let Some(i)=remove{f.rows.remove(i);}if ui.add_enabled(f.rows.len()<24,egui::Button::new("Add covered month")).clicked(){f.rows.push(AllocationRow{month:String::new(),employee:"0.00".into(),employer:String::new(),ec:if f.basis.agency==g::Agency::Sss{String::new()}else{"0.00".into()}});}
   field(ui,"Supporting document / review reference",&mut f.reference);note(ui,"Review note / reason",&mut f.note);
   let stale=allocation_stale(current,f);
   let result=allocation_action(f,today);if let Err(e)=&result{problem=Some(e.to_string());}if stale{problem=Some("The stored review/source changed. Discard this form and open the current version.".into());}
   if ui.add_enabled(!stale&&result.is_ok(),egui::Button::new("Save draft")).clicked(){action=result.ok();}
  }
  Form::Decision(f)=>{
   ui.heading(match f.operation.as_str(){"approve"=>"Approve contribution review","submit"=>"Submit contribution review","return"=>"Return review for editing",_=>"Cancel contribution review"});
   ui.label(current.map(|r|label(r,f.record.employee_id)).unwrap_or("Employee".into()));for a in&f.record.allocations{card(ui,|ui|{entry(ui,"Covered month",a.month.format("%B %Y").to_string());entry(ui,"Employee",crate::payroll::money(a.employee_cents));entry(ui,"Employer",crate::payroll::money(a.employer_cents));if f.record.agency==g::Agency::Sss{entry(ui,"EC",crate::payroll::money(a.ec_cents));}});}
   ui.add(egui::Label::new(&f.record.reference).wrap());ui.add(egui::Label::new(&f.record.note).wrap());
   let review=matches!(f.operation.as_str(),"submit"|"approve");if review{ui.checkbox(&mut f.reviewed,"I reviewed the coverage, supporting evidence and explicit employer/EC shares.");}note(ui,"Action reason",&mut f.reason);
   let stale=decision_stale(current,f);
   if stale{problem=Some("The review changed. Discard and open its current version.".into());}
   let can=!stale&&!f.reason.trim().is_empty()&&(!review||f.reviewed);
   if ui.add_enabled(can,egui::Button::new(match f.operation.as_str(){"approve"=>"Approve","submit"=>"Submit","return"=>"Return for editing",_=>"Cancel review"})).clicked(){let mut record=json!({"id":f.record.id,"expected_revision":f.record.revision,"reason":f.reason});if review{record["reviewed"]=json!(f.reviewed);}action=Some(g::Action{operation:format!("allocation_{}",f.operation),record});}
  }
  Form::Remittance(f)=>{
   ui.heading(if f.replaces.is_some(){"Correct voided remittance record"}else{"Record evidenced remittance"});ui.label(format!("Agency: {}",f.agency.label()));ui.label("Record an actual payment supported by an agency/bank receipt. This records history and does not send money or file a return.");
   ui.columns(2,|c|{field(&mut c[0],"Actual payment date (YYYY-MM-DD)",&mut f.date);field(&mut c[1],"Whole receipt amount (PHP)",&mut f.amount);});
   let mut remove=None;for(i,row)in f.rows.iter_mut().enumerate(){ui.push_id(i,|ui|card(ui,|ui|{ui.columns(2,|c|{c[0].label("Covered month");month(&mut c[0],"receipt_month",&mut row.month,NaiveDate::from_ymd_opt(2025,1,1).unwrap(),today);field(&mut c[1],"This month's share (PHP)",&mut row.amount);});if ui.button("Remove month").clicked(){remove=Some(i);}}));}
   if let Some(i)=remove{f.rows.remove(i);}if ui.add_enabled(f.rows.len()<24,egui::Button::new("Add covered month")).clicked(){f.rows.push(ReceiptRow{month:String::new(),amount:String::new()});}
   field(ui,"Agency / bank receipt reference",&mut f.reference);note(ui,"Supporting evidence / verification reference",&mut f.evidence);note(ui,"Reason for entry or correction",&mut f.reason);ui.checkbox(&mut f.confirmed,"I verified this actual remittance and its covered months.");
   let result=remittance_action(f,today);if let Err(e)=&result{problem=Some(e.to_string());}
   if ui.add_enabled(result.is_ok(),egui::Button::new("Save remittance record")).clicked(){action=result.ok();}
  }
  Form::Void(f)=>{
   ui.heading("Void incorrect remittance entry");ui.add(egui::Label::new(&f.receipt.reference).wrap());entry(ui,"Whole receipt amount",crate::payroll::money(f.receipt.amount_cents));ui.label("The original record and audit trail stay available. Voiding this entry does not reverse an agency or bank payment.");note(ui,"Reason this entry is incorrect or duplicated",&mut f.reason);ui.checkbox(&mut f.confirmed,"I verified that this HRIS entry should be excluded.");
   let stale=current.is_none_or(|r|r.wire.remittances.iter().find(|r|r.id==f.receipt.id).is_none_or(|r|r.void_id.is_some()));
   if stale{problem=Some("This receipt changed. Refresh its current status.".into());}
   if ui.add_enabled(!stale&&f.confirmed&&!f.reason.trim().is_empty(),egui::Button::new("Void record")).clicked(){action=Some(g::Action{operation:"remittance_void".into(),record:json!({"id":f.id,"remittance_id":f.receipt.id,"reason":f.reason})});}
  }
 });
    if let Some(error) = problem {
        ui.add(egui::Label::new(RichText::new(error).color(muted(ui))).wrap());
    }
    if discard {
        state.form = None;
    }
    if let Some(a) = action {
        out = Some(Intent::Save(a));
    }
    out
}
fn remittance_action(f: &RemittanceForm, today: NaiveDate) -> anyhow::Result<g::Action> {
    let date: NaiveDate = f
        .date
        .parse()
        .map_err(|_| anyhow::anyhow!("Enter the actual payment date."))?;
    let amount = g::parse_cents(&f.amount)?;
    let mut total = 0i64;
    let mut dates = std::collections::HashSet::new();
    anyhow::ensure!(
        f.confirmed
            && date >= NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
            && date <= today
            && amount > 0
            && !f.reference.trim().is_empty()
            && !f.evidence.trim().is_empty()
            && !f.reason.trim().is_empty(),
        "Verify the actual date, positive receipt amount, supporting evidence and reason."
    );
    let rows = f
        .rows
        .iter()
        .map(|r| {
            let mon: NaiveDate = r
                .month
                .parse()
                .map_err(|_| anyhow::anyhow!("Choose each covered month."))?;
            let n = g::parse_cents(&r.amount)?;
            anyhow::ensure!(
                mon.day() == 1
                    && mon >= NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
                    && mon <= today
                    && n > 0
                    && dates.insert(mon),
                "Use unique covered months and positive shares."
            );
            total = total
                .checked_add(n)
                .ok_or_else(|| anyhow::anyhow!("Receipt allocation overflow."))?;
            Ok(g::ReceiptAllocation {
                month: mon,
                amount_cents: n,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    anyhow::ensure!(
        (1..=24).contains(&rows.len()) && total == amount,
        "Month shares must exactly equal the whole receipt amount."
    );
    Ok(g::Action {
        operation: "remittance_record".into(),
        record: json!({"id":f.id,"agency":f.agency,"paid_on":date,"amount_cents":amount,"allocations":rows,"reference":f.reference.trim(),"evidence":f.evidence,"note":f.reason,"replaces_id":f.replaces,"received_confirmed":f.confirmed,"reason":f.reason}),
    })
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_government_preview(&mut self, preview: &str) {
        if preview.starts_with("payroll-government-registration") {
            self.prepare_registration_preview(preview);
            return;
        }
        if self.data.mode != Mode::Demo {
            return;
        }
        self.payroll_state.tab = 13;
        if preview.ends_with("manager") {
            self.data.access.role = Role::Manager;
        }
        self.payroll_state.government.bind(&self.data);
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::GovernmentPreparation)
        {
            return;
        }
        let mut report = g::preview();
        assert_eq!(report.wire.source.company_id, self.data.company.id);
        report.wire.source.company_name = self.data.company.name.clone();
        if preview.ends_with("allocation") {
            let r = &mut report.wire.reviews[0];
            r.status = "draft".into();
            r.revision = 0;
            r.submitted_by = None;
        }
        let state = &mut self.payroll_state.government;
        if preview.starts_with("payroll-government-declarations") {
            state.declarations_open = true;
            state.declarations = super::government_declarations_view::State::preview(preview);
            return;
        }
        if preview.starts_with("payroll-government-identities") {
            state.identities_open = true;
            state.identities = super::government_identity_view::State::preview();
            return;
        }
        if preview.starts_with("payroll-government-annual") {
            state.annual_open = true;
            state.annual = super::government_annual_view::State::preview(preview);
            return;
        }
        if preview.starts_with("payroll-government-bir-builder") {
            state.builder_open = true;
            state.builder = super::bir_builder_view::State::preview(&self.data, preview);
            return;
        }
        if preview.starts_with("payroll-government-bir") {
            state.agency = Some(g::Agency::Bir);
            state.month = report.filter().month.to_string();
            if preview.ends_with("overview") {
                state.report = None;
                return;
            }
            state.bir_open = true;
            state.bir = super::bir_file_view::State::preview(
                preview.contains("errors"),
                preview.contains("totals"),
                preview.contains("mwe"),
            );
            if preview.contains("arithmetic") {
                state.bir.preview_arithmetic();
            }
            return;
        }
        state.export_excel = preview.contains("-excel");
        if preview.starts_with("payroll-government-sss") {
            let p = g::sss_collection::fixture();
            state.month = p.report.filter().month.to_string();
            state.agency = Some(g::Agency::Sss);
            state.collection = Some(super::government_collection_view::State::preview(
                g::sss_collection::Basis::from_preparation(&p).unwrap(),
                preview.ends_with("review"),
            ));
            state.report = Some(p.report);
            return;
        }
        state.month = report.filter().month.to_string();
        state.agency = Some(report.filter().agency);
        state.form = if preview.ends_with("allocation") {
            Some(Form::Allocation(allocation_form(
                &report.wire.source.facts[1],
                Some(&report.wire.reviews[0]),
                None,
            )))
        } else if preview.ends_with("decision") {
            Some(Form::Decision(DecisionForm {
                record: report.wire.reviews[0].clone(),
                operation: "approve".into(),
                reason: String::new(),
                reviewed: false,
            }))
        } else if preview.ends_with("remittance") {
            let r = &report.wire.remittances[0];
            Some(Form::Remittance(RemittanceForm {
                id: Uuid::new_v4(),
                agency: r.agency,
                date: r.paid_on.to_string(),
                rows: r
                    .allocations
                    .iter()
                    .map(|a| ReceiptRow {
                        month: a.month.to_string(),
                        amount: g::decimal(a.amount_cents),
                    })
                    .collect(),
                amount: g::decimal(r.amount_cents),
                reference: "FICTIONAL-NEW-RECEIPT".into(),
                evidence: String::new(),
                reason: String::new(),
                replaces: None,
                confirmed: false,
            }))
        } else if preview.ends_with("void") {
            Some(Form::Void(VoidForm {
                receipt: report.wire.remittances[0].clone(),
                id: Uuid::new_v4(),
                reason: String::new(),
                confirmed: false,
            }))
        } else {
            None
        };
        if preview.ends_with("history") {
            state.history = Some(report.wire.reviews[0].id);
        }
        if let Ok(path) = std::env::var("HRIS_PREVIEW_GOVERNMENT_CSV") {
            std::fs::write(path, report.csv().unwrap()).unwrap();
        }
        state.report = Some(report);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_format_is_account_scoped_and_preserved_by_same_account_refresh() {
        let mut data = DemoStore::new().data;
        let mut state = State::default();
        state.bind(&data);
        state.export_excel = true;
        state.bind(&data);
        assert!(state.export_excel);
        assert!(!state.editing());
        data.access.user_id = Uuid::new_v4();
        state.bind(&data);
        assert!(!state.export_excel);
        state.export_excel = true;
        data.access.role = Role::Manager;
        state.bind(&data);
        assert!(!state.export_excel && state.report.is_none());
    }
    #[test]
    fn unrelated_refresh_and_failed_save_preserve_unsaved_form_but_scope_changes_clear_it() {
        let store = DemoStore::new();
        let report = g::preview();
        let mut s = State::default();
        s.bind(&store.data);
        s.month = report.filter().month.to_string();
        let mut draft = allocation_form(&report.wire.source.facts[1], None, None);
        draft.note = "Unsaved local evidence".into();
        s.form = Some(Form::Allocation(draft));
        s.accept(report.clone(), &store.data);
        assert!(matches!(&s.form,Some(Form::Allocation(f)) if f.note=="Unsaved local evidence"));
        s.pending = Some(g::Action {
            operation: "allocation_create".into(),
            record: json!({"id":Uuid::new_v4()}),
        });
        // The worker's error path clears pending, then a later read must keep input.
        s.pending = None;
        s.accept(report.clone(), &store.data);
        assert!(s.editing());
        s.pending = Some(g::Action {
            operation: "allocation_create".into(),
            record: json!({"id":Uuid::new_v4()}),
        });
        s.accept(report, &store.data);
        assert!(!s.editing());
        s.search = "Private previous scope".into();
        let mut changed = store.data.clone();
        changed.access.user_id = Uuid::new_v4();
        s.bind(&changed);
        assert!(s.report.is_none() && s.search.is_empty());
        changed.access.role = Role::Manager;
        s.bind(&changed);
        assert!(s.report.is_none() && !s.editing());
    }
    #[test]
    fn bir_read_only_results_clear_on_scope_change_and_cannot_reopen_a_closed_view() {
        let data = DemoStore::new().data;
        let mut state = State::default();
        state.bind(&data);
        let report = g::bir_file::Report {
            schedule: g::bir_file::Schedule::One,
            company_id: data.company.id,
            user_id: data.access.user_id,
            sha256: "a".repeat(64),
            file_name: "fictional.dat".into(),
            year: Some(2026),
            details: 1,
            employer_match: None,
            errors: 0,
            reviews: 1,
            issues: Vec::new(),
            totals: Vec::new(),
            arithmetic: Default::default(),
        };
        state.accept_bir(report.clone(), &data);
        assert!(state.bir.report.is_none());
        state.bir_open = true;
        state.accept_bir(report.clone(), &data);
        assert!(state.bir.report.is_some() && !state.editing());
        let mut changed = data.clone();
        changed.access.user_id = Uuid::new_v4();
        state.bind(&changed);
        assert!(!state.bir_open && state.bir.report.is_none());
        state.bind(&data);
        state.bir_open = true;
        state.accept_bir(report.clone(), &data);
        changed = data.clone();
        changed.access.role = Role::Manager;
        state.accept_bir(report, &changed);
        assert!(!state.bir_open && state.bir.report.is_none());
    }
    #[test]
    fn stale_and_withdrawn_sources_block_approval_but_allow_returning_the_saved_review() {
        let mut report = g::preview();
        let mut f = DecisionForm {
            record: report.wire.reviews[0].clone(),
            operation: "approve".into(),
            reason: "Reviewed".into(),
            reviewed: true,
        };
        assert!(!decision_stale(Some(&report), &f));
        report.wire.source.facts[1].state = g::State::Withdrawn;
        assert!(decision_stale(Some(&report), &f));
        f.operation = "return".into();
        assert!(!decision_stale(Some(&report), &f));
        report.wire.reviews[0].revision += 1;
        assert!(decision_stale(Some(&report), &f));
        let a = allocation_form(&report.wire.source.facts[1], None, None);
        assert!(allocation_stale(Some(&report), &a));
        assert!(allocation_stale(None, &a));
    }
    #[test]
    fn remittance_input_keeps_exact_centavos_and_retry_identity_and_rejects_unbalanced_coverage() {
        let report = g::preview();
        let r = &report.wire.remittances[0];
        let mut f = RemittanceForm {
            id: Uuid::new_v4(),
            agency: r.agency,
            date: r.paid_on.to_string(),
            rows: r
                .allocations
                .iter()
                .map(|a| ReceiptRow {
                    month: a.month.to_string(),
                    amount: g::decimal(a.amount_cents),
                })
                .collect(),
            amount: g::decimal(r.amount_cents),
            reference: r.reference.clone(),
            evidence: r.evidence.clone(),
            reason: "Verified synthetic receipt".into(),
            replaces: None,
            confirmed: true,
        };
        let a = remittance_action(&f, report.wire.source.as_of).unwrap();
        assert_eq!(a.record["amount_cents"], json!(500000));
        assert_eq!(
            a.id().unwrap(),
            remittance_action(&f, report.wire.source.as_of)
                .unwrap()
                .id()
                .unwrap()
        );
        f.rows[0].amount = "4599.99".into();
        assert!(remittance_action(&f, report.wire.source.as_of).is_err());
        f.rows[0].amount = "4600.00".into();
        f.confirmed = false;
        assert!(remittance_action(&f, report.wire.source.as_of).is_err());
    }
}
