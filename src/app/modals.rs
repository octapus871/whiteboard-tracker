use super::*;

impl WhiteboardApp {
    pub(super) fn render_quick_entry_modal(&mut self, ctx: &egui::Context) {
        let lang = self.active_lang();
        let s = tr(lang);
        let is_rtl = lang.is_rtl();
        let screen = ctx.viewport_rect();
        let (enter_pressed, ctrl_down) = ctx.input(|i| {
            (
                i.key_pressed(Key::Enter),
                i.modifiers.ctrl || i.modifiers.command,
            )
        });

        let mut should_create = false;
        let mut close_modal = false;
        let mut remove_idx: Option<usize> = None;
        let mut start_edit_idx: Option<usize> = None;
        let mut commit_edit_on_enter = false;
        let mut commit_edit_on_blur = false;

        let modal_w = (screen.width() - 48.0).clamp(360.0, 580.0);
        let modal_h = (screen.height() - 96.0).clamp(280.0, 500.0);
        let modal_rect = Rect::from_center_size(screen.center(), vec2(modal_w, modal_h));

        // Fullscreen dimmed backdrop (fixed centered modal, not draggable or resizable;
        // clicking strictly outside `modal_rect` closes cleanly).
        egui::Area::new(Id::new("quick_entry_modal_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .default_size(screen.size())
            .constrain(false)
            .fade_in(false)
            .show(ctx, |ui| {
                let (_, bg_resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    screen,
                    CornerRadius::same(0),
                    Color32::from_black_alpha(60),
                );
                let _surface_resp = ui.interact(
                    modal_rect,
                    Id::new("quick_entry_modal_surface"),
                    Sense::click(),
                );
                if bg_resp.clicked() {
                    if let Some(pos) = bg_resp.interact_pointer_pos() {
                        if !modal_rect.contains(pos) {
                            close_modal = true;
                        }
                    }
                }

                // 4.0px solid #0F172A hard offset shadow + #FFFFFF 0.0px card + 1.5px #0F172A border.
                ui.painter().rect_filled(
                    modal_rect.translate(vec2(-4.0, 4.0)),
                    CornerRadius::same(0),
                    theme::INK,
                );
                ui.painter()
                    .rect_filled(modal_rect, CornerRadius::same(0), theme::PANE);
                ui.painter().rect_stroke(
                    modal_rect,
                    CornerRadius::same(0),
                    Stroke::new(1.5, theme::INK),
                    StrokeKind::Inside,
                );

                let inner = modal_rect.shrink(18.0);
                let align = if is_rtl { Align::Max } else { Align::Min };
                let mut m_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(align)),
                );
                m_ui.set_clip_rect(inner);

                // Header: title on leading side, date + crisp X close on trailing side.
                let trailing_layout = if is_rtl {
                    Layout::left_to_right(Align::Center)
                } else {
                    Layout::right_to_left(Align::Center)
                };
                let (_, date) = clock_strings_in_lang(lang);
                dir_row(&mut m_ui, 30.0, is_rtl, |ui| {
                    ui.label(
                        RichText::new(disp(s.quick_entry_title))
                            .size(18.0)
                            .strong()
                            .color(theme::INK),
                    );
                    ui.with_layout(trailing_layout, |ui| {
                        let (x_rect, x_resp) = ui.allocate_exact_size(
                            vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                            Sense::click(),
                        );
                        let x_resp = x_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(disp(s.tip_close_esc));
                        let x_col = if x_resp.hovered() || x_resp.has_focus() {
                            theme::INK
                        } else {
                            theme::INK_SOFT
                        };
                        if x_resp.has_focus() {
                            draw_focus_ring(ui.painter(), x_rect);
                        }
                        let c = x_rect.center();
                        let st = Stroke::new(1.6, x_col);
                        ui.painter().line_segment(
                            [pos2(c.x - 5.0, c.y - 5.0), pos2(c.x + 5.0, c.y + 5.0)],
                            st,
                        );
                        ui.painter().line_segment(
                            [pos2(c.x - 5.0, c.y + 5.0), pos2(c.x + 5.0, c.y - 5.0)],
                            st,
                        );
                        if x_resp.clicked() {
                            close_modal = true;
                        }
                        if is_rtl {
                            ui.label(RichText::new(disp(&date)).size(12.5).color(theme::SLATE));
                        } else {
                            ui.label(mono(&date, 12.0).color(theme::SLATE));
                        }
                    });
                });

                m_ui.add_space(6.0);
                let (div_rect, _) =
                    m_ui.allocate_exact_size(vec2(m_ui.available_width(), 1.0), Sense::hover());
                m_ui.painter()
                    .rect_filled(div_rect, CornerRadius::same(0), theme::SUBTLE_RULE);
                m_ui.add_space(8.0);

                m_ui.add(
                    Label::new(
                        RichText::new(disp(s.quick_entry_hint))
                            .size(11.5)
                            .color(theme::TEXT_MUTED),
                    )
                    .wrap(),
                );
                m_ui.add_space(8.0);

                let input_w = m_ui.available_width();
                let response = rtl_text_edit(
                    &mut m_ui,
                    Id::new("quick_entry_input"),
                    &mut self.quick_entry_input,
                    s.quick_entry_placeholder,
                    16.0,
                    input_w,
                    Margin {
                        left: if is_rtl { 0 } else { 10 },
                        right: if is_rtl { 10 } else { 0 },
                        top: 10,
                        bottom: 10,
                    },
                    theme::INK,
                    is_rtl,
                )
                .response;
                if self.focus_quick_entry {
                    response.request_focus();
                    self.focus_quick_entry = false;
                }
                if response.changed() && self.quick_entry_editing_idx.is_none() {
                    self.quick_entry_error = None;
                }

                let input_had_focus = response.has_focus() || response.lost_focus();
                if enter_pressed && ctrl_down {
                    should_create = true;
                } else if enter_pressed
                    && !ctrl_down
                    && input_had_focus
                    && self.quick_entry_editing_idx.is_none()
                {
                    self.stage_quick_entry_input();
                    response.request_focus();
                }

                if let Some(err_msg) = &self.quick_entry_error {
                    m_ui.add_space(6.0);
                    m_ui.add(
                        Label::new(
                            RichText::new(disp(err_msg))
                                .size(12.5)
                                .strong()
                                .color(theme::RED_BAR),
                        )
                        .wrap(),
                    );
                }

                m_ui.add_space(10.0);

                // Reserve bottom height for action buttons so the staged list scrolls cleanly.
                let footer_h = 44.0;
                let list_max_h = (m_ui.available_height() - footer_h).max(60.0);

                // Crisp 0.0px staged task rows (`[01] Task Title` + `×` remove button);
                // clicking a row enters inline edit (`Enter` saves, `Esc` cancels).
                if !self.quick_entry_list.is_empty() {
                    m_ui.label(
                        RichText::new(format!(
                            "{} ({})",
                            disp(s.quick_entry_preview),
                            self.quick_entry_list.len()
                        ))
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                    );
                    m_ui.add_space(4.0);
                    ScrollArea::vertical()
                        .id_salt("quick_entry_staged_list")
                        .max_height((list_max_h - 24.0).max(48.0))
                        .show(&mut m_ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            let staged_len = self.quick_entry_list.len();
                            for idx in 0..staged_len {
                                let is_editing_row = self.quick_entry_editing_idx == Some(idx);
                                let row_w = ui.available_width();
                                let (row_rect, _) = ui.allocate_exact_size(
                                    vec2(row_w, 32.0),
                                    Sense::hover(),
                                );

                                let rm_size = MIN_HITBOX_SIZE;
                                let rm_rect = if is_rtl {
                                    Rect::from_center_size(
                                        pos2(row_rect.left() + 16.0, row_rect.center().y),
                                        vec2(rm_size, rm_size),
                                    )
                                } else {
                                    Rect::from_center_size(
                                        pos2(row_rect.right() - 16.0, row_rect.center().y),
                                        vec2(rm_size, rm_size),
                                    )
                                };

                                let badge = format!("[{:02}]", idx + 1);
                                let text_clip = if is_rtl {
                                    let badge_x = row_rect.right() - 10.0;
                                    Rect::from_min_max(
                                        pos2(rm_rect.right() + 6.0, row_rect.top() + 2.0),
                                        pos2(badge_x - 38.0, row_rect.bottom() - 2.0),
                                    )
                                } else {
                                    let badge_x = row_rect.left() + 10.0;
                                    Rect::from_min_max(
                                        pos2(badge_x + 38.0, row_rect.top() + 2.0),
                                        pos2(rm_rect.left() - 6.0, row_rect.bottom() - 2.0),
                                    )
                                };

                                let row_click_resp = if !is_editing_row {
                                    Some(
                                        ui.interact(
                                            text_clip,
                                            ui.id().with(("quick_entry_row_click", idx)),
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(CursorIcon::PointingHand),
                                    )
                                } else {
                                    None
                                };
                                let row_hovered =
                                    row_click_resp.as_ref().is_some_and(|r| r.hovered());

                                ui.painter().rect_filled(
                                    row_rect,
                                    CornerRadius::same(0),
                                    if is_editing_row {
                                        theme::PANE
                                    } else {
                                        theme::DRAWER_BG
                                    },
                                );
                                ui.painter().rect_stroke(
                                    row_rect,
                                    CornerRadius::same(0),
                                    Stroke::new(
                                        if is_editing_row { 1.5 } else { 1.0 },
                                        if is_editing_row || row_hovered {
                                            theme::INK
                                        } else {
                                            theme::SUBTLE_RULE
                                        },
                                    ),
                                    StrokeKind::Inside,
                                );

                                let rm_resp = ui
                                    .interact(
                                        rm_rect,
                                        ui.id().with(("quick_entry_rm", idx)),
                                        Sense::click(),
                                    )
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(disp(s.tip_remove_staged));
                                let rm_color = if rm_resp.hovered() || rm_resp.has_focus() {
                                    theme::RED_BAR
                                } else {
                                    theme::SLATE
                                };
                                if rm_resp.has_focus() {
                                    draw_focus_ring(ui.painter(), rm_rect);
                                }
                                ui.painter().text(
                                    rm_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "\u{d7}",
                                    FontId::monospace(14.0),
                                    rm_color,
                                );
                                if rm_resp.clicked() {
                                    remove_idx = Some(idx);
                                }

                                if is_rtl {
                                    let badge_x = row_rect.right() - 10.0;
                                    ui.painter().text(
                                        pos2(badge_x, row_rect.center().y),
                                        egui::Align2::RIGHT_CENTER,
                                        &badge,
                                        FontId::monospace(12.0),
                                        theme::SLATE,
                                    );
                                } else {
                                    let badge_x = row_rect.left() + 10.0;
                                    ui.painter().text(
                                        pos2(badge_x, row_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        &badge,
                                        FontId::monospace(12.0),
                                        theme::SLATE,
                                    );
                                }

                                if is_editing_row {
                                    let mut edit_ui = ui.new_child(
                                        UiBuilder::new().max_rect(text_clip).layout(
                                            if is_rtl {
                                                Layout::right_to_left(Align::Center)
                                            } else {
                                                Layout::left_to_right(Align::Center)
                                            },
                                        ),
                                    );
                                    edit_ui.set_clip_rect(text_clip);
                                    let edit_resp = rtl_text_edit(
                                        &mut edit_ui,
                                        Id::new(("quick_entry_row_edit", idx)),
                                        &mut self.quick_entry_list[idx],
                                        s.task_title_placeholder,
                                        14.0,
                                        text_clip.width().max(40.0),
                                        Margin::symmetric(4, 2),
                                        theme::INK,
                                        is_rtl,
                                    )
                                    .response;
                                    if self.quick_entry_focus_edit_idx == Some(idx) {
                                        edit_resp.request_focus();
                                        self.quick_entry_focus_edit_idx = None;
                                    }
                                    if edit_resp.changed() {
                                        self.quick_entry_error = None;
                                    }
                                    let row_had_focus =
                                        edit_resp.has_focus() || edit_resp.lost_focus();
                                    if enter_pressed && !ctrl_down && row_had_focus {
                                        commit_edit_on_enter = true;
                                    } else if edit_resp.lost_focus() && !enter_pressed {
                                        commit_edit_on_blur = true;
                                    }
                                } else {
                                    let title_disp = disp(&self.quick_entry_list[idx]);
                                    if is_rtl {
                                        ui.painter().with_clip_rect(text_clip).text(
                                            pos2(text_clip.right(), row_rect.center().y),
                                            egui::Align2::RIGHT_CENTER,
                                            &title_disp,
                                            FontId::proportional(14.0),
                                            theme::INK,
                                        );
                                    } else {
                                        ui.painter().with_clip_rect(text_clip).text(
                                            pos2(text_clip.left(), row_rect.center().y),
                                            egui::Align2::LEFT_CENTER,
                                            &title_disp,
                                            FontId::proportional(14.0),
                                            theme::INK,
                                        );
                                    }
                                    if row_click_resp.is_some_and(|r| r.clicked()) {
                                        start_edit_idx = Some(idx);
                                    }
                                }
                            }
                        });
                    m_ui.add_space(8.0);
                }

                let has_pending_input = self
                    .quick_entry_input
                    .split(&['/', '\n'][..])
                    .any(|part| !part.trim().is_empty());
                let can_create = !self.quick_entry_list.is_empty() || has_pending_input;

                m_ui.add_space(6.0);
                dir_row(&mut m_ui, 34.0, is_rtl, |ui| {
                    if can_create {
                        let submit =
                            pill_button(ui, &disp(s.quick_entry_create), theme::GRID, true);
                        if submit.clicked() {
                            should_create = true;
                        }
                    } else {
                        Frame::new()
                            .fill(theme::SUBTLE_RULE)
                            .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
                            .inner_margin(Margin::symmetric(14, 7))
                            .show(ui, |ui| {
                                ui.add(
                                    Label::new(
                                        mono(&disp(s.quick_entry_create), 13.0)
                                            .strong()
                                            .color(theme::MUTED),
                                    )
                                    .selectable(false),
                                );
                            });
                    }
                    if ui
                        .add(
                            Button::new(
                                RichText::new(disp(s.quick_entry_cancel))
                                    .size(13.0)
                                    .color(theme::TEXT_MUTED),
                            )
                            .frame(false),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        close_modal = true;
                    }
                });
            });

        if let Some(idx) = remove_idx {
            if idx < self.quick_entry_list.len() {
                self.quick_entry_list.remove(idx);
                if let Some(edit_idx) = self.quick_entry_editing_idx {
                    if edit_idx == idx {
                        self.quick_entry_editing_idx = None;
                        self.quick_entry_edit_backup = None;
                        self.quick_entry_focus_edit_idx = None;
                    } else if edit_idx > idx {
                        self.quick_entry_editing_idx = Some(edit_idx - 1);
                    }
                }
                self.quick_entry_error = None;
            }
        } else if commit_edit_on_enter {
            let edit_idx = self.quick_entry_editing_idx;
            if !self.commit_quick_entry_row_edit(false) {
                if let Some(idx) = edit_idx {
                    self.quick_entry_focus_edit_idx = Some(idx);
                }
            }
        } else if commit_edit_on_blur && start_edit_idx.is_none() && !should_create && !close_modal
        {
            self.commit_quick_entry_row_edit(true);
        }

        if let Some(idx) = start_edit_idx {
            if self.quick_entry_editing_idx.is_some() {
                self.commit_quick_entry_row_edit(true);
            }
            if idx < self.quick_entry_list.len() {
                self.quick_entry_editing_idx = Some(idx);
                self.quick_entry_edit_backup = Some(self.quick_entry_list[idx].clone());
                self.quick_entry_focus_edit_idx = Some(idx);
                self.quick_entry_error = None;
            }
        }

        if close_modal {
            self.close_quick_entry_modal();
        } else if should_create {
            self.process_quick_entry();
        }
    }

    /// Clean, centered `0.0px` Swiss first-launch language selector modal (`460px` wide,
    /// `#FFFFFF` fill, `1.5px` `#0F172A` border, `4.0px` solid `#0F172A` offset shadow).
    pub(super) fn render_language_selector_modal(&mut self, ctx: &egui::Context) {
        let screen = ctx.viewport_rect();
        let card_w = (screen.width() - 40.0).clamp(320.0, 460.0);
        let card_h = 196.0_f32.min(screen.height() - 48.0).max(168.0);
        let card_rect = Rect::from_center_size(screen.center(), vec2(card_w, card_h));
        let mut chosen: Option<AppLang> = None;

        egui::Area::new(Id::new("first_launch_language_modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .default_size(screen.size())
            .constrain(false)
            .fade_in(false)
            .show(ctx, |ui| {
                let _ = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    screen,
                    CornerRadius::same(0),
                    Color32::from_black_alpha(72),
                );

                // 4.0px solid #0F172A offset shadow + #FFFFFF 0.0px card + 1.5px #0F172A border.
                ui.painter().rect_filled(
                    card_rect.translate(vec2(-4.0, 4.0)),
                    CornerRadius::same(0),
                    theme::INK,
                );
                ui.painter()
                    .rect_filled(card_rect, CornerRadius::same(0), theme::PANE);
                ui.painter().rect_stroke(
                    card_rect,
                    CornerRadius::same(0),
                    Stroke::new(1.5, theme::INK),
                    StrokeKind::Inside,
                );

                let inner = card_rect.shrink(24.0);
                let mut m_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(Align::Center)),
                );
                m_ui.set_clip_rect(inner);

                let title_str = format!(
                    "{}  |  SELECT INTERFACE LANGUAGE",
                    shape_arabic("اختر لغة الواجهة")
                );
                m_ui.label(
                    RichText::new(title_str)
                        .size(15.0)
                        .strong()
                        .color(theme::INK),
                );
                m_ui.add_space(12.0);
                let (div, _) =
                    m_ui.allocate_exact_size(vec2(m_ui.available_width(), 1.0), Sense::hover());
                m_ui.painter()
                    .rect_filled(div, CornerRadius::same(0), theme::SUBTLE_RULE);
                m_ui.add_space(22.0);

                let gap = 16.0;
                let btn_w = ((m_ui.available_width() - gap) * 0.5).max(120.0);
                let btn_h = 46.0;
                m_ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (lang_opt, label, font) in [
                        (AppLang::Ar, shape_arabic("العربية"), FontId::proportional(17.0)),
                        (AppLang::En, "English".to_string(), FontId::proportional(16.5)),
                    ] {
                        let (b_rect, b_resp) =
                            ui.allocate_exact_size(vec2(btn_w, btn_h), Sense::click());
                        let b_resp = b_resp.on_hover_cursor(CursorIcon::PointingHand);
                        let (bg, fg) = if b_resp.hovered() {
                            (theme::INK, theme::PANE)
                        } else {
                            (theme::PANE, theme::INK)
                        };
                        ui.painter().rect_filled(b_rect, CornerRadius::same(0), bg);
                        ui.painter().rect_stroke(
                            b_rect,
                            CornerRadius::same(0),
                            Stroke::new(1.5, theme::INK),
                            StrokeKind::Inside,
                        );
                        ui.painter().text(
                            b_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &label,
                            font.clone(),
                            fg,
                        );
                        ui.painter().text(
                            b_rect.center() + vec2(0.45, 0.0),
                            egui::Align2::CENTER_CENTER,
                            &label,
                            font,
                            fg,
                        );
                        if b_resp.clicked() {
                            chosen = Some(lang_opt);
                        }
                    }
                });
            });

        if let Some(lang) = chosen {
            self.set_language(lang);
            // Update an untouched default profile title to match the chosen language.
            if let Some(p) = self.data.profiles.first_mut() {
                if p.panes.is_empty() {
                    use chrono::Datelike;
                    p.title = DayProfile::title_for_weekday(chrono::Local::now().weekday(), lang);
                }
            }
            self.mark_dirty();
        }
    }

    /// Blocking modal shown on startup whenever `data.json` fails to read or parse.
    /// Explains what happened and where the timestamped `.bak` copy was saved, and
    /// prevents auto-save from silently overwriting `data.json` until the user makes
    /// an explicit choice.
    pub(super) fn render_load_error_modal(&mut self, ctx: &egui::Context) {
        let Some(err) = self.load_error.clone() else {
            return;
        };
        let lang_explicit = self.data.language;
        let lang = lang_explicit.unwrap_or(AppLang::En);
        let s = tr(lang);
        let is_rtl = lang_explicit.map(|l| l.is_rtl()).unwrap_or(false);

        let screen = ctx.viewport_rect();
        let card_w = (screen.width() - 48.0).clamp(380.0, 620.0);
        let card_h = (screen.height() - 64.0).clamp(320.0, 440.0);
        let card_rect = Rect::from_center_size(screen.center(), vec2(card_w, card_h));

        let mut start_fresh = false;
        let mut exit_app = false;
        let mut copy_backup_path: Option<String> = None;

        egui::Area::new(Id::new("data_load_error_modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .default_size(screen.size())
            .constrain(false)
            .fade_in(false)
            .show(ctx, |ui| {
                // Full-screen blocking backdrop: never dismisses on outside click.
                let _ = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    screen,
                    CornerRadius::same(0),
                    Color32::from_black_alpha(120),
                );

                // 4.0px solid hard offset shadow + #FFFFFF 0.0px card + 2.0px #DC2626 border.
                ui.painter().rect_filled(
                    card_rect.translate(vec2(-4.0, 4.0)),
                    CornerRadius::same(0),
                    theme::INK,
                );
                ui.painter()
                    .rect_filled(card_rect, CornerRadius::same(0), theme::PANE);
                ui.painter().rect_stroke(
                    card_rect,
                    CornerRadius::same(0),
                    Stroke::new(2.0, theme::RED_BAR),
                    StrokeKind::Inside,
                );

                let inner = card_rect.shrink(24.0);
                let align = if is_rtl { Align::Max } else { Align::Min };
                let mut m_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(align)),
                );
                m_ui.set_clip_rect(inner);

                let title_text = if lang_explicit.is_none() {
                    format!(
                        "{}  |  DATA LOAD ERROR",
                        shape_arabic("خطأ في تحميل البيانات")
                    )
                } else {
                    disp(s.load_error_title)
                };
                m_ui.label(
                    RichText::new(title_text)
                        .size(18.0)
                        .strong()
                        .color(theme::RED_BAR),
                );
                m_ui.add_space(8.0);
                let (div, _) =
                    m_ui.allocate_exact_size(vec2(m_ui.available_width(), 1.0), Sense::hover());
                m_ui.painter()
                    .rect_filled(div, CornerRadius::same(0), theme::SUBTLE_RULE);
                m_ui.add_space(10.0);

                // Explanation of what happened.
                m_ui.add(
                    Label::new(
                        RichText::new(disp(s.load_error_intro))
                            .size(14.5)
                            .color(theme::INK),
                    )
                    .wrap(),
                );
                if lang_explicit.is_none() {
                    m_ui.add_space(4.0);
                    m_ui.add(
                        Label::new(
                            RichText::new(disp(
                                "تعذّر قراءة أو تحليل ملف data.json. لم يتم مسح ملفك تلقائياً.",
                            ))
                            .size(14.0)
                            .color(theme::INK_SOFT),
                        )
                        .wrap(),
                    );
                }

                m_ui.add_space(12.0);
                m_ui.label(
                    RichText::new(disp(s.load_error_details_label))
                        .size(12.5)
                        .strong()
                        .color(theme::SLATE),
                );
                m_ui.add_space(4.0);
                Frame::new()
                    .fill(theme::DRAWER_BG)
                    .stroke(Stroke::new(1.0, theme::SUBTLE_RULE))
                    .inner_margin(Margin::symmetric(10, 8))
                    .show(&mut m_ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(
                            Label::new(mono(&err.message, 12.5).color(theme::RED_BAR)).wrap(),
                        );
                    });

                m_ui.add_space(12.0);
                if let Some(backup_path) = &err.backup_path {
                    let backup_str = backup_path.display().to_string();
                    m_ui.label(
                        RichText::new(disp(s.load_error_backup_label))
                            .size(12.5)
                            .strong()
                            .color(theme::INK),
                    );
                    m_ui.add_space(4.0);
                    Frame::new()
                        .fill(theme::DRAWER_BG)
                        .stroke(Stroke::new(1.0, theme::INK))
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(&mut m_ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(Label::new(mono(&backup_str, 12.5).color(theme::INK)).wrap());
                        });
                    m_ui.add_space(6.0);
                    let copy_label = if self.load_error_copied_path {
                        disp(s.load_error_copied_path)
                    } else {
                        disp(s.load_error_copy_path)
                    };
                    let (cp_rect, cp_resp) =
                        m_ui.allocate_exact_size(vec2(120.0, 28.0), Sense::click());
                    let cp_resp = cp_resp.on_hover_cursor(CursorIcon::PointingHand);
                    let cp_bg = if cp_resp.hovered() {
                        theme::SEL_FILL
                    } else {
                        theme::PANE
                    };
                    m_ui.painter()
                        .rect_filled(cp_rect, CornerRadius::same(0), cp_bg);
                    m_ui.painter().rect_stroke(
                        cp_rect,
                        CornerRadius::same(0),
                        Stroke::new(1.2, theme::INK),
                        StrokeKind::Inside,
                    );
                    m_ui.painter().text(
                        cp_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        copy_label,
                        FontId::monospace(12.0),
                        theme::INK,
                    );
                    if cp_resp.clicked() {
                        copy_backup_path = Some(backup_str);
                    }
                } else {
                    m_ui.label(
                        RichText::new(disp(s.load_error_backup_failed_label))
                            .size(12.5)
                            .strong()
                            .color(theme::RED_BAR),
                    );
                    m_ui.add_space(4.0);
                    let fallback_detail = err
                        .backup_error
                        .clone()
                        .unwrap_or_else(|| err.file_path.display().to_string());
                    Frame::new()
                        .fill(theme::DRAWER_BG)
                        .stroke(Stroke::new(1.0, theme::RED_BAR))
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(&mut m_ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                Label::new(mono(&fallback_detail, 12.0).color(theme::RED_BAR))
                                    .wrap(),
                            );
                        });
                }

                // Bottom action buttons pinned cleanly below content.
                m_ui.add_space(16.0);
                m_ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    // Safe default: Exit without touching data.json.
                    let (exit_rect, exit_resp) =
                        ui.allocate_exact_size(vec2(150.0, 36.0), Sense::click());
                    let exit_resp = exit_resp.on_hover_cursor(CursorIcon::PointingHand);
                    let exit_bg = if exit_resp.hovered() {
                        theme::GRID
                    } else {
                        theme::INK
                    };
                    ui.painter()
                        .rect_filled(exit_rect, CornerRadius::same(0), exit_bg);
                    ui.painter().text(
                        exit_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        disp(s.load_error_exit),
                        if is_rtl {
                            FontId::proportional(14.0)
                        } else {
                            FontId::monospace(13.0)
                        },
                        theme::PANE,
                    );
                    if exit_resp.clicked() {
                        exit_app = true;
                    }

                    // Explicit acknowledgement: Start fresh (keeps .bak intact).
                    let (fresh_rect, fresh_resp) =
                        ui.allocate_exact_size(vec2(168.0, 36.0), Sense::click());
                    let fresh_resp = fresh_resp.on_hover_cursor(CursorIcon::PointingHand);
                    let (fresh_bg, fresh_fg) = if fresh_resp.hovered() {
                        (theme::RED_BAR, theme::PANE)
                    } else {
                        (theme::PANE, theme::RED_BAR)
                    };
                    ui.painter()
                        .rect_filled(fresh_rect, CornerRadius::same(0), fresh_bg);
                    ui.painter().rect_stroke(
                        fresh_rect,
                        CornerRadius::same(0),
                        Stroke::new(1.5, theme::RED_BAR),
                        StrokeKind::Inside,
                    );
                    ui.painter().text(
                        fresh_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        disp(s.load_error_start_fresh),
                        if is_rtl {
                            FontId::proportional(14.0)
                        } else {
                            FontId::monospace(13.0)
                        },
                        fresh_fg,
                    );
                    if fresh_resp.clicked() {
                        start_fresh = true;
                    }
                });
            });

        if let Some(path_str) = copy_backup_path {
            ctx.copy_text(path_str);
            self.load_error_copied_path = true;
        }
        if exit_app {
            // Keep `self.load_error` intact so `force_save` on exit does NOT overwrite `data.json`.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if start_fresh {
            self.load_error = None;
            self.load_error_copied_path = false;
        }
    }

    /// Clean, centered `0.0px` Swiss architectural modal (`max_width: 640px`,
    /// `1.5px` `#0F172A` border, `4.0px` shadow, `#FFFFFF` fill) listing
    /// chronological daily profiles newest-first with inline title editing,
    /// completion badges, `+ بروفايل جديد`, `ترحيل النشط من الأمس`, `تفعيل`,
    /// and 2-click `حذف` (disabled when only 1 profile remains).
    pub(super) fn render_profiles_modal(&mut self, ctx: &egui::Context) {
        let lang = self.active_lang();
        let s = tr(lang);
        let is_rtl = lang.is_rtl();
        let screen = ctx.viewport_rect();
        let mut close_modal = false;
        let mut create_new = false;
        let mut carry_over = false;
        let mut activate_idx: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;
        let mut dirty = false;

        let modal_w = (screen.width() - 48.0).clamp(360.0, 640.0);
        let modal_h = (screen.height() - 96.0).clamp(280.0, 520.0);
        let modal_rect = Rect::from_center_size(screen.center(), vec2(modal_w, modal_h));

        // Dimmed backdrop: clicking strictly outside `modal_rect` closes cleanly.
        egui::Area::new(Id::new("profiles_modal_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .default_size(screen.size())
            .constrain(false)
            .fade_in(false)
            .show(ctx, |ui| {
                let (_, bg_resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    screen,
                    CornerRadius::same(0),
                    Color32::from_black_alpha(60),
                );
                let _surface_resp = ui.interact(
                    modal_rect,
                    Id::new("profiles_modal_surface"),
                    Sense::click(),
                );
                if bg_resp.clicked() {
                    if let Some(pos) = bg_resp.interact_pointer_pos() {
                        if !modal_rect.contains(pos) {
                            close_modal = true;
                        }
                    }
                }

                // 4.0px solid #0F172A hard offset shadow + #FFFFFF 0.0px card + 1.5px #0F172A border.
                ui.painter().rect_filled(
                    modal_rect.translate(vec2(-4.0, 4.0)),
                    CornerRadius::same(0),
                    theme::INK,
                );
                ui.painter()
                    .rect_filled(modal_rect, CornerRadius::same(0), theme::PANE);
                ui.painter().rect_stroke(
                    modal_rect,
                    CornerRadius::same(0),
                    Stroke::new(1.5, theme::INK),
                    StrokeKind::Inside,
                );

                let inner = modal_rect.shrink(16.0);
                let top_align = if is_rtl { Align::Max } else { Align::Min };
                let mut m_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(top_align)),
                );
                m_ui.set_clip_rect(inner);

                // Top header: title on leading side, tag + crisp X close on trailing side.
                let trailing_layout = if is_rtl {
                    Layout::left_to_right(Align::Center)
                } else {
                    Layout::right_to_left(Align::Center)
                };
                dir_row(&mut m_ui, 32.0, is_rtl, |ui| {
                    ui.label(
                        RichText::new(disp(s.profiles_title))
                            .size(18.0)
                            .strong()
                            .color(theme::INK),
                    );
                    ui.with_layout(trailing_layout, |ui| {
                        let (x_rect, x_resp) = ui.allocate_exact_size(
                            vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                            Sense::click(),
                        );
                        let x_resp = x_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(disp(s.tip_close_esc));
                        let x_col = if x_resp.hovered() {
                            theme::INK
                        } else {
                            theme::INK_SOFT
                        };
                        let c = x_rect.center();
                        let st = Stroke::new(1.6, x_col);
                        ui.painter()
                            .line_segment([pos2(c.x - 5.0, c.y - 5.0), pos2(c.x + 5.0, c.y + 5.0)], st);
                        ui.painter()
                            .line_segment([pos2(c.x - 5.0, c.y + 5.0), pos2(c.x + 5.0, c.y - 5.0)], st);
                        if x_resp.clicked() {
                            close_modal = true;
                        }
                        ui.label(mono(s.profiles_tag, 12.0).color(theme::SLATE));
                    });
                });

                m_ui.add_space(10.0);

                // Action bar: date input (`YYYY-MM-DD`), `+ بروفايل جديد` / `+ NEW PROFILE`,
                // and `ترحيل النشط من الأمس` / `CARRY OVER ACTIVE`.
                let has_previous = self.previous_profile_index().is_some();
                dir_row(&mut m_ui, 34.0, is_rtl, |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    let date_resp = Frame::new()
                        .fill(theme::PANE)
                        .stroke(Stroke::new(
                            1.5,
                            if self.profile_date_error.is_some() {
                                theme::RED_BAR
                            } else {
                                theme::INK
                            },
                        ))
                        .corner_radius(CornerRadius::same(0))
                        .inner_margin(Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                            ui.add_sized(
                                vec2(104.0, 22.0),
                                TextEdit::singleline(&mut self.profile_new_date_input)
                                    .id(Id::new("profile_new_date_input"))
                                    .font(FontId::monospace(13.0))
                                    .hint_text("YYYY-MM-DD")
                                    .frame(false),
                            )
                        })
                        .inner;
                    if date_resp.changed() {
                        self.profile_date_error = None;
                    }
                    if (date_resp.has_focus() || date_resp.lost_focus())
                        && ui.input(|i| i.key_pressed(Key::Enter))
                    {
                        create_new = true;
                    }

                    let (new_rect, new_resp) =
                        ui.allocate_exact_size(vec2(142.0, 32.0), Sense::click());
                    let new_resp = new_resp.on_hover_cursor(CursorIcon::PointingHand);
                    let new_bg = if new_resp.hovered() {
                        theme::GRID
                    } else {
                        theme::INK
                    };
                    ui.painter()
                        .rect_filled(new_rect, CornerRadius::same(0), new_bg);
                    ui.painter().text(
                        new_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        disp(s.profiles_new),
                        if is_rtl {
                            FontId::proportional(13.5)
                        } else {
                            FontId::monospace(12.5)
                        },
                        theme::PANE,
                    );
                    if new_resp.clicked() {
                        create_new = true;
                    }

                    let (carry_rect, carry_resp) =
                        ui.allocate_exact_size(vec2(178.0, 32.0), Sense::click());
                    let carry_resp = if has_previous {
                        carry_resp.on_hover_cursor(CursorIcon::PointingHand)
                    } else {
                        carry_resp
                    };
                    let carry_bg = if has_previous && carry_resp.hovered() {
                        theme::SEL_FILL
                    } else {
                        theme::PANE
                    };
                    let carry_border = if has_previous {
                        theme::INK
                    } else {
                        theme::HINT
                    };
                    let carry_fg = if has_previous {
                        theme::INK
                    } else {
                        theme::MUTED
                    };
                    ui.painter()
                        .rect_filled(carry_rect, CornerRadius::same(0), carry_bg);
                    ui.painter().rect_stroke(
                        carry_rect,
                        CornerRadius::same(0),
                        Stroke::new(1.5, carry_border),
                        StrokeKind::Inside,
                    );
                    ui.painter().text(
                        carry_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        disp(s.profiles_carry_over),
                        if is_rtl {
                            FontId::proportional(13.5)
                        } else {
                            FontId::monospace(12.0)
                        },
                        carry_fg,
                    );
                    if carry_resp.clicked() && has_previous {
                        carry_over = true;
                    }
                });

                if let Some(err) = &self.profile_date_error {
                    m_ui.add_space(4.0);
                    m_ui.label(
                        RichText::new(disp(err))
                            .size(12.0)
                            .strong()
                            .color(theme::RED_BAR),
                    );
                }

                m_ui.add_space(10.0);
                let (div_rect, _) =
                    m_ui.allocate_exact_size(vec2(m_ui.available_width(), 1.0), Sense::hover());
                m_ui.painter()
                    .rect_filled(div_rect, CornerRadius::same(0), theme::SUBTLE_RULE);
                m_ui.add_space(8.0);

                // Chronological profile list (newest first).
                let profile_count = self.data.profiles.len();
                let active_idx = self.data.active_profile_idx;
                let list_h = m_ui.available_height().max(80.0);
                ScrollArea::vertical()
                    .id_salt("profiles_modal_list")
                    .auto_shrink([false, false])
                    .max_height(list_h)
                    .show(&mut m_ui, |ui| {
                        for idx in 0..profile_count {
                            let profile = &mut self.data.profiles[idx];
                            let profile_id = profile.id.clone();
                            let date_str = profile.date_str.clone();
                            let (done, total) = profile.completion_counts();
                            let is_active = idx == active_idx;
                            let is_confirming_del = self
                                .profile_confirm_delete_id
                                .as_deref()
                                == Some(profile_id.as_str());
                            let can_delete = profile_count > 1;

                            let row_w = ui.available_width();
                            let (row_rect, _) =
                                ui.allocate_exact_size(vec2(row_w, 48.0), Sense::hover());
                            if is_active {
                                ui.painter().rect_filled(
                                    row_rect,
                                    CornerRadius::same(0),
                                    theme::SEL_FILL,
                                );
                                ui.painter().rect_stroke(
                                    row_rect,
                                    CornerRadius::same(0),
                                    Stroke::new(1.5, theme::INK),
                                    StrokeKind::Inside,
                                );
                            }

                            let row_inner = row_rect.shrink2(vec2(8.0, 4.0));
                            let row_layout = if is_rtl {
                                Layout::right_to_left(Align::Center)
                            } else {
                                Layout::left_to_right(Align::Center)
                            };
                            let mut row_ui = ui.new_child(
                                UiBuilder::new().max_rect(row_inner).layout(row_layout),
                            );
                            row_ui.set_clip_rect(row_inner);
                            row_ui.spacing_mut().item_spacing.x = 8.0;

                            // Leading side: Monospace Bold date string (`YYYY-MM-DD`).
                            row_ui.label(mono(&date_str, 13.0).strong().color(theme::INK));

                            // Inline-editable profile title (`16.0px`).
                            let controls_w = 212.0;
                            let title_w = (row_ui.available_width() - controls_w).max(90.0);
                            let edit_resp = rtl_text_edit(
                                &mut row_ui,
                                Id::new(("profile_title_edit", profile_id.as_str())),
                                &mut profile.title,
                                s.profiles_title_placeholder,
                                16.0,
                                title_w,
                                Margin::symmetric(4, 2),
                                theme::INK,
                                is_rtl,
                            )
                            .response;
                            if self.profile_focus_title_id.as_deref() == Some(profile_id.as_str()) {
                                edit_resp.request_focus();
                                self.profile_focus_title_id = None;
                            }
                            if edit_resp.changed() {
                                dirty = true;
                            }

                            // Trailing controls cluster: Delete, Open/Active, completion badge.
                            row_ui.with_layout(trailing_layout, |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                // 1. 2-click Delete (disabled if only 1 profile remains).
                                if !can_delete {
                                    let (del_rect, _) = ui.allocate_exact_size(
                                        vec2(54.0, MIN_HITBOX_SIZE),
                                        Sense::hover(),
                                    );
                                    ui.painter().text(
                                        del_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        disp(s.action_delete),
                                        if is_rtl {
                                            FontId::proportional(12.5)
                                        } else {
                                            FontId::monospace(11.5)
                                        },
                                        theme::HINT,
                                    );
                                } else if is_confirming_del {
                                    let (c_rect, c_resp) = ui.allocate_exact_size(
                                        vec2(64.0, MIN_HITBOX_SIZE),
                                        Sense::click(),
                                    );
                                    let c_resp = c_resp.on_hover_cursor(CursorIcon::PointingHand);
                                    let pill =
                                        Rect::from_center_size(c_rect.center(), vec2(62.0, 22.0));
                                    ui.painter().rect_filled(
                                        pill,
                                        CornerRadius::same(0),
                                        theme::RED_BAR,
                                    );
                                    ui.painter().text(
                                        pill.center(),
                                        egui::Align2::CENTER_CENTER,
                                        disp(s.action_confirm),
                                        if is_rtl {
                                            FontId::proportional(12.5)
                                        } else {
                                            FontId::monospace(11.0)
                                        },
                                        theme::PANE,
                                    );
                                    if c_resp.clicked() {
                                        delete_idx = Some(idx);
                                    }
                                } else {
                                    let del_resp = index_text_action(
                                        ui,
                                        s.action_delete,
                                        if is_rtl { 42.0 } else { 54.0 },
                                        theme::SLATE,
                                        theme::RED_BAR,
                                    );
                                    if del_resp.clicked() {
                                        self.profile_confirm_delete_id = Some(profile_id.clone());
                                    }
                                }

                                // 2. Activate (`تفعيل` / `OPEN`) or active badge (`نشط` / `ACTIVE`).
                                if is_active {
                                    let (act_rect, _) = ui.allocate_exact_size(
                                        vec2(56.0, MIN_HITBOX_SIZE),
                                        Sense::hover(),
                                    );
                                    let pill =
                                        Rect::from_center_size(act_rect.center(), vec2(54.0, 22.0));
                                    ui.painter().rect_filled(
                                        pill,
                                        CornerRadius::same(0),
                                        theme::GREEN_BAR,
                                    );
                                    ui.painter().text(
                                        pill.center(),
                                        egui::Align2::CENTER_CENTER,
                                        disp(s.action_active_badge),
                                        if is_rtl {
                                            FontId::proportional(12.5)
                                        } else {
                                            FontId::monospace(11.0)
                                        },
                                        theme::PANE,
                                    );
                                } else {
                                    let (open_rect, open_resp) = ui.allocate_exact_size(
                                        vec2(56.0, MIN_HITBOX_SIZE),
                                        Sense::click(),
                                    );
                                    let open_resp =
                                        open_resp.on_hover_cursor(CursorIcon::PointingHand);
                                    let pill = Rect::from_center_size(
                                        open_rect.center(),
                                        vec2(54.0, 22.0),
                                    );
                                    let bg = if open_resp.hovered() {
                                        theme::INK
                                    } else {
                                        theme::PANE
                                    };
                                    let fg = if open_resp.hovered() {
                                        theme::PANE
                                    } else {
                                        theme::INK
                                    };
                                    ui.painter().rect_filled(pill, CornerRadius::same(0), bg);
                                    ui.painter().rect_stroke(
                                        pill,
                                        CornerRadius::same(0),
                                        Stroke::new(1.2, theme::INK),
                                        StrokeKind::Inside,
                                    );
                                    ui.painter().text(
                                        pill.center(),
                                        egui::Align2::CENTER_CENTER,
                                        disp(s.action_open),
                                        if is_rtl {
                                            FontId::proportional(12.5)
                                        } else {
                                            FontId::monospace(11.5)
                                        },
                                        fg,
                                    );
                                    if open_resp.clicked() {
                                        activate_idx = Some(idx);
                                    }
                                }

                                // 3. Completion badge (`done/total` in `#10B981` or `#0F172A`).
                                let badge_col = if total > 0 && done == total {
                                    theme::GREEN_BAR
                                } else {
                                    theme::INK
                                };
                                ui.label(
                                    mono(format!("{}/{}", done, total), 13.0)
                                        .strong()
                                        .color(badge_col),
                                );
                            });

                            if idx + 1 < profile_count {
                                let (sep, _) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 1.0),
                                    Sense::hover(),
                                );
                                ui.painter()
                                    .rect_filled(sep, CornerRadius::same(0), theme::SUBTLE_RULE);
                            }
                        }
                    });
            });

        if dirty {
            self.mark_dirty();
        }
        if create_new {
            let date_input = self.profile_new_date_input.clone();
            match self.create_profile_for_date(&date_input, Some(ctx)) {
                Ok(new_id) => {
                    let edit_id = Id::new(("profile_title_edit", new_id.as_str()));
                    ctx.memory_mut(|m| m.request_focus(edit_id));
                }
                Err(err) => {
                    self.profile_date_error = Some(err);
                }
            }
        } else if carry_over {
            self.carry_over_active_from_previous(Some(ctx));
            self.show_profiles_modal = false;
        } else if let Some(idx) = activate_idx {
            self.switch_to_profile(idx, Some(ctx));
            self.show_profiles_modal = false;
        } else if let Some(idx) = delete_idx {
            self.delete_profile_at(idx, Some(ctx));
        } else if close_modal {
            self.show_profiles_modal = false;
            self.profile_confirm_delete_id = None;
            self.profile_focus_title_id = None;
            self.profile_date_error = None;
        }
    }

    /// Minimal First-Run Quick Guide (`520px` wide, `0.0px` Swiss welcome card,
    /// `#FFFFFF` fill, `1.5px` `#0F172A` border, `4.0px` solid `#0F172A` shadow).
    pub(super) fn render_onboarding_modal(&mut self, ctx: &egui::Context) {
        let lang = self.active_lang();
        let s = tr(lang);
        let is_rtl = lang.is_rtl();
        let screen = ctx.viewport_rect();
        let card_w = (screen.width() - 40.0).clamp(320.0, 520.0);
        let card_h = 348.0_f32.min(screen.height() - 48.0).max(260.0);
        let card_rect = Rect::from_center_size(screen.center(), vec2(card_w, card_h));
        let mut start_work = false;

        egui::Area::new(Id::new("onboarding_quick_guide"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .default_size(screen.size())
            .constrain(false)
            .fade_in(false)
            .show(ctx, |ui| {
                let _ = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    screen,
                    CornerRadius::same(0),
                    Color32::from_black_alpha(64),
                );

                // 4.0px solid #0F172A offset shadow + #FFFFFF card + 1.5px #0F172A border.
                ui.painter().rect_filled(
                    card_rect.translate(vec2(-4.0, 4.0)),
                    CornerRadius::same(0),
                    theme::INK,
                );
                ui.painter()
                    .rect_filled(card_rect, CornerRadius::same(0), theme::PANE);
                ui.painter().rect_stroke(
                    card_rect,
                    CornerRadius::same(0),
                    Stroke::new(1.5, theme::INK),
                    StrokeKind::Inside,
                );

                let inner = card_rect.shrink(24.0);
                let align = if is_rtl { Align::Max } else { Align::Min };
                let mut m_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(align)),
                );
                m_ui.set_clip_rect(inner);

                // Title: `دليل الاستخدام السريع` / `QUICK START GUIDE` (`20.0px` Bold `#0F172A`).
                m_ui.label(
                    RichText::new(disp(s.guide_title))
                        .size(20.0)
                        .strong()
                        .color(theme::INK),
                );
                m_ui.add_space(10.0);

                let (div, _) =
                    m_ui.allocate_exact_size(vec2(m_ui.available_width(), 1.0), Sense::hover());
                m_ui.painter()
                    .rect_filled(div, CornerRadius::same(0), theme::SUBTLE_RULE);
                m_ui.add_space(14.0);

                let bullet_font = FontId::proportional(15.5);
                let bullet_color = Color32::from_rgb(0x1E, 0x29, 0x3B);
                let wrap_w = m_ui.available_width().max(120.0);
                for line in s.guide_bullets {
                    let (wrapped, size) = wrap_and_shape_rtl(&m_ui, line, bullet_font.clone(), wrap_w);
                    let (line_rect, _) =
                        m_ui.allocate_exact_size(vec2(wrap_w, size.y + 4.0), Sense::hover());
                    let mut job = egui::text::LayoutJob::default();
                    job.wrap.max_width = if is_rtl { f32::INFINITY } else { wrap_w };
                    job.halign = if is_rtl { Align::RIGHT } else { Align::LEFT };
                    job.append(
                        &wrapped,
                        0.0,
                        egui::TextFormat {
                            font_id: bullet_font.clone(),
                            color: bullet_color,
                            ..Default::default()
                        },
                    );
                    let galley = m_ui.fonts_mut(|f| f.layout_job(job));
                    let anchor_x = if is_rtl {
                        line_rect.right()
                    } else {
                        line_rect.left()
                    };
                    m_ui.painter().galley(
                        pos2(anchor_x, line_rect.top()),
                        galley,
                        bullet_color,
                    );
                    m_ui.add_space(8.0);
                }

                m_ui.add_space(10.0);
                // High-contrast CTA button `ابدأ العمل` / `START WORKING` (`#0F172A` fill, `#FFFFFF` 15.5px Bold).
                let (btn_rect, btn_resp) =
                    m_ui.allocate_exact_size(vec2(170.0, 38.0), Sense::click());
                let btn_resp = btn_resp.on_hover_cursor(CursorIcon::PointingHand);
                let btn_bg = if btn_resp.hovered() {
                    theme::GRID
                } else {
                    theme::INK
                };
                m_ui.painter()
                    .rect_filled(btn_rect, CornerRadius::same(0), btn_bg);
                let cta_text = disp(s.guide_cta);
                let cta_font = FontId::proportional(15.5);
                m_ui.painter().text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &cta_text,
                    cta_font.clone(),
                    theme::PANE,
                );
                m_ui.painter().text(
                    btn_rect.center() + vec2(0.45, 0.0),
                    egui::Align2::CENTER_CENTER,
                    &cta_text,
                    cta_font,
                    theme::PANE,
                );
                if btn_resp.clicked() {
                    start_work = true;
                }
            });

        if start_work {
            self.data.has_seen_onboarding = true;
            self.show_onboarding_modal = false;
            if self.data.current_day.panes.is_empty() {
                self.open_quick_entry_modal();
            }
            self.mark_dirty();
        }
    }

    pub(super) fn show_status_toast(&mut self, msg: String, ctx: Option<&egui::Context>) {
        let now = ctx
            .map(|c| c.input(|i| i.time))
            .unwrap_or(self.last_frame_time);
        self.undo_toast = Some((msg, now + UNDO_TOAST_DURATION_SECS));
    }

    pub(super) fn start_export_json_dialog(&mut self, ctx: &egui::Context) {
        if self.data_dialog_rx.is_some() {
            return;
        }
        self.data_io_error = None;
        let default_name = format!("whiteboard_backup_{}.json", self.active_profile_date());
        let title = tr(self.active_lang()).data_io_export_json.to_string();
        let (tx, rx) = mpsc::channel();
        let ctx_clone = ctx.clone();
        self.data_dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let res = pick_save_file(&title, &default_name, "JSON files", "json");
            let _ = tx.send(DataDialogResult::ExportJson(res));
            ctx_clone.request_repaint();
        });
    }

    pub(super) fn start_export_markdown_dialog(&mut self, ctx: &egui::Context) {
        if self.data_dialog_rx.is_some() {
            return;
        }
        self.data_io_error = None;
        let default_name = format!("whiteboard_{}.md", self.active_profile_date());
        let title = tr(self.active_lang()).data_io_export_md.to_string();
        let (tx, rx) = mpsc::channel();
        let ctx_clone = ctx.clone();
        self.data_dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let res = pick_save_file(&title, &default_name, "Markdown files", "md");
            let _ = tx.send(DataDialogResult::ExportMarkdown(res));
            ctx_clone.request_repaint();
        });
    }

    pub(super) fn start_import_json_dialog(&mut self, ctx: &egui::Context) {
        if self.data_dialog_rx.is_some() {
            return;
        }
        self.data_io_error = None;
        let title = tr(self.active_lang()).data_io_import_json.to_string();
        let (tx, rx) = mpsc::channel();
        let ctx_clone = ctx.clone();
        self.data_dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let res = pick_open_file(&title, "JSON files", "json");
            let _ = tx.send(DataDialogResult::ImportJson(res));
            ctx_clone.request_repaint();
        });
    }

    pub(super) fn poll_data_dialog(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.data_dialog_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.data_dialog_rx = None;
                match result {
                    DataDialogResult::ExportJson(Ok(Some(path))) => {
                        self.data.sync_active_profile_from_current_day();
                        match export_app_data_to_path(&self.data, &path) {
                            Ok(()) => {
                                let name = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("backup.json");
                                let msg = match self.active_lang() {
                                    AppLang::Ar => format!("تم التصدير: {}", name),
                                    AppLang::En => format!("Exported JSON: {}", name),
                                };
                                self.show_data_io_modal = false;
                                self.data_io_error = None;
                                self.show_status_toast(msg, Some(ctx));
                            }
                            Err(e) => {
                                self.data_io_error = Some(e);
                            }
                        }
                    }
                    DataDialogResult::ExportMarkdown(Ok(Some(path))) => {
                        let active_title = self
                            .data
                            .profiles
                            .get(self.data.active_profile_idx)
                            .map(|p| p.title.as_str());
                        let md = format_day_as_markdown(&self.data.current_day, active_title);
                        match export_markdown_to_path(&md, &path) {
                            Ok(()) => {
                                let name = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("day.md");
                                let msg = match self.active_lang() {
                                    AppLang::Ar => format!("تم تصدير Markdown: {}", name),
                                    AppLang::En => format!("Exported Markdown: {}", name),
                                };
                                self.show_data_io_modal = false;
                                self.data_io_error = None;
                                self.show_status_toast(msg, Some(ctx));
                            }
                            Err(e) => {
                                self.data_io_error = Some(e);
                            }
                        }
                    }
                    DataDialogResult::ImportJson(Ok(Some(path))) => {
                        match import_app_data_from_path(&path) {
                            Ok(imported) => {
                                self.pending_import = Some(PendingImport {
                                    source_path: path,
                                    data: imported,
                                });
                                self.data_io_error = None;
                            }
                            Err(e) => {
                                self.data_io_error = Some(e);
                            }
                        }
                    }
                    DataDialogResult::ExportJson(Ok(None))
                    | DataDialogResult::ExportMarkdown(Ok(None))
                    | DataDialogResult::ImportJson(Ok(None)) => {}
                    DataDialogResult::ExportJson(Err(e))
                    | DataDialogResult::ExportMarkdown(Err(e))
                    | DataDialogResult::ImportJson(Err(e)) => {
                        self.data_io_error = Some(e);
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.data_dialog_rx = None;
            }
        }
    }

    pub(super) fn confirm_pending_import_with<B, S>(
        &mut self,
        backup_fn: B,
        save_fn: S,
        ctx: Option<&egui::Context>,
    ) -> Result<PathBuf, String>
    where
        B: FnOnce(&AppData) -> Result<PathBuf, String>,
        S: FnOnce(&AppData) -> Result<(), String>,
    {
        let Some(pending) = self.pending_import.clone() else {
            return Err("No pending import to confirm".to_string());
        };
        self.data.sync_active_profile_from_current_day();
        let backup_path = match backup_fn(&self.data) {
            Ok(p) => p,
            Err(e) => {
                let msg = format!("Backup failed — import cancelled: {}", e);
                self.data_io_error = Some(msg.clone());
                return Err(msg);
            }
        };

        let mut imported = pending.data;
        if imported.profiles.is_empty() {
            imported.ensure_profiles_initialized();
        } else {
            let prev_active_id = imported
                .profiles
                .get(imported.active_profile_idx)
                .map(|p| p.id.clone());
            imported
                .profiles
                .sort_by(|a, b| b.created_ts.cmp(&a.created_ts));
            imported.active_profile_idx = prev_active_id
                .and_then(|id| imported.profiles.iter().position(|p| p.id == id))
                .unwrap_or(0)
                .min(imported.profiles.len().saturating_sub(1));
            if let Some(active) = imported.profiles.get(imported.active_profile_idx) {
                imported.current_day.panes = active.panes.clone();
                imported.current_day.date_iso = active.date_str.clone();
            }
        }

        self.close_all_note_drawers(ctx);
        self.data = imported;
        self.pending_import = None;
        self.show_data_io_modal = false;
        self.data_io_error = None;
        self.show_quick_entry_modal = false;
        self.show_onboarding_modal = false;
        self.show_profiles_modal = false;
        self.undo_stack.clear();
        self.pending_card_drawer_click = None;
        self.current_page = 0;
        self.focused_pane_id = None;
        self.focused_node_id = None;
        self.editing_node_id = None;
        self.selected_node_ids.clear();
        self.selected_pane_ids.clear();
        self.pane_inputs.clear();
        self.scroll_seeded.clear();
        self.index_selected_pane_id = None;
        self.index_confirm_delete_pane_id = None;
        self.index_editing_pane_id = None;
        self.index_edit_backup_title = None;
        self.profile_confirm_delete_id = None;

        self.mark_dirty();
        self.try_save_with(Instant::now(), save_fn);

        let bak_name = backup_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("data.json.bak");
        let toast = match self.active_lang() {
            AppLang::Ar => format!("تم الاستيراد (نسخة احتياطية: {})", bak_name),
            AppLang::En => format!("Imported (backup: {})", bak_name),
        };
        self.show_status_toast(toast, ctx);
        Ok(backup_path)
    }

    pub fn confirm_pending_import(&mut self, ctx: Option<&egui::Context>) -> Result<PathBuf, String> {
        self.confirm_pending_import_with(backup_current_app_data, save_app_data, ctx)
    }

    /// Centered Swiss architectural modal for JSON Export/Import (with automatic pre-import
    /// `.bak` backup & confirmation) and current-day Markdown export.
    pub(super) fn render_data_io_modal(&mut self, ctx: &egui::Context) {
        let lang = self.active_lang();
        let s = tr(lang);
        let is_rtl = lang.is_rtl();
        let screen = ctx.viewport_rect();
        let waiting_dialog = self.data_dialog_rx.is_some();

        let modal_w = (screen.width() - 48.0).clamp(380.0, 560.0);
        let modal_h = (screen.height() - 96.0).clamp(300.0, 410.0);
        let modal_rect = Rect::from_center_size(screen.center(), vec2(modal_w, modal_h));

        let mut close_modal = false;
        let mut trigger_export_json = false;
        let mut trigger_export_md = false;
        let mut trigger_import_json = false;
        let mut do_confirm_import = false;
        let mut do_cancel_import = false;

        egui::Area::new(Id::new("data_io_modal_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .default_size(screen.size())
            .constrain(false)
            .fade_in(false)
            .show(ctx, |ui| {
                let (_, bg_resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    screen,
                    CornerRadius::same(0),
                    Color32::from_black_alpha(64),
                );
                let _surface_resp = ui.interact(
                    modal_rect,
                    Id::new("data_io_modal_surface"),
                    Sense::click(),
                );
                if !waiting_dialog && self.pending_import.is_none() && bg_resp.clicked() {
                    if let Some(pos) = bg_resp.interact_pointer_pos() {
                        if !modal_rect.contains(pos) {
                            close_modal = true;
                        }
                    }
                }

                // 4.0px solid #0F172A offset shadow + #FFFFFF card + 1.5px #0F172A border.
                ui.painter().rect_filled(
                    modal_rect.translate(vec2(-4.0, 4.0)),
                    CornerRadius::same(0),
                    theme::INK,
                );
                ui.painter()
                    .rect_filled(modal_rect, CornerRadius::same(0), theme::PANE);
                ui.painter().rect_stroke(
                    modal_rect,
                    CornerRadius::same(0),
                    Stroke::new(1.5, theme::INK),
                    StrokeKind::Inside,
                );

                let inner = modal_rect.shrink(20.0);
                let align = if is_rtl { Align::Max } else { Align::Min };
                let mut m_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(align)),
                );
                m_ui.set_clip_rect(inner);

                let trailing_layout = if is_rtl {
                    Layout::left_to_right(Align::Center)
                } else {
                    Layout::right_to_left(Align::Center)
                };
                let title_str = if self.pending_import.is_some() {
                    disp(s.data_io_confirm_title)
                } else {
                    disp(s.data_io_title)
                };
                dir_row(&mut m_ui, 30.0, is_rtl, |ui| {
                    ui.label(
                        RichText::new(title_str)
                            .size(18.0)
                            .strong()
                            .color(theme::INK),
                    );
                    ui.with_layout(trailing_layout, |ui| {
                        let (x_rect, x_resp) = ui.allocate_exact_size(
                            vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                            Sense::click(),
                        );
                        let x_resp = x_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(disp(s.tip_close_esc));
                        let c = x_rect.center();
                        let st = Stroke::new(
                            1.6,
                            if x_resp.hovered() {
                                theme::INK
                            } else {
                                theme::INK_SOFT
                            },
                        );
                        ui.painter().line_segment(
                            [pos2(c.x - 5.0, c.y - 5.0), pos2(c.x + 5.0, c.y + 5.0)],
                            st,
                        );
                        ui.painter().line_segment(
                            [pos2(c.x - 5.0, c.y + 5.0), pos2(c.x + 5.0, c.y - 5.0)],
                            st,
                        );
                        if x_resp.clicked() && !waiting_dialog {
                            if self.pending_import.is_some() {
                                do_cancel_import = true;
                            } else {
                                close_modal = true;
                            }
                        }
                    });
                });

                m_ui.add_space(8.0);
                let (div, _) =
                    m_ui.allocate_exact_size(vec2(m_ui.available_width(), 1.0), Sense::hover());
                m_ui.painter()
                    .rect_filled(div, CornerRadius::same(0), theme::SUBTLE_RULE);
                m_ui.add_space(12.0);

                if let Some(pending) = self.pending_import.clone() {
                    let profile_count = pending.data.profiles.len().max(1);
                    let pane_count: usize = if pending.data.profiles.is_empty() {
                        pending.data.current_day.panes.len()
                    } else {
                        pending.data.profiles.iter().map(|p| p.panes.len()).sum()
                    };
                    let archived_count = pending.data.archived_panes.len();
                    let summary = match lang {
                        AppLang::Ar => format!(
                            "البروفايلات: {} • اللوحات: {} • الأرشيف: {}",
                            profile_count, pane_count, archived_count
                        ),
                        AppLang::En => format!(
                            "Profiles: {} • Panes: {} • Archived: {}",
                            profile_count, pane_count, archived_count
                        ),
                    };

                    Frame::new()
                        .fill(theme::DRAWER_BG)
                        .stroke(Stroke::new(1.0, theme::SUBTLE_RULE))
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(&mut m_ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                Label::new(
                                    mono(pending.source_path.display().to_string(), 12.0)
                                        .color(theme::INK),
                                )
                                .wrap(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(disp(&summary))
                                    .size(13.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                        });

                    m_ui.add_space(10.0);
                    m_ui.add(
                        Label::new(
                            RichText::new(disp(s.data_io_confirm_warning))
                                .size(13.5)
                                .color(theme::INK_SOFT),
                        )
                        .wrap(),
                    );

                    if let Some(err) = &self.data_io_error {
                        m_ui.add_space(8.0);
                        m_ui.add(
                            Label::new(
                                RichText::new(disp(err))
                                    .size(12.5)
                                    .strong()
                                    .color(theme::RED_BAR),
                            )
                            .wrap(),
                        );
                    }

                    m_ui.add_space(16.0);
                    dir_row(&mut m_ui, 36.0, is_rtl, |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let (confirm_rect, confirm_resp) =
                            ui.allocate_exact_size(vec2(170.0, 34.0), Sense::click());
                        let confirm_resp = confirm_resp.on_hover_cursor(CursorIcon::PointingHand);
                        let bg = if confirm_resp.hovered() {
                            theme::INK
                        } else {
                            theme::RED_BAR
                        };
                        ui.painter()
                            .rect_filled(confirm_rect, CornerRadius::same(0), bg);
                        ui.painter().text(
                            confirm_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            disp(s.data_io_confirm_btn),
                            if is_rtl {
                                FontId::proportional(13.5)
                            } else {
                                FontId::monospace(12.5)
                            },
                            theme::PANE,
                        );
                        if confirm_resp.clicked() {
                            do_confirm_import = true;
                        }

                        let (cancel_rect, cancel_resp) =
                            ui.allocate_exact_size(vec2(110.0, 34.0), Sense::click());
                        let cancel_resp = cancel_resp.on_hover_cursor(CursorIcon::PointingHand);
                        let c_bg = if cancel_resp.hovered() {
                            theme::SEL_FILL
                        } else {
                            theme::PANE
                        };
                        ui.painter()
                            .rect_filled(cancel_rect, CornerRadius::same(0), c_bg);
                        ui.painter().rect_stroke(
                            cancel_rect,
                            CornerRadius::same(0),
                            Stroke::new(1.5, theme::INK),
                            StrokeKind::Inside,
                        );
                        ui.painter().text(
                            cancel_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            disp(s.quick_entry_cancel),
                            if is_rtl {
                                FontId::proportional(13.5)
                            } else {
                                FontId::monospace(12.5)
                            },
                            theme::INK,
                        );
                        if cancel_resp.clicked() {
                            do_cancel_import = true;
                        }
                    });
                } else {
                    let action_row = |ui: &mut Ui,
                                          label: &str,
                                          desc: &str,
                                          primary: bool,
                                          disabled: bool|
                     -> bool {
                        let mut clicked = false;
                        Frame::new()
                            .fill(theme::DRAWER_BG)
                            .stroke(Stroke::new(1.0, theme::SUBTLE_RULE))
                            .inner_margin(Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                dir_row(ui, 36.0, is_rtl, |ui| {
                                    let btn_w = 196.0;
                                    let (b_rect, b_resp) = ui.allocate_exact_size(
                                        vec2(btn_w, 32.0),
                                        if disabled {
                                            Sense::hover()
                                        } else {
                                            Sense::click()
                                        },
                                    );
                                    let b_resp = if disabled {
                                        b_resp
                                    } else {
                                        b_resp.on_hover_cursor(CursorIcon::PointingHand)
                                    };
                                    let (bg, fg, border) = if disabled {
                                        (theme::SUBTLE_RULE, theme::MUTED, theme::HINT)
                                    } else if primary {
                                        (
                                            if b_resp.hovered() {
                                                theme::GRID
                                            } else {
                                                theme::INK
                                            },
                                            theme::PANE,
                                            theme::INK,
                                        )
                                    } else {
                                        (
                                            if b_resp.hovered() {
                                                theme::SEL_FILL
                                            } else {
                                                theme::PANE
                                            },
                                            theme::INK,
                                            theme::INK,
                                        )
                                    };
                                    ui.painter()
                                        .rect_filled(b_rect, CornerRadius::same(0), bg);
                                    ui.painter().rect_stroke(
                                        b_rect,
                                        CornerRadius::same(0),
                                        Stroke::new(1.5, border),
                                        StrokeKind::Inside,
                                    );
                                    ui.painter().text(
                                        b_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        disp(label),
                                        if is_rtl {
                                            FontId::proportional(13.0)
                                        } else {
                                            FontId::monospace(11.5)
                                        },
                                        fg,
                                    );
                                    if !disabled && b_resp.clicked() {
                                        clicked = true;
                                    }
                                    ui.add_space(10.0);
                                    ui.add(
                                        Label::new(
                                            RichText::new(disp(desc))
                                                .size(12.5)
                                                .color(theme::INK_SOFT),
                                        )
                                        .wrap(),
                                    );
                                });
                            });
                        ui.add_space(8.0);
                        clicked
                    };

                    if action_row(
                        &mut m_ui,
                        s.data_io_export_json,
                        s.data_io_export_json_desc,
                        true,
                        waiting_dialog,
                    ) {
                        trigger_export_json = true;
                    }
                    if action_row(
                        &mut m_ui,
                        s.data_io_export_md,
                        s.data_io_export_md_desc,
                        false,
                        waiting_dialog,
                    ) {
                        trigger_export_md = true;
                    }
                    if action_row(
                        &mut m_ui,
                        s.data_io_import_json,
                        s.data_io_import_json_desc,
                        false,
                        waiting_dialog,
                    ) {
                        trigger_import_json = true;
                    }

                    if waiting_dialog {
                        m_ui.add_space(4.0);
                        m_ui.label(
                            RichText::new(disp(s.data_io_waiting_dialog))
                                .size(12.5)
                                .color(theme::AMBER_INK),
                        );
                    }
                    if let Some(err) = &self.data_io_error {
                        m_ui.add_space(4.0);
                        m_ui.add(
                            Label::new(
                                RichText::new(disp(err))
                                    .size(12.5)
                                    .strong()
                                    .color(theme::RED_BAR),
                            )
                            .wrap(),
                        );
                    }
                }
            });

        if trigger_export_json {
            self.start_export_json_dialog(ctx);
        } else if trigger_export_md {
            self.start_export_markdown_dialog(ctx);
        } else if trigger_import_json {
            self.start_import_json_dialog(ctx);
        } else if do_confirm_import {
            let _ = self.confirm_pending_import(Some(ctx));
        } else if do_cancel_import {
            self.pending_import = None;
            self.data_io_error = None;
        } else if close_modal {
            self.show_data_io_modal = false;
            self.pending_import = None;
            self.data_io_error = None;
        }
    }
}
