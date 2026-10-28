use super::*;
use crate::notifications::{self, Category, ReadAction, Target};
#[derive(Default)]
pub(super) struct State {
    pub unread_only: bool,
    pub category: Option<Category>,
    pub page: usize,
    refreshed: Option<std::time::Instant>,
    scope: Option<crate::access::Access>,
    items: std::sync::Arc<Vec<notifications::Alert>>,
    pub unread: usize,
    payroll_only: bool,
}
impl State {
    pub fn invalidate(&mut self) {
        self.refreshed = None;
    }
    pub fn for_product(
        &mut self,
        data: &Snapshot,
        product: crate::desktop::Product,
    ) -> std::sync::Arc<Vec<notifications::Alert>> {
        let payroll_only = product == crate::desktop::Product::Payroll;
        if self.payroll_only != payroll_only
            || self.scope.as_ref() != Some(&data.access)
            || self.refreshed.is_none_or(|at| at.elapsed().as_secs() >= 60)
        {
            let mut items = notifications::alerts(data, chrono::Utc::now());
            if payroll_only {
                items.retain(|a| a.category == Category::Payroll);
            }
            self.payroll_only = payroll_only;
            self.unread = items
                .iter()
                .filter(|a| !notifications::is_read(data, &a.key))
                .count();
            self.items = std::sync::Arc::new(items);
            self.scope = Some(data.access.clone());
            self.refreshed = Some(std::time::Instant::now());
        }
        self.items.clone()
    }
}
impl HrisApp {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_notifications_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("notifications") || self.data.mode != Mode::Demo {
            return;
        }
        use crate::documents::{DocumentCategory, DocumentDetails, EmployeeDocument};
        let now = chrono::Utc::now();
        let today = self.data.company.today();
        let own = self
            .data
            .access
            .employee_id
            .unwrap_or(self.data.employees[0].id);
        self.data.leave.clear();
        self.data.documents.clear();
        self.data.attendance.clear();
        self.data.requests = Default::default();
        self.data.overtime = Default::default();
        self.data.leave_setup.events.clear();
        let peer = self.data.employees.iter().find(|e| e.id != own).unwrap().id;
        if form.contains("manager") {
            self.data.access.role = Role::Manager;
        }
        if self.data.access.role == Role::Manager {
            self.data.access.direct_reports = vec![peer];
        }
        for (i, (person, status, offset)) in [
            (peer, LeaveStatus::Pending, 1),
            (own, LeaveStatus::Approved, 3),
        ]
        .into_iter()
        .enumerate()
        {
            let date = today + Duration::days(offset);
            let mut r = LeaveDraft {
                employee_id: Some(person),
                starts_on: date.to_string(),
                ends_on: date.to_string(),
                reason: "Fictional leave fixture".into(),
                ..Default::default()
            }
            .request(self.data.company.id)
            .unwrap();
            r.id = Uuid::from_u128(99500 + i as u128);
            r.status = status;
            r.policy.revision = 1;
            if status == LeaveStatus::Approved {
                self.data.leave_setup.events.push(crate::leave::LeaveEvent {
                    id: Uuid::from_u128(99600 + i as u128),
                    company_id: self.data.company.id,
                    request_id: r.id,
                    employee_id: person,
                    actor_id: None,
                    actor_name: "Sample HR".into(),
                    action: "approved".into(),
                    note: String::new(),
                    created_at: now,
                    revision: 1,
                });
            }
            self.data.leave.push(r);
        }
        for (i, offset) in [-2, 0, 7, 30].into_iter().enumerate() {
            self.data.documents.push(EmployeeDocument{id:Uuid::from_u128(99700+i as u128),company_id:self.data.company.id,employee_id:peer,
             details:DocumentDetails{title:if form.contains("long") {"Employment contract and supporting certification with an intentionally long reference for checking how reminder cards wrap within the window".into()}else{format!("Sample contract {}",i+1)},category:DocumentCategory::Contract,notes:String::new(),issued_on:None,expires_on:Some(today+Duration::days(offset)),contract_start:Some(today-Duration::days(365)),contract_end:Some(today+Duration::days(offset))},
             original_filename:"fictional-contract.pdf".into(),mime_type:"application/pdf".into(),byte_size:1,sha256:"a".repeat(64),object_name:"fictional/preview.pdf".into(),state:DocumentState::Ready,archived:false,revision:1,created_by:Uuid::nil(),created_by_name:"Demo HR".into(),created_at:now,updated_at:now});
        }
        self.data.attendance.push(Attendance {
            id: Uuid::from_u128(99800),
            company_id: self.data.company.id,
            employee_id: own,
            work_date: today - Duration::days(2),
            checked_in_at: now - Duration::hours(30),
            checked_out_at: None,
            revision: 0,
            planned_shift: None,
            calculation_snapshot: None,
        });
        self.data.notification_reads.read_keys = Vec::new();
        if form.contains("lifecycle") {
            self.data.access.employee_id = Some(own);
            self.data.lifecycle_notifications =
                Some(notifications::lifecycle::preview(&self.data, now));
        }
        if form.contains("read") {
            let keys = notifications::alerts(&self.data, now)
                .iter()
                .take(3)
                .map(|a| a.key.clone())
                .collect();
            notifications::apply_reads(&mut self.data, &ReadAction::Mark { keys, read: true })
                .unwrap();
        }
        if form.contains("unread") {
            self.notification_state.unread_only = true;
        }
        if form.contains("documents") {
            self.notification_state.category = Some(Category::Documents);
        }
        if form.contains("approvals") {
            self.notification_state.category = Some(Category::Approvals);
        }
        if form.contains("payroll") {
            self.notification_state.category = Some(Category::Payroll);
        }
        if form.contains("learning") {
            self.notification_state.category = Some(Category::Learning);
        }
        if form.contains("empty") {
            self.data.leave.clear();
            self.data.documents.clear();
            self.data.attendance.clear();
        }
        self.page = Page::Notifications;
    }
    pub(super) fn notifications_page(&mut self, ui: &mut Ui) {
        let all = self
            .notification_state
            .for_product(&self.data, self.desktop.product);
        let unread = all
            .iter()
            .filter(|a| !notifications::is_read(&self.data, &a.key))
            .count();
        if let Some(s) = self
            .data
            .lifecycle_notifications
            .as_ref()
            .filter(|s| s.truncated)
        {
            ui.add(egui::Label::new(format!("Showing {} of {} lifecycle notices. The current feed is bounded; older notices are not included in this view.",s.items.len(),s.total_items)).wrap());
        }
        card(ui, |ui| {
            ui.label(
                RichText::new(format!("{unread} unread · {} current items", all.len())).strong(),
            );
            ui.label(RichText::new("Updates refresh after sign-in, every minute while connected, and with Refresh. Read items remain actionable; a changed request or expiry stage can appear as a new item.").small().color(muted(ui)));
            let before = (
                self.notification_state.unread_only,
                self.notification_state.category,
            );
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.notification_state.unread_only, "Unread only");
                ui.selectable_value(&mut self.notification_state.category, None, "All");
                for c in Category::ALL {
                    if (self.desktop.product != crate::desktop::Product::Payroll
                        || c == Category::Payroll)
                        && (c != Category::Documents
                            || self
                                .data
                                .access
                                .allows(crate::staff_roles::Capability::PersonnelDocuments))
                    {
                        ui.selectable_value(
                            &mut self.notification_state.category,
                            Some(c),
                            c.label(),
                        );
                    }
                }
            });
            if before
                != (
                    self.notification_state.unread_only,
                    self.notification_state.category,
                )
            {
                self.notification_state.page = 0;
            }
        });
        let rows: Vec<_> = all
            .iter()
            .filter(|a| {
                (!self.notification_state.unread_only
                    || !notifications::is_read(&self.data, &a.key))
                    && self
                        .notification_state
                        .category
                        .is_none_or(|c| c == a.category)
            })
            .cloned()
            .collect();
        let pages = rows.len().max(1).div_ceil(25);
        self.notification_state.page = self.notification_state.page.min(pages - 1);
        let start = self.notification_state.page * 25;
        let page = &rows[start..(start + 25).min(rows.len())];
        let to_read: Vec<_> = page
            .iter()
            .filter(|a| !notifications::is_read(&self.data, &a.key))
            .map(|a| a.key.clone())
            .collect();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.notification_state.page > 0,
                    egui::Button::new("Previous"),
                )
                .clicked()
            {
                self.notification_state.page -= 1;
            }
            ui.label(format!(
                "Page {} of {pages}",
                self.notification_state.page + 1
            ));
            if ui
                .add_enabled(
                    self.notification_state.page + 1 < pages,
                    egui::Button::new("Next"),
                )
                .clicked()
            {
                self.notification_state.page += 1;
            }
            if ui
                .add_enabled(
                    !to_read.is_empty(),
                    egui::Button::new("Mark this page read"),
                )
                .clicked()
            {
                self.send(Command::Change(Action::NotificationReads(
                    ReadAction::Mark {
                        keys: to_read,
                        read: true,
                    },
                )));
            }
        });
        if page.is_empty() {
            empty(ui, "No current notifications match these filters.");
        }
        for a in page {
            let read = notifications::is_read(&self.data, &a.key);
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(if read { "Read" } else { "Unread" })
                            .small()
                            .color(if read { muted(ui) } else { accent(ui) }),
                    );
                    ui.label(RichText::new(a.category.label()).small().color(muted(ui)));
                    if a.urgent {
                        ui.label(
                            RichText::new("Needs attention")
                                .small()
                                .color(ui.visuals().warn_fg_color),
                        );
                    }
                });
                ui.label(RichText::new(&a.title).strong());
                ui.label(&a.detail);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Open record").clicked() {
                        self.send(Command::Change(Action::NotificationReads(
                            ReadAction::Mark {
                                keys: vec![a.key.clone()],
                                read: true,
                            },
                        )));
                        self.open_notification(&a.target, ui.ctx());
                    }
                    if ui
                        .button(if read { "Mark unread" } else { "Mark read" })
                        .clicked()
                    {
                        self.send(Command::Change(Action::NotificationReads(
                            ReadAction::Mark {
                                keys: vec![a.key.clone()],
                                read: !read,
                            },
                        )));
                    }
                });
            });
        }
        if self.desktop.product == crate::desktop::Product::Hris {
            ui.collapsing("Read history",|ui| {
            ui.label(RichText::new(format!("{} read items saved privately to your account. Reset marks current items unread; source records and approvals are unchanged.",notifications::read_count(&self.data))).small().color(muted(ui)));
            if ui.add_enabled(notifications::read_count(&self.data)>0,egui::Button::new("Reset read history")).clicked(){self.send(Command::Change(Action::NotificationReads(ReadAction::Reset)));}
        });
        } else {
            ui.label(RichText::new("Read markers follow your own account across HRIS and Payroll. Mark individual items or the current page; unrelated HR read history is preserved.").small().color(muted(ui)));
        }
        if self.desktop.product == crate::desktop::Product::Hris {
            ui.label(RichText::new("Document reminders cover active employees' filed, unarchived documents: expiry or contract end within 30 days, today, or past date. Your upcoming approved leave covers seven days. Own request updates cover the last 30 days. Unfinished check-outs require at least 24 hours elapsed. Read is not an approval. Email delivery and device connections are separate setup work.").small().color(muted(ui)));
        }
        ui.add(egui::Label::new(RichText::new("Payroll notices separate issued advice from recorded individual payments. Appraisal and training deadlines cover the next seven days and overdue open tasks; recorded certificates cover 30 days and past expiry. Read status does not approve or change a record.").small().color(muted(ui))).wrap());
    }
    fn open_notification(&mut self, target: &Target, ctx: &egui::Context) {
        match target {
            Target::Leave(id) => {
                self.navigate(Page::Leave, ctx);
                self.leave_state.view = leave_view::View::Requests;
                self.leave_state.detail = Some(*id);
            }
            Target::Correction(id) => {
                self.navigate(Page::Attendance, ctx);
                self.shift_state.tab = shifts_view::Tab::Requests;
                self.request_state.detail = Some(*id);
            }
            Target::Overtime(id) => {
                self.navigate(Page::Attendance, ctx);
                self.shift_state.tab = shifts_view::Tab::Overtime;
                self.overtime_state.detail = Some(*id);
            }
            Target::Document { employee, title } => {
                self.navigate(Page::Employees, ctx);
                self.open_employee(*employee);
                self.contacts_section = ContactSection::Documents;
                self.document_search = title.clone();
            }
            Target::Attendance(date) => {
                self.navigate(Page::Attendance, ctx);
                self.shift_state.tab = shifts_view::Tab::Daily;
                self.attendance_date = *date;
                self.calendar_month = month_start(*date);
            }
            Target::Lifecycle {
                page,
                record,
                employee,
            } => {
                if page.starts_with("payroll_") {
                    self.navigate(Page::Payroll, ctx);
                    self.payroll_state.open_notice(page, *record);
                } else {
                    self.navigate(Page::Employees, ctx);
                    self.employee_view = EmployeeView::Development;
                    if page == "appraisal" {
                        self.performance_state.open_notice(*record);
                    } else {
                        self.performance_state
                            .open_learning_notice(page == "certificate");
                        self.learning_state.open_notice(
                            &self.data,
                            *employee,
                            *record,
                            page == "certificate",
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod product_filter_tests {
    use super::*;
    #[test]
    fn product_changes_invalidate_shared_notice_cache_and_keep_only_financial_alerts_in_payroll() {
        let mut d = DemoStore::new().data;
        let payroll = Uuid::new_v4();
        let training = Uuid::new_v4();
        d.lifecycle_notifications=Some(serde_json::from_value(serde_json::json!({"version":"lifecycle-notifications-v1","company_id":d.company.id,"user_id":d.access.user_id,"employee_id":d.access.employee_id,"role":d.access.role,"surface":"desktop","as_of":d.company.today(),"generated_at":chrono::Utc::now(),"read_keys":[],"total_items":2,"truncated":false,"items":[{"key":format!("payroll-monthly-issued:{payroll}:0"),"kind":"payroll-monthly-issued","title":"Own fictional payslip","detail":"Issued advice","date":d.company.today(),"urgent":false,"target_id":payroll,"target_page":"payroll_monthly","employee_id":d.access.employee_id},{"key":format!("training-due:{training}:0:today"),"kind":"training-due","title":"Own fictional training","detail":"Training task","date":d.company.today(),"urgent":true,"target_id":training,"target_page":"training","employee_id":d.access.employee_id}]})).unwrap());
        let mut state = State::default();
        let all = state.for_product(&d, crate::desktop::Product::Hris);
        let payroll = state.for_product(&d, crate::desktop::Product::Payroll);
        assert!(!payroll.is_empty());
        assert!(payroll.iter().all(|n| n.category == Category::Payroll));
        assert!(all.len() > payroll.len());
        let restored = state.for_product(&d, crate::desktop::Product::Hris);
        assert_eq!(all.len(), restored.len());
    }
}
