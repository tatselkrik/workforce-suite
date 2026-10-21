use super::*;
use crate::organization::{self, Chart, Filters, RootReason};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct State {
    pub filters: Filters,
    pub collapsed: HashSet<Uuid>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChartAction {
    Open(Uuid),
    Toggle(Uuid),
}

impl HrisApp {
    pub(super) fn organization_chart(&mut self, ui: &mut Ui) {
        let previous = self.organization.filters.clone();
        let readable = organization::readable(&self.data);
        let mut departments: Vec<_> = readable.iter().map(|e| e.department.clone()).collect();
        departments.sort_by_cached_key(|d| (d.to_lowercase(), d.clone()));
        departments.dedup();
        let locations: HashSet<_> = readable.iter().filter_map(|e| e.work_location_id).collect();
        ui.horizontal_wrapped(|ui| {
            ui.add_sized(
                [240.0, 36.0],
                egui::TextEdit::singleline(&mut self.organization.filters.search)
                    .hint_text("Search name, code or position…"),
            );
            egui::ComboBox::from_id_salt("chart_department")
                .width(170.0)
                .truncate()
                .selected_text(
                    self.organization
                        .filters
                        .department
                        .as_deref()
                        .map(|d| {
                            if d.is_empty() {
                                "Department not set"
                            } else {
                                d
                            }
                        })
                        .unwrap_or("All departments"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.organization.filters.department,
                        None,
                        "All departments",
                    );
                    for department in &departments {
                        ui.selectable_value(
                            &mut self.organization.filters.department,
                            Some(department.clone()),
                            if department.is_empty() {
                                "Department not set"
                            } else {
                                department
                            },
                        );
                    }
                });
            let current = match self.organization.filters.location {
                EmploymentFilter::All => "All locations".into(),
                EmploymentFilter::NotSet => "Location not set".into(),
                EmploymentFilter::Choice(id) => Chart::location_label(&self.data, Some(id)),
            };
            egui::ComboBox::from_id_salt("chart_location")
                .width(170.0)
                .truncate()
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.organization.filters.location,
                        EmploymentFilter::All,
                        "All locations",
                    );
                    ui.selectable_value(
                        &mut self.organization.filters.location,
                        EmploymentFilter::NotSet,
                        "Location not set",
                    );
                    for option in self.data.work_locations.iter().filter(|c| {
                        c.company_id == self.data.company.id && locations.contains(&c.id)
                    }) {
                        ui.selectable_value(
                            &mut self.organization.filters.location,
                            EmploymentFilter::Choice(option.id),
                            Chart::location_label(&self.data, Some(option.id)),
                        );
                    }
                });
        });
        drop(readable);
        ui.horizontal_wrapped(|ui| {
            if self.data.access.role.manages_hr() {
                ui.checkbox(
                    &mut self.organization.filters.include_archived,
                    "Include archived",
                );
            }
            if ui.button("Clear filters").clicked() {
                self.organization = State::default();
            }
            if ui.button("Expand all").clicked() {
                self.organization.collapsed.clear();
            }
            if ui.button("Collapse all").clicked() {
                self.organization.collapsed = Chart::build(&self.data, &self.organization.filters)
                    .nodes
                    .iter()
                    .filter(|n| !n.children.is_empty())
                    .map(|n| n.employee.id)
                    .collect();
            }
        });
        if previous != self.organization.filters {
            self.organization.collapsed.clear();
        }
        let chart = Chart::build(&self.data, &self.organization.filters);
        ui.label(
            RichText::new(format!(
                "{} / {} employees match | {} in reporting context",
                chart.matching_count, chart.eligible_count, chart.context_count
            ))
            .color(muted(ui))
            .small(),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(if self.data.access.role == Role::Manager {
                "Your team: your record and active direct reports only."
            } else {
                "Managers above matching employees stay visible as reporting context."
            })
            .color(muted(ui))
            .small(),
        );
        ui.label(
            RichText::new("Select a card to open it. Edit Reports to in Employment.")
                .color(muted(ui))
                .small(),
        );
        if chart.invalid_cycles > 0 {
            ui.label(RichText::new("Some reporting relationships form a loop and need HR review. They are shown separately; no saved relationship was changed.").color(Color32::from_rgb(180, 90, 30)));
        }
        ui.add_space(10.0);
        if chart.nodes.is_empty() {
            card(ui, |ui| {
                empty(
                    ui,
                    if chart.eligible_count == 0 {
                        "No employees are available in this view."
                    } else {
                        "No employees match these filters. Clear filters to see the organization."
                    },
                )
            });
            return;
        }
        if let Some(action) = show_rows(ui, &chart, &self.organization.collapsed, &self.data) {
            match action {
                ChartAction::Open(id) => self.open_employee(id),
                ChartAction::Toggle(id) => {
                    if !self.organization.collapsed.remove(&id) {
                        self.organization.collapsed.insert(id);
                    }
                }
            }
        }
    }
}

fn show_rows(
    ui: &mut Ui,
    chart: &Chart,
    collapsed: &HashSet<Uuid>,
    data: &Snapshot,
) -> Option<ChartAction> {
    let mut action = None;
    for row in chart.rows(collapsed) {
        let node = &chart.nodes[row.index];
        ui.push_id(node.employee.id, |ui| {
            let response = ui
                .horizontal_top(|ui| {
                    let indent = row.depth.min(5) as f32 * 18.0;
                    // Reserve the same gutter for branches and leaves. Button minimum
                    // widths otherwise make peers drift when only one has children.
                    let (gutter, _) = ui
                        .allocate_exact_size(Vec2::new(indent + 32.0, 28.0), egui::Sense::hover());
                    if !node.children.is_empty() {
                        let expanded = !collapsed.contains(&node.employee.id);
                        let button = ui
                            .place(
                                egui::Rect::from_min_size(
                                    gutter.min + Vec2::new(indent, 0.0),
                                    Vec2::new(28.0, 28.0),
                                ),
                                egui::Button::new(if expanded { "v" } else { ">" }),
                            )
                            .on_hover_text(if expanded {
                                "Collapse team"
                            } else {
                                "Expand team"
                            });
                        if button.clicked() {
                            action = Some(ChartAction::Toggle(node.employee.id));
                        }
                    }
                    let response = node_card(ui, node, row.depth, data);
                    if response.clicked() {
                        action = Some(ChartAction::Open(node.employee.id));
                    }
                    if row.depth > 0 {
                        let rect = response.rect;
                        let x = rect.left() - 12.0;
                        let stroke =
                            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
                        ui.painter().line_segment(
                            [
                                egui::pos2(x, rect.top() - 8.0),
                                egui::pos2(x, rect.center().y),
                            ],
                            stroke,
                        );
                        ui.painter().line_segment(
                            [
                                egui::pos2(x, rect.center().y),
                                egui::pos2(rect.left(), rect.center().y),
                            ],
                            stroke,
                        );
                    }
                })
                .response;
            #[cfg(feature = "qa-screenshot")]
            if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("chart-bottom") {
                response.scroll_to_me(Some(Align::Max));
            }
            #[cfg(feature = "qa-screenshot")]
            if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("chart-root")
                && node.employee.id.as_u128() == 100
            {
                response.scroll_to_me(Some(Align::Min));
            }
            #[cfg(not(feature = "qa-screenshot"))]
            let _ = response;
        });
        ui.add_space(8.0);
    }
    action
}

fn node_card(
    ui: &mut Ui,
    node: &organization::Node,
    depth: usize,
    data: &Snapshot,
) -> egui::Response {
    let width = ui.available_width();
    let frame = Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .corner_radius(8)
        .inner_margin(12);
    let inner_width = (width - frame.total_margin().sum().x).max(1.0);
    let response = frame
        .show(ui, |ui| {
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.set_width(inner_width);
                ui.add(
                    egui::Label::new(RichText::new(&node.employee.full_name).strong().size(16.0))
                        .wrap(),
                );
                ui.add(
                    egui::Label::new(if node.employee.job_title.is_empty() {
                        "Position not set"
                    } else {
                        &node.employee.job_title
                    })
                    .wrap(),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{} · {}",
                            if node.employee.department.is_empty() {
                                "Department not set"
                            } else {
                                &node.employee.department
                            },
                            Chart::location_label(data, node.employee.work_location_id)
                        ))
                        .color(muted(ui))
                        .small(),
                    )
                    .wrap(),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(&node.employee.employee_code)
                            .color(muted(ui))
                            .small(),
                    );
                    if !node.matched {
                        badge(ui, "Reporting context", true);
                    }
                    if node.employee.status == EmployeeStatus::Archived {
                        badge(ui, "Archived", false);
                    }
                    if depth > 5 {
                        ui.label(
                            RichText::new(format!("Level {}", depth + 1))
                                .color(muted(ui))
                                .small(),
                        );
                    }
                    if let Some(reason) = node.root_reason {
                        ui.label(
                            RichText::new(match reason {
                                RootReason::NoManager => "No assigned manager",
                                RootReason::ManagerOutsideView => "Manager outside this view",
                                RootReason::InvalidCycle => "Reporting needs review",
                            })
                            .color(muted(ui))
                            .small(),
                        );
                    }
                    if !node.children.is_empty() {
                        ui.label(
                            RichText::new(format!(
                                "{} direct reports in view",
                                node.children.len()
                            ))
                            .color(muted(ui))
                            .small(),
                        );
                    }
                });
            });
        })
        .response
        .interact(egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            format!("Open employee {}", node.employee.full_name),
        )
    });
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Open employee record")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_and_leaf_peers_share_an_exact_card_origin() {
        for scale in [1.0, 1.25] {
            let mut data = DemoStore::new().data;
            data.employees.truncate(4);
            let root = data.employees[0].id;
            let branch = data.employees[1].id;
            for (i, e) in data.employees.iter_mut().enumerate() {
                e.full_name = format!("Alignment {i}");
                e.reports_to = match i {
                    0 => None,
                    1 | 2 => Some(root),
                    _ => Some(branch),
                };
            }
            let chart = Chart::build(&data, &Filters::default());
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            ctx.set_zoom_factor(scale);
            let mut positions = std::collections::HashMap::new();
            for _ in 0..3 {
                let output = frame(&ctx, raw(700.0, Vec::new()), |ui| {
                    show_rows(ui, &chart, &HashSet::new(), &data);
                });
                positions.clear();
                for shape in output.shapes {
                    if let egui::epaint::Shape::Text(text) = shape.shape {
                        if text.galley.job.text.starts_with("Alignment ") {
                            positions.insert(text.galley.job.text.clone(), text.pos.x);
                        }
                    }
                }
            }
            assert!(
                (positions["Alignment 1"] - positions["Alignment 2"]).abs() < 0.5,
                "Branches and leaves at the same depth must align"
            );
            assert!(
                (positions["Alignment 1"] - positions["Alignment 0"] - 18.0).abs() < 0.5,
                "Each depth must use one consistent indentation step"
            );
        }
    }

    fn frame(
        context: &egui::Context,
        input: egui::RawInput,
        run: impl FnMut(&mut Ui),
    ) -> egui::FullOutput {
        let mut output = context.run_ui(input, run);
        output.textures_delta.clear(); // Headless input/paint test; no renderer owns these deltas.
        output
    }

    fn raw(width: f32, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 2000.0),
            )),
            events,
            ..Default::default()
        }
    }

    #[test]
    fn long_chart_cards_wrap_and_reserve_height_at_normal_and_125_percent() {
        for scale in [1.0, 1.25] {
            let mut data = DemoStore::new().data;
            for e in &mut data.employees {
                e.full_name =
                    "Synthetic employee with a long name for the regional operations department"
                        .into();
                e.job_title =
                    "Regional coordinator of employee services and administrative operations"
                        .into();
                e.department = "Regional employee services and business support operations".into();
            }
            let chart = Chart::build(&data, &Filters::default());
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            ctx.set_zoom_factor(scale);
            let mut output = None;
            let mut rectangles = Vec::new();
            for _ in 0..3 {
                rectangles.clear();
                output = Some(frame(&ctx, raw(430.0, Vec::new()), |ui| {
                    for node in chart.nodes.iter().take(2) {
                        ui.horizontal_top(|ui| {
                            rectangles.push(node_card(ui, node, 30, &data).rect);
                        });
                        ui.add_space(8.0);
                    }
                }));
            }
            assert!(rectangles[0].bottom() + 6.0 <= rectangles[1].top());
            let output = output.unwrap();
            let mut names = 0;
            let mut name_bounds = Vec::new();
            let mut job_bounds = Vec::new();
            for shape in &output.shapes {
                if let egui::epaint::Shape::Text(text) = &shape.shape {
                    let bounds = text.galley.rect.translate(text.pos.to_vec2());
                    assert!(
                        rectangles
                            .iter()
                            .any(|r| r.expand(1.0).contains_rect(bounds)),
                        "Text outside its card: {}",
                        text.galley.job.text
                    );
                    if text.galley.job.text.starts_with("Synthetic employee") {
                        names += 1;
                        name_bounds.push(bounds);
                        assert!(text.galley.rows.len() > 1);
                    }
                    if text.galley.job.text.starts_with("Regional coordinator") {
                        job_bounds.push(bounds);
                    }
                }
            }
            assert_eq!(names, 2);
            assert_eq!(job_bounds.len(), 2);
            for (name, job) in name_bounds.iter().zip(job_bounds) {
                assert!(
                    job.top() >= name.bottom(),
                    "Job title must follow the full name vertically"
                );
            }
        }
    }

    #[test]
    fn clicking_a_card_opens_that_employee_and_the_expander_only_toggles_the_team() {
        let mut data = DemoStore::new().data;
        data.employees
            .retain(|e| matches!(e.id.as_u128(), 101 | 103));
        let chart = Chart::build(&data, &Filters::default());
        let collapsed = HashSet::new();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut card = egui::Rect::NOTHING;
        for _ in 0..3 {
            frame(&ctx, raw(600.0, Vec::new()), |ui| {
                card = node_card(ui, &chart.nodes[0], 0, &data).rect;
            });
        }
        let click = |position: egui::Pos2, pressed| {
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let mut action = None;
        for pressed in [true, false] {
            frame(&ctx, raw(600.0, click(card.center(), pressed)), |ui| {
                let response = node_card(ui, &chart.nodes[0], 0, &data);
                if response.clicked() {
                    action = Some(ChartAction::Open(chart.nodes[0].employee.id));
                }
            });
        }
        assert_eq!(action, Some(ChartAction::Open(chart.nodes[0].employee.id)));
        // The visible root is Morgan (103); its independent expander is left of the card.
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        for _ in 0..3 {
            frame(&ctx, raw(600.0, Vec::new()), |ui| {
                show_rows(ui, &chart, &collapsed, &data);
            });
        }
        action = None;
        for pressed in [true, false] {
            frame(
                &ctx,
                raw(600.0, click(egui::pos2(12.0, 14.0), pressed)),
                |ui| {
                    action = show_rows(ui, &chart, &collapsed, &data).or(action);
                },
            );
        }
        assert_eq!(action, Some(ChartAction::Toggle(Uuid::from_u128(103))));
    }

    #[test]
    fn deep_filtered_chart_rows_cannot_expand_the_page_horizontally() {
        for scale in [1.0, 1.25] {
            let mut data = DemoStore::new().data;
            let template = data.employees[0].clone();
            data.employees = (5000..5030)
                .map(|id| {
                    let mut e = template.clone();
                    e.id = Uuid::from_u128(id);
                    e.employee_code = format!("QA-{id}");
                    e.reports_to = (id > 5000).then_some(Uuid::from_u128(id - 1));
                    e
                })
                .collect();
            let chart = Chart::build(
                &data,
                &Filters {
                    search: "QA-5029".into(),
                    ..Default::default()
                },
            );
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            ctx.set_zoom_factor(scale);
            for _ in 0..3 {
                frame(&ctx, raw(430.0, Vec::new()), |ui| {
                    let right = ui.available_rect_before_wrap().right();
                    let response = ui
                        .scope(|ui| {
                            show_rows(ui, &chart, &HashSet::new(), &data);
                        })
                        .response;
                    assert!(
                        response.rect.right() <= right + 0.5,
                        "Chart expanded beyond its viewport: {} > {right}",
                        response.rect.right()
                    );
                });
            }
        }
    }
}
