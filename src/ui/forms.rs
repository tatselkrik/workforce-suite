use super::*;

fn date_error(value: &str, required: bool) -> Option<&'static str> {
    if value.trim().is_empty() {
        required.then_some("Choose a date or enter YYYY-MM-DD.")
    } else {
        parse_date(value)
            .is_err()
            .then_some("Enter a real date as YYYY-MM-DD, with a year from 1900 to 2200.")
    }
}

pub(super) fn error(ui: &mut Ui, message: impl Into<String>) {
    ui.add(
        egui::Label::new(
            RichText::new(message.into())
                .small()
                .color(ui.visuals().error_fg_color),
        )
        .wrap(),
    );
}

pub(super) fn required_text(ui: &mut Ui, label: &str, value: &mut String, hint: &str) {
    field(
        ui,
        &format!("{} *", label.trim_end_matches(" *")),
        value,
        hint,
    );
    if value.trim().is_empty() {
        ui.label(RichText::new("Required.").small().color(muted(ui)));
    }
}

/// Calendar assistance edits the same draft string as typing. Opening or
/// dismissing a calendar must never create a value for an optional blank.
pub(super) fn date(ui: &mut Ui, label: &str, value: &mut String, required: bool, today: NaiveDate) {
    date_with_id(
        ui,
        label,
        value,
        required,
        today,
        ui.make_persistent_id(("form-date", label)),
    );
}

pub(super) fn date_with_id(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    required: bool,
    today: NaiveDate,
    input_id: egui::Id,
) {
    ui.push_id(input_id, |ui| {
        let label = label.trim_end_matches(" *").trim_end_matches(" (optional)");
        ui.add(
            egui::Label::new(
                RichText::new(format!(
                    "{label} {}",
                    if required { "*" } else { "(optional)" },
                ))
                .size(13.0),
            )
            .wrap(),
        );
        let month_id = ui.make_persistent_id("calendar-month");
        let width = ui.available_width().min(620.0);
        let inline = width >= 270.0;
        let button_width = |text: &str| {
            let text_width = ui
                .painter()
                .layout_no_wrap(
                    text.into(),
                    egui::TextStyle::Button.resolve(ui.style()),
                    Color32::WHITE,
                )
                .size()
                .x;
            (text_width + 2.0 * ui.spacing().button_padding.x).max(ui.spacing().interact_size.x)
        };
        let buttons_width = button_width("Calendar")
            + ui.spacing().item_spacing.x
            + if required {
                0.0
            } else {
                button_width("Clear") + ui.spacing().item_spacing.x
            }
            + 10.0;
        let mut calendar = None;
        let mut clear = false;
        ui.horizontal(|ui| {
            let input_width = if inline { width - buttons_width } else { width };
            let edit = ui.add_sized(
                [input_width, 34.0],
                egui::TextEdit::singleline(value)
                    .hint_text("YYYY-MM-DD")
                    .id(input_id),
            );
            if edit.changed() {
                if let Ok(parsed) = parse_date(value) {
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(month_id, month_start(parsed)));
                }
            }
            if inline {
                calendar = Some(ui.button("Calendar").on_hover_text("Choose a date"));
                if !required {
                    clear = ui
                        .add_enabled(!value.is_empty(), egui::Button::new("Clear"))
                        .on_hover_text("Leave this optional date blank")
                        .clicked();
                }
            }
        });
        if !inline {
            ui.horizontal_wrapped(|ui| {
                calendar = Some(ui.button("Calendar").on_hover_text("Choose a date"));
                if !required {
                    clear = ui
                        .add_enabled(!value.is_empty(), egui::Button::new("Clear"))
                        .on_hover_text("Leave this optional date blank")
                        .clicked();
                }
            });
        }
        if clear {
            value.clear();
            ui.ctx().data_mut(|data| data.remove::<NaiveDate>(month_id));
        }
        let calendar = calendar.expect("Every date field has a calendar button");
        if calendar.clicked() {
            let selected = parse_date(value).unwrap_or(today);
            ui.ctx()
                .data_mut(|data| data.insert_temp(month_id, month_start(selected)));
        }
        if !ui.is_enabled() {
            egui::Popup::close_id(ui.ctx(), egui::Popup::default_response_id(&calendar));
        } else {
            egui::Popup::from_toggle_button_response(&calendar).show(|ui| {
                let mut selected = parse_date(value).unwrap_or(today);
                let mut month = ui.ctx().data_mut(|data| {
                    data.get_temp::<NaiveDate>(month_id)
                        .unwrap_or_else(|| month_start(selected))
                });
                if date_calendar(
                    ui,
                    &mut selected,
                    &mut month,
                    today,
                    NaiveDate::from_ymd_opt(2200, 12, 31).unwrap(),
                    None,
                ) {
                    *value = selected.to_string();
                    ui.close();
                }
                ui.ctx().data_mut(|data| data.insert_temp(month_id, month));
                if ui
                    .button("Today")
                    .on_hover_text("Use today's company date")
                    .clicked()
                {
                    *value = today.to_string();
                    ui.close();
                }
            });
        }
        if let Some(message) = date_error(value, required) {
            if value.trim().is_empty() {
                ui.add(egui::Label::new(RichText::new(message).small().color(muted(ui))).wrap());
            } else {
                error(ui, message);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(shape: &egui::epaint::Shape, text: &str) -> Option<egui::Pos2> {
        match shape {
            egui::epaint::Shape::Text(s) if s.galley.job.text == text => {
                Some(s.pos + s.galley.size() * 0.5)
            }
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|s| position(s, text)),
            _ => None,
        }
    }

    fn frame(
        ctx: &egui::Context,
        value: &mut String,
        required: bool,
        events: Vec<egui::Event>,
        enabled: bool,
    ) -> egui::FullOutput {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(500.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    date(
                        ui,
                        "Due date",
                        value,
                        required,
                        NaiveDate::from_ymd_opt(2028, 2, 20).unwrap(),
                    );
                });
            },
        )
    }

    fn click(ctx: &egui::Context, value: &mut String, required: bool, text: &str, enabled: bool) {
        let mut output = frame(ctx, value, required, vec![], enabled);
        output.textures_delta.clear();
        let mut output = frame(ctx, value, required, vec![], enabled);
        let pos = output
            .shapes
            .iter()
            .find_map(|s| position(&s.shape, text))
            .unwrap_or_else(|| panic!("Missing control: {text}"));
        output.textures_delta.clear();
        for pressed in [true, false] {
            let mut output = frame(
                ctx,
                value,
                required,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                enabled,
            );
            output.textures_delta.clear();
        }
    }

    #[test]
    fn optional_blanks_and_invalid_typed_values_are_preserved_until_a_choice() {
        let ctx = egui::Context::default();
        let mut value = String::new();
        click(&ctx, &mut value, false, "Calendar", true);
        assert!(value.is_empty());
        click(&ctx, &mut value, false, "29", true);
        assert_eq!(value, "2028-02-29");
        click(&ctx, &mut value, false, "Clear", true);
        assert!(value.is_empty());
        value = "2027-02-29".into();
        frame(&ctx, &mut value, false, vec![], true)
            .textures_delta
            .clear();
        assert_eq!(value, "2027-02-29");
        assert!(date_error(&value, false).is_some());
    }

    #[test]
    fn today_uses_the_supplied_company_date_and_disabled_controls_do_not_change_drafts() {
        let ctx = egui::Context::default();
        let mut value = "2030-12-12".into();
        click(&ctx, &mut value, false, "Calendar", false);
        click(&ctx, &mut value, false, "Clear", false);
        assert_eq!(value, "2030-12-12");
        click(&ctx, &mut value, false, "Calendar", true);
        click(&ctx, &mut value, false, "Today", true);
        assert_eq!(value, "2028-02-20");
    }

    #[test]
    fn calendar_supports_future_dates_and_required_inputs_have_no_clear_action() {
        let ctx = egui::Context::default();
        let mut value = "2031-12-12".into();
        click(&ctx, &mut value, true, "Calendar", true);
        click(&ctx, &mut value, true, "31", true);
        assert_eq!(value, "2031-12-31");
        let mut output = frame(&ctx, &mut value, true, vec![], true);
        assert!(
            !output
                .shapes
                .iter()
                .any(|s| position(&s.shape, "Clear").is_some())
        );
        output.textures_delta.clear();
        assert!(date_error("", true).is_some());
        assert!(date_error("", false).is_none());
        assert!(date_error("2028-02-29", true).is_none());
        assert!(date_error("2028-2-29", false).is_some());
        assert!(date_error("2201-01-01", false).is_some());
    }

    #[test]
    fn dismissing_the_calendar_preserves_an_invalid_draft_without_filling_it() {
        let ctx = egui::Context::default();
        let mut value = "not yet a date".into();
        click(&ctx, &mut value, false, "Calendar", true);
        frame(
            &ctx,
            &mut value,
            false,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            true,
        )
        .textures_delta
        .clear();
        assert_eq!(value, "not yet a date");
        let mut output = frame(&ctx, &mut value, false, vec![], true);
        assert!(
            !output
                .shapes
                .iter()
                .any(|s| position(&s.shape, "Today").is_some())
        );
        output.textures_delta.clear();
    }

    #[test]
    fn making_an_open_form_readonly_closes_its_calendar_and_preserves_the_draft() {
        let ctx = egui::Context::default();
        let mut value = "2031-12-12".into();
        click(&ctx, &mut value, false, "Calendar", true);
        for enabled in [false, true] {
            let mut output = frame(&ctx, &mut value, false, vec![], enabled);
            assert!(
                !output
                    .shapes
                    .iter()
                    .any(|s| position(&s.shape, "Today").is_some())
            );
            output.textures_delta.clear();
            assert_eq!(value, "2031-12-12");
        }
    }

    #[test]
    fn date_fields_in_separate_scopes_have_independent_calendars_and_values() {
        let ctx = egui::Context::default();
        let today = NaiveDate::from_ymd_opt(2028, 2, 20).unwrap();
        let mut values = ["2028-02-29".to_owned(), "2030-12-01".to_owned()];
        let mut frame = |events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(600.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    for (i, value) in values.iter_mut().enumerate() {
                        ui.push_id(i, |ui| date(ui, "Date", value, false, today));
                    }
                },
            )
        };
        frame(vec![]).textures_delta.clear();
        let mut output = frame(vec![]);
        let button = output
            .shapes
            .iter()
            .filter_map(|s| position(&s.shape, "Calendar"))
            .nth(1)
            .unwrap();
        output.textures_delta.clear();
        for text in [None, Some("31")] {
            let pos = if let Some(text) = text {
                let mut output = frame(vec![]);
                let pos = output
                    .shapes
                    .iter()
                    .find_map(|s| position(&s.shape, text))
                    .unwrap();
                output.textures_delta.clear();
                pos
            } else {
                button
            };
            for pressed in [true, false] {
                frame(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ])
                .textures_delta
                .clear();
            }
        }
        drop(frame);
        assert_eq!(values, ["2028-02-29", "2030-12-31"]);
    }

    #[test]
    fn date_fields_fit_narrow_forms_with_required_optional_and_error_messages() {
        for width in [160.0, 270.0, 480.0] {
            for required in [false, true] {
                for font_scale in [1.0, 1.25] {
                    let ctx = egui::Context::default();
                    configure_theme(&ctx);
                    ctx.all_styles_mut(|style| {
                        for font in style.text_styles.values_mut() {
                            font.size *= font_scale;
                        }
                    });
                    let mut value = "not a date".into();
                    for _ in 0..2 {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    Vec2::new(width, 600.0),
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                date(
                                    ui,
                                    "Coverage from",
                                    &mut value,
                                    required,
                                    NaiveDate::from_ymd_opt(2028, 2, 20).unwrap(),
                                );
                                assert!(
                                    ui.min_rect().right() <= width + 1.0,
                                    "Date overflow {width}, {required}, {font_scale}"
                                );
                            },
                        );
                        output.textures_delta.clear();
                    }
                }
            }
        }
    }
}
