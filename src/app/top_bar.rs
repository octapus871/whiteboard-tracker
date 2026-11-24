use super::*;

impl WhiteboardApp {
    /// Ultra-minimalist single-line responsive top bar divided into three strictly
    /// clipped, non-overlapping horizontal zones with a minimum `12.0px` buffer.
    ///
    /// Mirrors horizontally for Arabic RTL (`AppLang::Ar`):
    /// - Leading zone (Left in EN, Right in AR): tabs (`HOME | INDEX | HARBOR`),
    ///   profile selector pill `[ YYYY-MM-DD ▾ ]`, `AR | EN` toggle, and `+` Quick Entry.
    /// - Center zone: `‹ PAGE 1 / N ›` (EN) / `‹ صفحة 1 / N ›` (mirrored in AR).
    /// - Trailing zone (Right in EN, Left in AR): responsive localized clock/date +
    ///   window controls (`—`, `□`, `×`).
    /// Window dragging (`ViewportCommand::StartDrag`) is strictly limited to empty
    /// whitespace gaps between clusters so it never intercepts button clicks.
    pub(super) fn render_top_bar(&mut self, ui: &mut Ui) {
        const TOP_BAR_EDGE_PAD: f32 = 16.0;
        const TOP_BAR_ZONE_BUFFER: f32 = 12.0;

        let ctx = ui.ctx().clone();
        let bar_rect = ui.max_rect();
        let available_width = bar_rect.width();
        let content_rect = Rect::from_min_max(
            pos2(
                (bar_rect.left() + TOP_BAR_EDGE_PAD).min(bar_rect.center().x),
                bar_rect.top(),
            ),
            pos2(
                (bar_rect.right() - TOP_BAR_EDGE_PAD).max(bar_rect.center().x),
                bar_rect.bottom(),
            ),
        );
        let total_pages = self.total_pages();
        let lang = self.active_lang();
        let is_rtl = lang.is_rtl();
        let s = tr(lang);

        let mut any_control_hovered = false;
        let mut surrender_focus = false;

        // ── TRAILING ZONE (Right in EN, Left in AR): Window controls + responsive clock ──
        let show_full_clock = available_width >= 1050.0;
        let show_compact_time = (820.0..1050.0).contains(&available_width);
        let clock_zone_max_w = if show_full_clock {
            340.0
        } else if show_compact_time {
            210.0
        } else {
            120.0
        };
        let clock_zone_rect = if is_rtl {
            Rect::from_min_max(
                content_rect.left_top(),
                pos2(
                    (content_rect.left() + clock_zone_max_w).min(content_rect.right()),
                    content_rect.bottom(),
                ),
            )
        } else {
            Rect::from_min_max(
                pos2(
                    (content_rect.right() - clock_zone_max_w).max(content_rect.left()),
                    content_rect.top(),
                ),
                content_rect.right_bottom(),
            )
        };
        let mut clock_ui = ui.new_child(UiBuilder::new().max_rect(clock_zone_rect).layout(
            if is_rtl {
                Layout::left_to_right(Align::Center)
            } else {
                Layout::right_to_left(Align::Center)
            },
        ));
        clock_ui.set_clip_rect(clock_zone_rect);
        clock_ui.spacing_mut().item_spacing.x = 8.0;

        let close_btn = window_control(&mut clock_ui, "\u{d7}", &disp(s.tip_close));
        any_control_hovered |= close_btn.hovered();
        if close_btn.clicked() {
            self.force_save();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let max_btn = window_control(&mut clock_ui, "\u{25a1}", &disp(s.tip_maximize));
        any_control_hovered |= max_btn.hovered();
        if max_btn.clicked() {
            let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        let min_btn = window_control(&mut clock_ui, "\u{2014}", &disp(s.tip_minimize));
        any_control_hovered |= min_btn.hovered();
        if min_btn.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        let mut clock_edge = if is_rtl {
            min_btn.rect.right()
        } else {
            min_btn.rect.left()
        };
        if show_full_clock || show_compact_time {
            clock_ui.add_space(6.0);
            let (time, _date) = clock_strings_in_lang(lang);
            let time_resp = if is_rtl {
                clock_ui.label(
                    RichText::new(disp(&time))
                        .size(13.5)
                        .strong()
                        .color(theme::INK),
                )
            } else {
                clock_ui.label(mono(&time, 14.0).strong().color(theme::INK))
            };
            clock_edge = if is_rtl {
                clock_edge.max(time_resp.rect.right())
            } else {
                clock_edge.min(time_resp.rect.left())
            };
            if show_full_clock {
                let short_date = short_date_string_in_lang(lang);
                let date_resp = if is_rtl {
                    clock_ui.label(RichText::new(disp(&short_date)).size(12.5).color(theme::SLATE))
                } else {
                    clock_ui.label(mono(short_date, 12.5).color(theme::SLATE))
                };
                clock_edge = if is_rtl {
                    clock_edge.max(date_resp.rect.right())
                } else {
                    clock_edge.min(date_resp.rect.left())
                };
            }
        }
        clock_edge = if is_rtl {
            clock_edge.min(clock_zone_rect.right())
        } else {
            clock_edge.max(clock_zone_rect.left())
        };

        // ── LEADING ZONE (Left in EN, Right in AR): Tabs, profile pill, AR|EN, + ──
        let center_reserve = 152.0;
        let nav_zone_rect = if is_rtl {
            let nav_left = (clock_edge + TOP_BAR_ZONE_BUFFER * 2.0 + center_reserve)
                .min(content_rect.right() - 120.0)
                .max((clock_edge + TOP_BAR_ZONE_BUFFER).min(content_rect.right()));
            Rect::from_min_max(
                pos2(nav_left, content_rect.top()),
                content_rect.right_bottom(),
            )
        } else {
            let nav_right = (clock_edge - TOP_BAR_ZONE_BUFFER * 2.0 - center_reserve)
                .max(content_rect.left() + 120.0)
                .min((clock_edge - TOP_BAR_ZONE_BUFFER).max(content_rect.left()));
            Rect::from_min_max(
                content_rect.left_top(),
                pos2(nav_right, content_rect.bottom()),
            )
        };
        let mut nav_ui = ui.new_child(UiBuilder::new().max_rect(nav_zone_rect).layout(
            if is_rtl {
                Layout::right_to_left(Align::Center)
            } else {
                Layout::left_to_right(Align::Center)
            },
        ));
        nav_ui.set_clip_rect(nav_zone_rect);
        nav_ui.spacing_mut().item_spacing.x = 6.0;

        let raw_nav_edge = if self.focused_pane_id.is_some() {
            let back_resp = top_bar_tab(&mut nav_ui, &disp(s.back_btn), true);
            any_control_hovered |= back_resp.hovered();
            if back_resp.clicked() {
                self.focused_pane_id = None;
                surrender_focus = true;
            }
            let plus_resp = top_bar_quick_add_button(&mut nav_ui, self.show_quick_entry_modal)
                .on_hover_text(format!("{} (Ctrl+N)", disp(s.quick_entry_title)));
            any_control_hovered |= plus_resp.hovered();
            if plus_resp.clicked() {
                self.open_quick_entry_modal();
                surrender_focus = true;
            }
            if is_rtl {
                plus_resp.rect.left()
            } else {
                plus_resp.rect.right()
            }
        } else {
            let home_active = self.active_mode == AppMode::LiveBoard && !self.show_master_index;
            let index_active = self.active_mode == AppMode::LiveBoard && self.show_master_index;
            let harbor_active = self.active_mode == AppMode::Harbor;

            let home_resp = top_bar_tab(&mut nav_ui, &disp(s.tab_home), home_active);
            any_control_hovered |= home_resp.hovered();
            if home_resp.clicked() {
                self.active_mode = AppMode::LiveBoard;
                self.show_master_index = false;
                self.focused_pane_id = None;
                self.index_confirm_delete_pane_id = None;
                self.index_editing_pane_id = None;
                self.index_edit_backup_title = None;
                surrender_focus = true;
            }

            nav_ui.label(mono("|", 13.0).color(theme::DIVIDER));

            let index_resp = top_bar_tab(&mut nav_ui, &disp(s.tab_index), index_active);
            any_control_hovered |= index_resp.hovered();
            if index_resp.clicked() {
                self.active_mode = AppMode::LiveBoard;
                self.show_master_index = true;
                self.focused_pane_id = None;
                self.index_confirm_delete_pane_id = None;
                self.index_editing_pane_id = None;
                self.index_edit_backup_title = None;
                surrender_focus = true;
            }

            nav_ui.label(mono("|", 13.0).color(theme::DIVIDER));

            let harbor_resp = top_bar_tab(&mut nav_ui, &disp(s.tab_harbor), harbor_active);
            any_control_hovered |= harbor_resp.hovered();
            if harbor_resp.clicked() {
                self.active_mode = AppMode::Harbor;
                self.show_master_index = false;
                self.focused_pane_id = None;
                self.index_confirm_delete_pane_id = None;
                self.index_editing_pane_id = None;
                self.index_edit_backup_title = None;
                surrender_focus = true;
            }

            nav_ui.add_space(4.0);
            let date_str = self.active_profile_date().to_string();
            let pill_resp = profile_selector_pill(&mut nav_ui, &date_str, self.show_profiles_modal)
                .on_hover_text(disp(s.profiles_title));
            any_control_hovered |= pill_resp.hovered();
            if pill_resp.clicked() {
                self.data.sync_active_profile_from_current_day();
                self.show_profiles_modal = !self.show_profiles_modal;
                self.profile_confirm_delete_id = None;
                surrender_focus = true;
            }

            nav_ui.add_space(4.0);
            let lang_resp = lang_toggle_button(&mut nav_ui, lang)
                .on_hover_text(disp(s.tip_switch_lang));
            any_control_hovered |= lang_resp.hovered();
            if lang_resp.clicked() {
                self.set_language(lang.toggle());
                surrender_focus = true;
            }

            let plus_resp = top_bar_quick_add_button(&mut nav_ui, self.show_quick_entry_modal)
                .on_hover_text(format!("{} (Ctrl+N)", disp(s.quick_entry_title)));
            any_control_hovered |= plus_resp.hovered();
            if plus_resp.clicked() {
                self.open_quick_entry_modal();
                surrender_focus = true;
            }
            if is_rtl {
                plus_resp.rect.left()
            } else {
                plus_resp.rect.right()
            }
        };

        let (left_used_right, right_used_left) = if is_rtl {
            (clock_edge, raw_nav_edge.max(nav_zone_rect.left()))
        } else {
            (raw_nav_edge.min(nav_zone_rect.right()), clock_edge)
        };

        // ── CENTER ZONE: `‹ PAGE 1 / N ›` (mirrored in AR) centered strictly between Left & Right ──
        let avail_center_left = left_used_right + TOP_BAR_ZONE_BUFFER;
        let avail_center_right = right_used_left - TOP_BAR_ZONE_BUFFER;
        let avail_center_w = (avail_center_right - avail_center_left).max(0.0);
        let mut center_used_rect = Rect::from_min_max(
            pos2(avail_center_left, content_rect.top()),
            pos2(avail_center_left, content_rect.bottom()),
        );
        if avail_center_w > 8.0 {
            let center_w = 176.0_f32.min(avail_center_w);
            let min_cx = avail_center_left + center_w * 0.5;
            let max_cx = (avail_center_right - center_w * 0.5).max(min_cx);
            let center_x = bar_rect.center().x.clamp(min_cx, max_cx);
            let center_rect = Rect::from_center_size(
                pos2(center_x, content_rect.center().y),
                vec2(center_w, content_rect.height()),
            );
            let mut center_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(center_rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            center_ui.set_clip_rect(center_rect);
            center_used_rect = center_rect;
            center_ui.with_layout(
                Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    let row = ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let can_prev = self.current_page > 0;
                        let can_next = self.current_page + 1 < total_pages;

                        // Left chevron button: prev in EN (`‹`), next in AR (`‹`).
                        let (left_btn_enabled, left_tip) = if is_rtl {
                            (can_next, disp(s.tip_next_page))
                        } else {
                            (can_prev, disp(s.tip_prev_page))
                        };
                        let (left_rect, left_resp) = ui.allocate_exact_size(
                            vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                            Sense::click(),
                        );
                        let left_resp = left_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(left_tip);
                        any_control_hovered |= left_resp.hovered();
                        let left_color = if left_btn_enabled {
                            theme::INK
                        } else {
                            theme::MUTED
                        };
                        if left_btn_enabled && left_resp.hovered() {
                            ui.painter().rect_filled(
                                Rect::from_center_size(left_rect.center(), vec2(22.0, 22.0)),
                                CornerRadius::same(0),
                                theme::SEL_FILL,
                            );
                        }
                        if left_resp.has_focus() {
                            draw_focus_ring(ui.painter(), left_rect);
                        }
                        ui.painter().text(
                            left_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "‹",
                            FontId::monospace(16.0),
                            left_color,
                        );
                        if left_resp.clicked() && left_btn_enabled {
                            if is_rtl {
                                self.next_page();
                            } else {
                                self.prev_page();
                            }
                            surrender_focus = true;
                        }

                        let page_text = match lang {
                            AppLang::Ar => format!(
                                "{} {} / {}",
                                shape_arabic(s.page_word),
                                self.current_page + 1,
                                total_pages
                            ),
                            AppLang::En => {
                                format!("{} {} / {}", s.page_word, self.current_page + 1, total_pages)
                            }
                        };
                        let page_font = if is_rtl {
                            FontId::proportional(14.0)
                        } else {
                            FontId::monospace(13.5)
                        };
                        let page_text_w = ui
                            .fonts_mut(|f| {
                                f.layout_no_wrap(
                                    page_text.clone(),
                                    page_font.clone(),
                                    Color32::TRANSPARENT,
                                )
                            })
                            .size()
                            .x;
                        let (page_rect, page_resp) = ui.allocate_exact_size(
                            vec2((page_text_w + 10.0).max(74.0), MIN_HITBOX_SIZE),
                            Sense::click(),
                        );
                        let page_resp = page_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(disp(s.tip_page_cycle));
                        any_control_hovered |= page_resp.hovered();
                        if total_pages > 1 && page_resp.hovered() {
                            ui.painter().rect_filled(
                                Rect::from_center_size(
                                    page_rect.center(),
                                    vec2(page_rect.width(), 22.0),
                                ),
                                CornerRadius::same(0),
                                theme::SEL_FILL,
                            );
                        }
                        if page_resp.has_focus() {
                            draw_focus_ring(ui.painter(), page_rect);
                        }
                        ui.painter().text(
                            page_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &page_text,
                            page_font.clone(),
                            theme::INK,
                        );
                        ui.painter().text(
                            page_rect.center() + vec2(0.45, 0.0),
                            egui::Align2::CENTER_CENTER,
                            &page_text,
                            page_font,
                            theme::INK,
                        );
                        if page_resp.clicked() && total_pages > 1 {
                            self.current_page = (self.current_page + 1) % total_pages;
                            self.focused_pane_id = None;
                            surrender_focus = true;
                        }

                        // Right chevron button: next in EN (`›`), prev in AR (`›`).
                        let (right_btn_enabled, right_tip) = if is_rtl {
                            (can_prev, disp(s.tip_prev_page))
                        } else {
                            (can_next, disp(s.tip_next_page))
                        };
                        let (right_rect, right_resp) = ui.allocate_exact_size(
                            vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                            Sense::click(),
                        );
                        let right_resp = right_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(right_tip);
                        any_control_hovered |= right_resp.hovered();
                        let right_color = if right_btn_enabled {
                            theme::INK
                        } else {
                            theme::MUTED
                        };
                        if right_btn_enabled && right_resp.hovered() {
                            ui.painter().rect_filled(
                                Rect::from_center_size(right_rect.center(), vec2(22.0, 22.0)),
                                CornerRadius::same(0),
                                theme::SEL_FILL,
                            );
                        }
                        if right_resp.has_focus() {
                            draw_focus_ring(ui.painter(), right_rect);
                        }
                        ui.painter().text(
                            right_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "›",
                            FontId::monospace(16.0),
                            right_color,
                        );
                        if right_resp.clicked() && right_btn_enabled {
                            if is_rtl {
                                self.prev_page();
                            } else {
                                self.next_page();
                            }
                            surrender_focus = true;
                        }
                    });
                    center_used_rect = row.response.rect.intersect(center_rect);
                },
            );
        }

        // Window dragging strictly on EMPTY top-bar whitespace gaps only — never
        // overlapping any tab, profile pill, language toggle, page arrow, or window button.
        let gap_left_center = Rect::from_min_max(
            pos2(left_used_right + 4.0, bar_rect.top()),
            pos2(center_used_rect.left() - 4.0, bar_rect.bottom()),
        );
        let gap_center_right = Rect::from_min_max(
            pos2(center_used_rect.right() + 4.0, bar_rect.top()),
            pos2(right_used_left - 4.0, bar_rect.bottom()),
        );
        let mut start_drag = false;
        for (slot, gap_rect) in [(0usize, gap_left_center), (1usize, gap_center_right)] {
            if gap_rect.width() > 6.0 {
                let drag_resp = ui.interact(
                    gap_rect,
                    ui.id().with(("top_bar_drag_gap", slot)),
                    Sense::drag(),
                );
                if drag_resp.drag_started() && !any_control_hovered {
                    start_drag = true;
                }
            }
        }
        if start_drag {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if surrender_focus {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }

        // 1px black hairline separating the strip from the board.
        let rule = Rect::from_min_max(
            pos2(bar_rect.left(), bar_rect.bottom() - theme::HAIRLINE),
            pos2(bar_rect.right(), bar_rect.bottom()),
        );
        ui.painter()
            .rect_filled(rule, CornerRadius::same(0), theme::GRID);
    }
}

/// A minimal text window control (minimize / maximize / close) with a generous `28x28` hitbox.
pub(super) fn window_control(ui: &mut Ui, glyph: &str, tip: &str) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let color = if response.hovered() || response.has_focus() {
        theme::INK
    } else {
        theme::INK_SOFT
    };
    if response.has_focus() {
        draw_focus_ring(ui.painter(), rect);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::monospace(14.0),
        color,
    );
    response.on_hover_text(tip)
}

/// Single-click architectural top-bar navigation tab (`HOME`/`الرئيسية`, `INDEX`/`الفهرس`, `HARBOR`/`الأرشيف`)
/// with full-bar height hit target, `PointingHand` cursor, and crisp `2.0px`
/// solid `#0F172A` active bottom underline.
pub(super) fn top_bar_tab(ui: &mut Ui, text: &str, active: bool) -> egui::Response {
    let is_ar = crate::rtl_text::contains_arabic(text);
    let font = if is_ar {
        FontId::proportional(14.5)
    } else {
        FontId::monospace(14.0)
    };
    let text_w = ui
        .fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::TRANSPARENT))
        .size()
        .x;
    let tab_w = (text_w + 14.0).max(48.0);
    let tab_h = ui.available_height().max(32.0);
    let (rect, response) = ui.allocate_exact_size(vec2(tab_w, tab_h), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let color = if active || response.hovered() || response.has_focus() {
        theme::INK
    } else {
        theme::SLATE
    };
    if response.has_focus() {
        draw_focus_ring(ui.painter(), rect);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font.clone(),
        color,
    );
    if active {
        ui.painter().text(
            rect.center() + vec2(0.45, 0.0),
            egui::Align2::CENTER_CENTER,
            text,
            font,
            color,
        );
        let underline = Rect::from_min_max(
            pos2(rect.left() + 4.0, rect.bottom() - 2.0),
            pos2(rect.right() - 4.0, rect.bottom()),
        );
        ui.painter()
            .rect_filled(underline, CornerRadius::same(0), theme::INK);
    }
    response
}

/// Crisp single-click `AR | EN` language switcher in the top bar (`>=28px` hitbox,
/// `PointingHand` cursor, highlights the active language in bold `#0F172A`).
pub(super) fn lang_toggle_button(ui: &mut Ui, active: AppLang) -> egui::Response {
    let font = FontId::monospace(12.5);
    let btn_w = 58.0;
    let (hit_rect, resp) = ui.allocate_exact_size(
        vec2(btn_w, ui.available_height().max(MIN_HITBOX_SIZE)),
        Sense::click(),
    );
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let box_rect = Rect::from_center_size(hit_rect.center(), vec2(btn_w, 22.0));
    let bg = if resp.hovered() {
        theme::SEL_FILL
    } else {
        theme::PANE
    };
    ui.painter()
        .rect_filled(box_rect, CornerRadius::same(0), bg);
    ui.painter().rect_stroke(
        box_rect,
        CornerRadius::same(0),
        Stroke::new(1.0, theme::INK),
        StrokeKind::Inside,
    );
    if resp.has_focus() {
        draw_focus_ring(ui.painter(), hit_rect);
    }

    let ar_col = if active == AppLang::Ar {
        theme::INK
    } else {
        theme::MUTED
    };
    let en_col = if active == AppLang::En {
        theme::INK
    } else {
        theme::MUTED
    };
    let ar_pos = pos2(box_rect.left() + 14.0, box_rect.center().y);
    let sep_pos = box_rect.center();
    let en_pos = pos2(box_rect.right() - 14.0, box_rect.center().y);

    ui.painter()
        .text(ar_pos, egui::Align2::CENTER_CENTER, "AR", font.clone(), ar_col);
    if active == AppLang::Ar {
        ui.painter().text(
            ar_pos + vec2(0.45, 0.0),
            egui::Align2::CENTER_CENTER,
            "AR",
            font.clone(),
            ar_col,
        );
    }
    ui.painter().text(
        sep_pos,
        egui::Align2::CENTER_CENTER,
        "|",
        font.clone(),
        theme::DIVIDER,
    );
    ui.painter()
        .text(en_pos, egui::Align2::CENTER_CENTER, "EN", font.clone(), en_col);
    if active == AppLang::En {
        ui.painter().text(
            en_pos + vec2(0.45, 0.0),
            egui::Align2::CENTER_CENTER,
            "EN",
            font,
            en_col,
        );
    }
    resp
}

/// Crisp single-click `+` button in the top bar that opens Quick Task Entry anytime.
pub(super) fn top_bar_quick_add_button(ui: &mut Ui, open: bool) -> egui::Response {
    let (hit_rect, resp) = ui.allocate_exact_size(
        vec2(MIN_HITBOX_SIZE, ui.available_height().max(MIN_HITBOX_SIZE)),
        Sense::click(),
    );
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let box_rect = Rect::from_center_size(hit_rect.center(), vec2(24.0, 22.0));
    let (bg, fg) = if open || resp.hovered() {
        (theme::INK, theme::PANE)
    } else {
        (theme::PANE, theme::INK)
    };
    ui.painter()
        .rect_filled(box_rect, CornerRadius::same(0), bg);
    ui.painter().rect_stroke(
        box_rect,
        CornerRadius::same(0),
        Stroke::new(1.0, theme::INK),
        StrokeKind::Inside,
    );
    if resp.has_focus() {
        draw_focus_ring(ui.painter(), hit_rect);
    }
    ui.painter().text(
        box_rect.center(),
        egui::Align2::CENTER_CENTER,
        "+",
        FontId::monospace(15.0),
        fg,
    );
    resp
}

/// Compact single-click chronological profile selector pill `[ YYYY-MM-DD ▾ ]`
/// (`14.0px` SemiBold, `0.0px` sharp border, painter-drawn downward triangle).
pub(super) fn profile_selector_pill(ui: &mut Ui, date_str: &str, open: bool) -> egui::Response {
    let font = FontId::monospace(13.5);
    let label = format!("[ {}   ]", date_str);
    let text_w = ui
        .fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), Color32::TRANSPARENT))
        .size()
        .x;
    let pill_w = (text_w + 14.0).max(122.0);
    let (hit_rect, response) =
        ui.allocate_exact_size(vec2(pill_w, ui.available_height().max(MIN_HITBOX_SIZE)), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let box_rect = Rect::from_center_size(hit_rect.center(), vec2(pill_w, 24.0));
    let (bg, fg) = if open {
        (theme::INK, theme::PANE)
    } else if response.hovered() {
        (theme::SEL_FILL, theme::INK)
    } else {
        (theme::PANE, theme::INK)
    };
    ui.painter()
        .rect_filled(box_rect, CornerRadius::same(0), bg);
    ui.painter().rect_stroke(
        box_rect,
        CornerRadius::same(0),
        Stroke::new(if open { 1.5 } else { 1.0 }, theme::INK),
        StrokeKind::Inside,
    );
    if response.has_focus() {
        draw_focus_ring(ui.painter(), hit_rect);
    }
    ui.painter().text(
        pos2(box_rect.left() + 7.0, box_rect.center().y),
        egui::Align2::LEFT_CENTER,
        &label,
        font.clone(),
        fg,
    );
    ui.painter().text(
        pos2(box_rect.left() + 7.45, box_rect.center().y),
        egui::Align2::LEFT_CENTER,
        &label,
        font,
        fg,
    );
    // Crisp painter-drawn downward triangle inside the right bracket (never relies on Unicode ▾).
    let tri_cx = box_rect.right() - 18.0;
    let tri_cy = box_rect.center().y;
    ui.painter().add(Shape::convex_polygon(
        vec![
            pos2(tri_cx - 3.2, tri_cy - 1.5),
            pos2(tri_cx + 3.2, tri_cy - 1.5),
            pos2(tri_cx, tri_cy + 2.5),
        ],
        fg,
        Stroke::NONE,
    ));
    response
}
