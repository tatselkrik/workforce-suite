use super::*;
use crate::reports::{self, Filter};
pub(super) fn chooser(ui: &mut Ui, filter: &mut Filter) {
    let available = reports::available_columns(filter.kind);
    let count = filter.columns.as_ref().map_or(available.len(), Vec::len);
    let header =
        egui::CollapsingHeader::new(format!("Columns · {count} of {} selected", available.len()))
            .id_salt(("report-columns", filter.kind.slug()));
    #[cfg(feature = "qa-screenshot")]
    let header = header.default_open(
        std::env::var("HRIS_PREVIEW_FORM")
            .unwrap_or_default()
            .contains("columns"),
    );
    let shown=header.show(ui,|ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button("All columns").clicked(){filter.columns=None;}
            if ui.button("Clear selection").clicked(){filter.columns=Some(Vec::new());}
            ui.label(RichText::new("Selected fields appear in this order in the table and CSV.").small().color(muted(ui)));
        });
        let mut selected=filter.columns.clone().unwrap_or_else(||available.iter().map(|s|(*s).to_owned()).collect());
        let order:Vec<_>=selected.iter().cloned().chain(available.iter().filter(|column|!selected.iter().any(|s|s==**column)).map(|s|(*s).to_owned())).collect();
        let mut changed=false;
        egui::ScrollArea::vertical().id_salt(("column-choices",filter.kind.slug())).max_height(230.0).min_scrolled_height(230.0).auto_shrink([false,true]).show(ui,|ui| {
            for column in order {
                ui.push_id(&column,|ui| {
                    ui.horizontal(|ui| {
                        let index=selected.iter().position(|s|s==&column);
                        let mut checked=index.is_some();
                        if ui.checkbox(&mut checked,&column).changed() {
                            if checked{selected.push(column.clone());}else{selected.retain(|s|s!=&column);}
                            changed=true;
                        }
                        if let Some(index)=selected.iter().position(|s|s==&column) {
                            if ui.add_enabled(index>0,egui::Button::new("Up").small()).clicked(){selected.swap(index,index-1);changed=true;}
                            if ui.add_enabled(index+1<selected.len(),egui::Button::new("Down").small()).clicked(){selected.swap(index,index+1);changed=true;}
                        }
                    });
                });
            }
        });
        if changed{filter.columns=Some(selected);}
        ui.label(RichText::new("Save a view to keep this column order. Export always includes company, report, period and generation metadata. Workforce charts use the full scoped report.").small().color(muted(ui)));
    });
    #[cfg(feature = "qa-screenshot")]
    if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("columns") {
        shown.header_response.scroll_to_me(Some(Align::Min));
    }
    let _ = shown;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paint(
        ctx: &egui::Context,
        filter: &mut Filter,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut frame = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(700.0, 1100.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                chooser(ui, filter);
            },
        );
        frame.textures_delta.clear();
        frame
    }
    fn position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.galley.rect.translate(t.pos.to_vec2()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing control: {label}"))
    }
    fn click(ctx: &egui::Context, filter: &mut Filter, pos: egui::Pos2) {
        for pressed in [true, false] {
            paint(
                ctx,
                filter,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    fn open(ctx: &egui::Context, filter: &mut Filter) -> egui::FullOutput {
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            ctx.style_mut_of(theme, |style| style.animation_time = 0.0);
        }
        let mut frame = paint(ctx, filter, vec![]);
        for _ in 0..3 {
            frame = paint(ctx, filter, vec![]);
        }
        let count = reports::available_columns(filter.kind).len();
        let header = format!("Columns · {count} of {count} selected");
        click(ctx, filter, position(&frame, &header));
        paint(ctx, filter, vec![])
    }
    #[test]
    fn choosing_and_reordering_columns_changes_the_actual_report_filter() {
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut f = Filter::new(chrono::Utc::now().date_naive());
        let output = open(&ctx, &mut f);
        click(&ctx, &mut f, position(&output, "Employee code"));
        let columns = f.columns.as_ref().unwrap();
        assert!(!columns.iter().any(|c| c == "Employee code"));
        assert_eq!(columns[0], "Full name");
        let output = paint(&ctx, &mut f, vec![]);
        click(&ctx, &mut f, position(&output, "Down"));
        assert_eq!(f.columns.as_ref().unwrap()[0], "Department (current)");
        assert_eq!(f.columns.as_ref().unwrap()[1], "Full name");
    }
    #[test]
    fn clearing_columns_blocks_generation_and_all_columns_restores_default() {
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut f = Filter::new(chrono::Utc::now().date_naive());
        let output = open(&ctx, &mut f);
        click(&ctx, &mut f, position(&output, "Clear selection"));
        assert!(f.validate().is_err());
        let output = paint(&ctx, &mut f, vec![]);
        click(&ctx, &mut f, position(&output, "All columns"));
        assert!(f.columns.is_none());
        f.validate().unwrap();
    }
}
