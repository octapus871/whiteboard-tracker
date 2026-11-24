use super::*;

impl WhiteboardApp {
    pub(super) fn render_bottom_bar(&mut self, ui: &mut Ui) {
        let lang = self.active_lang();
        let s = tr(lang);
        let counts = self.data.current_day.calculate_counts();
        let rate = counts.completion_rate();

        // Continuous 2px progress hairline spanning the full window width:
        // fills right-to-left in Arabic (RTL) and left-to-right in English (LTR).
        let rect = ui.max_rect();
        let width = rect.width();
        let done = (rate / 100.0).clamp(0.0, 1.0);
        if lang.is_rtl() {
            let split_x = rect.right() - width * done;
            ui.painter().rect_filled(
                Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(split_x, rect.top() + 2.0)),
                CornerRadius::same(0),
                theme::GRID,
            );
            ui.painter().rect_filled(
                Rect::from_min_max(
                    pos2(split_x, rect.top()),
                    pos2(rect.right(), rect.top() + 2.0),
                ),
                CornerRadius::same(0),
                theme::GREEN_BAR,
            );
        } else {
            let split_x = rect.left() + width * done;
            ui.painter().rect_filled(
                Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(split_x, rect.top() + 2.0)),
                CornerRadius::same(0),
                theme::GREEN_BAR,
            );
            ui.painter().rect_filled(
                Rect::from_min_max(
                    pos2(split_x, rect.top()),
                    pos2(rect.right(), rect.top() + 2.0),
                ),
                CornerRadius::same(0),
                theme::GRID,
            );
        }

        ui.add_space(2.0);

        // Borderless localized metrics separated by dots.
        ui.horizontal_centered(|ui| {
            ui.add_space(24.0);
            metric(ui, counts.completed_count, theme::GREEN_SHADOW, s.status_done, lang);
            ui.label(mono("\u{2022}", 12.5).color(theme::DIVIDER));
            metric(ui, counts.active_count, theme::AMBER_SHADOW, s.status_active, lang);
            ui.label(mono("\u{2022}", 12.5).color(theme::DIVIDER));
            metric(ui, counts.cancelled_count, theme::RED_SHADOW, s.status_cancelled, lang);
            ui.label(mono("\u{2022}", 12.5).color(theme::DIVIDER));
            metric(ui, counts.total, theme::INK, s.status_total, lang);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(16.0);
                // Subtle `?` help button (>= 28x28px hitbox) to reopen Quick Guide anytime.
                let (help_slot, _) =
                    ui.allocate_exact_size(vec2(MIN_HITBOX_SIZE, 26.0), Sense::hover());
                let help_hit = Rect::from_center_size(
                    help_slot.center(),
                    vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                );
                let help_resp = ui
                    .interact(help_hit, ui.id().with("bottom_help_btn"), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text(disp(s.guide_title));
                let help_color = if help_resp.hovered() || help_resp.has_focus() {
                    theme::INK
                } else {
                    theme::SLATE
                };
                if help_resp.hovered() {
                    let pill = Rect::from_center_size(help_slot.center(), vec2(22.0, 22.0));
                    ui.painter()
                        .rect_filled(pill, CornerRadius::same(0), theme::SEL_FILL);
                }
                if help_resp.has_focus() {
                    draw_focus_ring(ui.painter(), help_hit);
                }
                ui.painter().text(
                    help_slot.center(),
                    egui::Align2::CENTER_CENTER,
                    "?",
                    FontId::monospace(13.0),
                    help_color,
                );
                if help_resp.clicked() {
                    self.show_onboarding_modal = true;
                }

                ui.add_space(4.0);
                let io_label = disp(s.data_io_btn);
                let io_font = if lang.is_rtl() {
                    FontId::proportional(12.0)
                } else {
                    FontId::monospace(11.5)
                };
                let io_text_w = ui
                    .fonts_mut(|f| {
                        f.layout_no_wrap(io_label.clone(), io_font.clone(), Color32::TRANSPARENT)
                    })
                    .size()
                    .x;
                let (io_slot, _) =
                    ui.allocate_exact_size(vec2((io_text_w + 12.0).max(56.0), 26.0), Sense::hover());
                let io_hit = Rect::from_center_size(
                    io_slot.center(),
                    vec2(io_slot.width(), MIN_HITBOX_SIZE),
                );
                let io_resp = ui
                    .interact(io_hit, ui.id().with("bottom_data_io_btn"), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text(disp(s.data_io_title));
                let io_pill = Rect::from_center_size(io_slot.center(), vec2(io_slot.width(), 20.0));
                if self.show_data_io_modal || io_resp.hovered() {
                    ui.painter()
                        .rect_filled(io_pill, CornerRadius::same(0), theme::SEL_FILL);
                    ui.painter().rect_stroke(
                        io_pill,
                        CornerRadius::same(0),
                        Stroke::new(1.0, theme::INK),
                        StrokeKind::Inside,
                    );
                }
                if io_resp.has_focus() {
                    draw_focus_ring(ui.painter(), io_hit);
                }
                ui.painter().text(
                    io_slot.center(),
                    egui::Align2::CENTER_CENTER,
                    &io_label,
                    io_font,
                    if self.show_data_io_modal || io_resp.hovered() || io_resp.has_focus() {
                        theme::INK
                    } else {
                        theme::SLATE
                    },
                );
                if io_resp.clicked() {
                    self.show_data_io_modal = true;
                    self.data_io_error = None;
                }

                ui.add_space(6.0);
                if let Some(err) = self.save_error.as_deref() {
                    let preview: String = if err.chars().count() > 56 {
                        let truncated: String = err.chars().take(53).collect();
                        format!("{}...", truncated)
                    } else {
                        err.to_string()
                    };
                    let err_text = format!("{}: {}", s.status_save_error, preview);
                    let resp = if lang.is_rtl() {
                        ui.label(
                            RichText::new(disp(&err_text))
                                .size(12.5)
                                .strong()
                                .color(theme::RED_BAR),
                        )
                    } else {
                        ui.label(mono(&err_text, 12.5).strong().color(theme::RED_BAR))
                    };
                    resp.on_hover_text(err);
                } else if self.dirty {
                    if lang.is_rtl() {
                        ui.label(
                            RichText::new(disp(s.status_saving))
                                .size(12.5)
                                .strong()
                                .color(theme::AMBER_INK),
                        );
                    } else {
                        ui.label(mono(s.status_saving, 12.5).strong().color(theme::AMBER_INK));
                    }
                } else if lang.is_rtl() {
                    ui.label(
                        RichText::new(disp(s.status_auto_saved))
                            .size(12.5)
                            .color(theme::SLATE),
                    );
                } else {
                    ui.label(mono(s.status_auto_saved, 12.5).color(theme::SLATE));
                }
                if let Some((toast_msg, _)) = &self.undo_toast {
                    ui.label(mono("\u{2022}", 12.5).color(theme::DIVIDER));
                    ui.label(
                        RichText::new(disp(toast_msg))
                            .size(12.5)
                            .strong()
                            .color(theme::INK),
                    );
                }
            });
        });
    }
}

/// A borderless localized metric: `2 DONE` / `2 مكتمل`.
pub(super) fn metric(ui: &mut Ui, value: usize, color: Color32, label: &str, lang: AppLang) {
    ui.horizontal(|ui| {
        ui.label(mono(value.to_string(), 12.5).strong().color(color));
        if lang.is_rtl() {
            ui.label(RichText::new(disp(label)).size(12.5).color(theme::INK_SOFT));
        } else {
            ui.label(mono(label, 12.5).color(theme::INK_SOFT));
        }
    });
}
