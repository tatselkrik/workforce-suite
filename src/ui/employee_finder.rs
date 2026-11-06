use super::*;
use crate::{desktop::Product, domain::Employee};

const QUERY_ID: &str = "find-employee-query";
const MAX_RESULTS: usize = 40;

pub(super) struct State {
    query: String,
    focus_query: bool,
}

fn employee_matches<'a>(data: &'a Snapshot, query: &str) -> Vec<&'a Employee> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut matches: Vec<_> = data
        .employees
        .iter()
        .filter(|employee| {
            employee.company_id == data.company.id
                && data.access.can_read_operational_employee(employee)
        })
        .filter(|employee| {
            employee.full_name.to_lowercase().contains(&query)
                || employee.employee_code.to_lowercase().contains(&query)
        })
        .collect();
    matches.sort_by(|a, b| (&a.full_name, &a.employee_code).cmp(&(&b.full_name, &b.employee_code)));
    matches
}

fn finder_dialog(
    ctx: &egui::Context,
    data: &Snapshot,
    state: &mut State,
    can_open: bool,
) -> egui::ModalResponse<Option<Uuid>> {
    egui::Modal::new(egui::Id::unique("employee-finder")).show(ctx, |ui| {
        let width = (ctx.content_rect().width() - 72.0).clamp(240.0, 520.0);
        ui.set_width(width);
        ui.horizontal(|ui| {
            ui.heading("Find employee");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    ui.close();
                }
            });
        });
        let query = ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .id(egui::Id::unique(QUERY_ID))
                .hint_text("Name or employee code")
                .char_limit(120)
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(&mut state.focus_query) {
            query.request_focus();
        }
        if state.query.trim().is_empty() {
            ui.label("Enter a name or employee code to search your directory.");
            return None;
        }
        let results = employee_matches(data, &state.query);
        if results.is_empty() {
            ui.label("No employees match your search.");
            return None;
        }
        let enter = (query.has_focus() || query.lost_focus())
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let mut open = if can_open && enter && results.len() == 1 {
            Some(results[0].id)
        } else {
            None
        };
        let shown = results.len().min(MAX_RESULTS);
        let height = (ctx.content_rect().height() - 220.0).clamp(96.0, 300.0);
        let results_scroll = egui::ScrollArea::vertical()
            .id_salt("employee-finder-results")
            .max_height(height);
        let results_scroll = if query.changed() {
            results_scroll.vertical_scroll_offset(0.0)
        } else {
            results_scroll
        };
        results_scroll.show(ui, |ui| {
            for employee in results.iter().take(shown) {
                let archived = if employee.status == EmployeeStatus::Archived {
                    " · Archived"
                } else {
                    ""
                };
                let label = format!(
                    "{} · {}{archived}",
                    employee.full_name, employee.employee_code,
                );
                let button = egui::Button::new(label)
                    .wrap_mode(egui::TextWrapMode::Wrap)
                    .min_size(Vec2::new(ui.available_width(), 30.0));
                if ui.add_enabled(can_open, button).clicked() {
                    open = Some(employee.id);
                }
            }
        });
        if results.len() > MAX_RESULTS {
            ui.small(format!(
                "Showing {shown} of {}. Type more to narrow the results.",
                results.len()
            ));
        } else if results.len() == 1 {
            ui.small("Press Enter to open the match.");
        }
        open
    })
}

impl HrisApp {
    pub(super) fn employee_finder_available(&self) -> bool {
        self.desktop.product == Product::Hris
            && self.data.mode != Mode::SignedOut
            && self.data.access.uses_desktop()
            && workspaces::people_directory(&self.data.access)
    }

    fn can_open_employee_finder(&self) -> bool {
        self.employee_finder_available()
            && !self.busy
            && !self.has_unsaved_edits()
            && self.correction_history.is_none()
            && self.session_confirmation.is_none()
            && !self.update_confirmation
    }

    fn employee_in_finder_scope(&self, id: Uuid) -> bool {
        self.employee_finder_available()
            && self.data.employees.iter().any(|employee| {
                employee.id == id
                    && employee.company_id == self.data.company.id
                    && self.data.access.can_read_operational_employee(employee)
            })
    }

    pub(super) fn reconcile_found_employee(&mut self, ctx: &egui::Context) {
        if self.employee_record_search.is_some()
            && self
                .employee_form
                .as_ref()
                .and_then(|draft| draft.id)
                .is_none_or(|id| !self.employee_in_finder_scope(id))
        {
            self.employee_record_search = None;
            self.employee_form = None;
            self.contacts_form = None;
            self.document_form = None;
            self.account_form = None;
            self.clear_profile_secrets(ctx);
        }
    }

    pub(super) fn return_to_employee_search(&mut self) {
        if let Some(mut state) = self.employee_record_search.take() {
            state.focus_query = true;
            self.employee_finder = Some(state);
        }
    }

    fn open_employee_finder(&mut self) {
        if self.employee_finder.is_none() && self.can_open_employee_finder() {
            self.employee_finder = Some(State {
                query: String::new(),
                focus_query: true,
            });
        }
    }

    pub(super) fn employee_finder_button(&mut self, ui: &mut Ui) {
        if self.employee_finder_available()
            && ui
                .add_enabled(
                    self.can_open_employee_finder(),
                    egui::Button::new(RichText::new("Find employee").size(12.0)),
                )
                .on_hover_text("Ctrl+K")
                .on_disabled_hover_text("Finish the open edit or action first.")
                .clicked()
        {
            self.open_employee_finder();
        }
    }

    pub(super) fn employee_finder_shortcut(&mut self, ctx: &egui::Context) {
        if self.employee_finder.is_none()
            && self.can_open_employee_finder()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::K))
        {
            self.open_employee_finder();
        }
    }

    pub(super) fn show_employee_finder(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.employee_finder.take() else {
            return;
        };
        if !self.employee_finder_available() {
            clear_edit_history(ctx, &[QUERY_ID]);
            return;
        }
        let modal = finder_dialog(ctx, &self.data, &mut state, self.can_open_employee_finder());
        if modal.should_close() {
            clear_edit_history(ctx, &[QUERY_ID]);
            return;
        }
        if let Some(id) = modal.inner
            && self.can_open_employee_finder()
            && self.employee_in_finder_scope(id)
        {
            self.open_employee(id);
            self.employee_record_search = Some(state);
            clear_edit_history(ctx, &[QUERY_ID]);
            return;
        }
        self.employee_finder = Some(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finder_focuses_search_and_opens_one_match_with_enter() {
        let data = DemoStore::new().data;
        let ctx = egui::Context::default();
        let mut state = State {
            query: String::new(),
            focus_query: true,
        };
        let mut draw = |events, can_open| {
            let mut selection = None;
            let mut closed = false;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(800.0, 576.0),
                    )),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |_| {
                    let dialog = finder_dialog(&ctx, &data, &mut state, can_open);
                    selection = selection.or(dialog.inner);
                    closed |= dialog.should_close();
                },
            );
            output.textures_delta.clear();
            (selection, closed)
        };
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(draw(vec![], true), (None, false));
        assert_eq!(
            draw(vec![egui::Event::Text("ns-002".into())], true),
            (None, false)
        );
        assert_eq!(
            draw(vec![key(egui::Key::Enter)], true),
            (Some(data.employees[1].id), false)
        );
        assert_eq!(draw(vec![key(egui::Key::Escape)], true), (None, true));
    }

    #[test]
    fn employee_search_requires_the_current_team_and_company_scope() {
        let mut data = DemoStore::new().data;
        let report = data.employees[1].id;
        data.access.role = Role::Manager;
        data.access.employee_id = Some(data.employees[3].id);
        data.access.direct_reports = vec![report, data.employees[2].id];
        let results = employee_matches(&data, " ns-00 ");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|employee| employee.id == report));
        assert_eq!(employee_matches(&data, "JAMIE")[0].id, report);
        data.employees[1].company_id = Uuid::from_u128(80);
        assert!(employee_matches(&data, "Jamie").is_empty());
        assert!(employee_matches(&data, "Taylor").is_empty());
        assert!(employee_matches(&data, "   ").is_empty());
    }
}
