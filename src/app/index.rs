use super::*;

impl WhiteboardApp {
    /// Full-canvas Swiss architectural Master Index view (`#FFFFFF`, `0.0px`):
    /// lists only main pane titles grouped into pages of 4 separated by `2.0px`
    /// solid `#0F172A` dividers (`PAGE 01`, `PAGE 02`, ...), with inline add,
    /// row selection + Up/Down keyboard reordering across pages, F2 / double-click /
    /// `تعديل` inline RTL rename (`Enter` saves, `Esc` cancels), `أرشفة`, and
    /// 2-click `حذف` confirmation.
    pub(super) fn render_master_index(&mut self, ui: &mut Ui) {
        let lang = self.active_lang();
        let s = tr(lang);
        let is_rtl = lang.is_rtl();
        let full_rect = ui.max_rect();
        ui.painter()
            .rect_filled(full_rect, CornerRadius::same(0), theme::PANE);

        let mut open_page: Option<usize> = None;
        let mut move_up: Option<usize> = None;
        let mut move_down: Option<usize> = None;
        let mut move_next_page: Option<usize> = None;
        let mut archive_idx: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;
        let mut add_title: Option<String> = None;
        let mut focus_add_input = false;
        let mut start_edit: Option<(String, String)> = None;
        let mut commit_edit = false;
        let mut dirty = false;

        let ctx = ui.ctx().clone();

        // Keyboard controls when a row is selected in INDEX (and no TextEdit is active):
        // Up / Down moves the selected main task across pages; F2 starts inline rename;
        // Enter opens that pane's page on HOME.
        if !ctx.wants_keyboard_input() && self.index_editing_pane_id.is_none() {
            if let Some(sel_id) = self.index_selected_pane_id.clone() {
                if let Some(sel_idx) = self
                    .data
                    .current_day
                    .panes
                    .iter()
                    .position(|p| p.id == sel_id)
                {
                    let (key_up, key_down, key_f2, key_enter) = ctx.input(|i| {
                        let no_mod = !i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.command;
                        (
                            no_mod && i.key_pressed(Key::ArrowUp),
                            no_mod && i.key_pressed(Key::ArrowDown),
                            no_mod && i.key_pressed(Key::F2),
                            no_mod && i.key_pressed(Key::Enter),
                        )
                    });
                    if key_up && sel_idx > 0 {
                        move_up = Some(sel_idx);
                    } else if key_down && sel_idx + 1 < self.data.current_day.panes.len() {
                        move_down = Some(sel_idx);
                    } else if key_f2 {
                        let backup = self.data.current_day.panes[sel_idx].title.clone();
                        start_edit = Some((sel_id, backup));
                    } else if key_enter {
                        open_page = Some(sel_idx / PANES_PER_PAGE);
                    }
                }
            }
        }

        let total_pages = self.total_pages();
        let pane_count = self.data.current_day.panes.len();
        let top_align = if is_rtl { Align::Max } else { Align::Min };
        let trailing_layout = if is_rtl {
            Layout::left_to_right(Align::Center)
        } else {
            Layout::right_to_left(Align::Center)
        };

        ScrollArea::vertical()
            .id_salt("master_index_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                let content_w = (avail_w - 64.0).clamp(360.0, 920.0);
                let side_pad = ((avail_w - content_w) * 0.5).max(16.0);

                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(side_pad);
                    ui.allocate_ui_with_layout(
                        vec2(content_w, 0.0),
                        Layout::top_down(top_align),
                        |ui| {
                            ui.set_min_width(content_w);

                            // Top inline TextEdit to add a new main pane on Enter.
                            let input_frame = Frame::new()
                                .fill(theme::PANE)
                                .stroke(Stroke::new(1.5, theme::INK))
                                .corner_radius(CornerRadius::same(0))
                                .inner_margin(Margin::symmetric(12, 6));
                            input_frame.show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                let add_resp = rtl_text_edit(
                                    ui,
                                    Id::new("master_index_new_pane_input"),
                                    &mut self.index_new_pane_input,
                                    s.index_new_pane_placeholder,
                                    16.0,
                                    f32::INFINITY,
                                    Margin::symmetric(4, 4),
                                    theme::INK,
                                    is_rtl,
                                )
                                .response;

                                let submitted = (add_resp.has_focus() || add_resp.lost_focus())
                                    && ui.input(|i| i.key_pressed(Key::Enter));
                                if submitted {
                                    let trimmed = self.index_new_pane_input.trim().to_string();
                                    if !trimmed.is_empty() {
                                        add_title = Some(trimmed);
                                        self.index_new_pane_input.clear();
                                        focus_add_input = true;
                                    }
                                }
                            });

                            ui.add_space(20.0);

                            if pane_count == 0 {
                                ui.add(
                                    Label::new(
                                        RichText::new(disp(s.index_empty_hint))
                                            .size(13.0)
                                            .color(theme::SLATE),
                                    )
                                    .selectable(false)
                                    .halign(top_align),
                                );
                                return;
                            }

                            for page_idx in 0..total_pages {
                                let start = page_idx * PANES_PER_PAGE;
                                let end = (start + PANES_PER_PAGE).min(pane_count);
                                if start >= end {
                                    break;
                                }

                                if page_idx > 0 {
                                    ui.add_space(18.0);
                                }

                                // Page group header (`PAGE 01` / `صفحة 01`) paired with 2.0px #0F172A divider.
                                dir_row(ui, 26.0, is_rtl, |ui| {
                                    let page_label = if is_rtl {
                                        format!("{} {:02}", shape_arabic(s.page_word), page_idx + 1)
                                    } else {
                                        format!("{} {:02}", s.page_word, page_idx + 1)
                                    };
                                    let label_rt = if is_rtl {
                                        RichText::new(&page_label)
                                            .size(14.5)
                                            .strong()
                                            .color(theme::INK)
                                    } else {
                                        mono(&page_label, 14.0).strong().color(theme::INK)
                                    };
                                    let page_resp = ui
                                        .add(
                                            Label::new(label_rt)
                                                .selectable(false)
                                                .sense(Sense::click()),
                                        )
                                        .on_hover_cursor(CursorIcon::PointingHand);
                                    if page_resp.clicked() {
                                        open_page = Some(page_idx);
                                    }
                                    if page_idx == self.current_page {
                                        let active_rt = if is_rtl {
                                            RichText::new(disp(s.action_active_badge))
                                                .size(12.0)
                                                .strong()
                                                .color(theme::GREEN_BAR)
                                        } else {
                                            mono(s.action_active_badge, 11.5)
                                                .strong()
                                                .color(theme::GREEN_BAR)
                                        };
                                        ui.label(active_rt);
                                    }
                                });

                                let (rule_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), 2.0),
                                    Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    rule_rect,
                                    CornerRadius::same(0),
                                    theme::INK,
                                );
                                ui.add_space(4.0);

                                for idx in start..end {
                                    let pane = &mut self.data.current_day.panes[idx];
                                    let pane_id = pane.id.clone();
                                    let (done, total) = pane.progress_counts();
                                    let is_selected = self.index_selected_pane_id.as_deref()
                                        == Some(pane_id.as_str());
                                    let is_editing = self.index_editing_pane_id.as_deref()
                                        == Some(pane_id.as_str());
                                    let is_confirming_del = self
                                        .index_confirm_delete_pane_id
                                        .as_deref()
                                        == Some(pane_id.as_str());

                                    let row_w = ui.available_width();
                                    let (row_rect, row_bg_resp) =
                                        ui.allocate_exact_size(vec2(row_w, 46.0), Sense::click());
                                    if is_selected {
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
                                    if row_bg_resp.clicked() {
                                        self.index_selected_pane_id = Some(pane_id.clone());
                                        self.index_confirm_delete_pane_id = None;
                                    }

                                    let row_inner = row_rect.shrink2(vec2(8.0, 2.0));
                                    let row_layout = if is_rtl {
                                        Layout::right_to_left(Align::Center)
                                    } else {
                                        Layout::left_to_right(Align::Center)
                                    };
                                    let mut row_ui = ui.new_child(
                                        UiBuilder::new().max_rect(row_inner).layout(row_layout),
                                    );
                                    row_ui.set_clip_rect(row_inner);
                                    row_ui.spacing_mut().item_spacing.x = 10.0;

                                    // Leading: monospace index `[0N]`.
                                    row_ui.label(
                                        mono(format!("[{:02}]", idx + 1), 14.0)
                                            .strong()
                                            .color(theme::INK_SOFT),
                                    );

                                    // Title region: single-click selects row, double-click edits inline.
                                    let controls_w = if is_rtl {
                                        if is_confirming_del { 356.0 } else { 340.0 }
                                    } else if is_confirming_del {
                                        398.0
                                    } else {
                                        382.0
                                    };
                                    let title_w = (row_ui.available_width() - controls_w).max(100.0);

                                    if is_editing {
                                        let edit_resp = rtl_text_edit(
                                            &mut row_ui,
                                            Id::new(("index_pane_title_edit", pane_id.as_str())),
                                            &mut pane.title,
                                            s.task_title_placeholder,
                                            18.0,
                                            title_w,
                                            Margin::symmetric(6, 4),
                                            theme::INK,
                                            is_rtl,
                                        )
                                        .response;
                                        if self.index_focus_edit_id.as_deref()
                                            == Some(pane_id.as_str())
                                        {
                                            edit_resp.request_focus();
                                            self.index_focus_edit_id = None;
                                        }
                                        if edit_resp.changed() {
                                            dirty = true;
                                        }
                                        let enter_pressed = (edit_resp.has_focus()
                                            || edit_resp.lost_focus())
                                            && row_ui.input(|i| i.key_pressed(Key::Enter));
                                        if enter_pressed || edit_resp.lost_focus() {
                                            commit_edit = true;
                                        }
                                    } else {
                                        let title_display = if pane.title.trim().is_empty() {
                                            disp(s.untitled)
                                        } else {
                                            disp(&pane.title)
                                        };
                                        let title_resp = row_ui
                                            .add_sized(
                                                vec2(title_w, 34.0),
                                                Label::new(
                                                    RichText::new(title_display)
                                                        .size(18.0)
                                                        .strong()
                                                        .color(theme::INK),
                                                )
                                                .selectable(false)
                                                .sense(Sense::click())
                                                .halign(top_align)
                                                .truncate(),
                                            )
                                            .on_hover_cursor(CursorIcon::PointingHand);
                                        if title_resp.double_clicked() {
                                            self.index_selected_pane_id = Some(pane_id.clone());
                                            self.index_confirm_delete_pane_id = None;
                                            start_edit =
                                                Some((pane_id.clone(), pane.title.clone()));
                                        } else if title_resp.clicked() {
                                            self.index_selected_pane_id = Some(pane_id.clone());
                                            self.index_confirm_delete_pane_id = None;
                                        }
                                    }

                                    // Trailing controls cluster.
                                    row_ui.with_layout(trailing_layout, |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;

                                        // 1. Delete (`حذف` / `DELETE` -> 2-click confirm).
                                        if is_confirming_del {
                                            let w = if is_rtl { 54.0 } else { 68.0 };
                                            let (confirm_rect, confirm_resp) =
                                                ui.allocate_exact_size(
                                                    vec2(w, MIN_HITBOX_SIZE),
                                                    Sense::click(),
                                                );
                                            let confirm_resp = confirm_resp
                                                .on_hover_cursor(CursorIcon::PointingHand);
                                            let pill = Rect::from_center_size(
                                                confirm_rect.center(),
                                                vec2(w - 2.0, 22.0),
                                            );
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
                                            if confirm_resp.clicked() {
                                                delete_idx = Some(idx);
                                            }
                                        } else {
                                            let del_resp = index_text_action(
                                                ui,
                                                s.action_delete,
                                                if is_rtl { 38.0 } else { 52.0 },
                                                theme::SLATE,
                                                theme::RED_BAR,
                                            );
                                            if del_resp.clicked() {
                                                self.index_selected_pane_id =
                                                    Some(pane_id.clone());
                                                self.index_confirm_delete_pane_id =
                                                    Some(pane_id.clone());
                                            }
                                        }

                                        // 2. Archive (`أرشفة` / `ARCHIVE`).
                                        let arch_resp = index_text_action(
                                            ui,
                                            s.action_archive,
                                            if is_rtl { 46.0 } else { 58.0 },
                                            theme::SLATE,
                                            theme::INK,
                                        );
                                        if arch_resp.clicked() {
                                            self.index_confirm_delete_pane_id = None;
                                            archive_idx = Some(idx);
                                        }

                                        // 3. Inline rename (`تعديل` / `EDIT`).
                                        let edit_btn = index_text_action(
                                            ui,
                                            s.action_edit,
                                            if is_rtl { 44.0 } else { 42.0 },
                                            if is_editing { theme::INK } else { theme::SLATE },
                                            theme::INK,
                                        );
                                        if edit_btn.clicked() {
                                            self.index_selected_pane_id = Some(pane_id.clone());
                                            self.index_confirm_delete_pane_id = None;
                                            if is_editing {
                                                commit_edit = true;
                                            } else {
                                                start_edit =
                                                    Some((pane_id.clone(), pane.title.clone()));
                                            }
                                        }

                                        // 4. Reorder up / down (painter-drawn vectors, 28x28 hitboxes).
                                        let can_up = idx > 0;
                                        let can_down = idx + 1 < pane_count;

                                        let up_resp = index_arrow_button(ui, true, can_up);
                                        if up_resp.clicked() && can_up {
                                            self.index_selected_pane_id = Some(pane_id.clone());
                                            move_up = Some(idx);
                                            self.index_confirm_delete_pane_id = None;
                                        }

                                        let down_resp = index_arrow_button(ui, false, can_down);
                                        if down_resp.clicked() && can_down {
                                            self.index_selected_pane_id = Some(pane_id.clone());
                                            move_down = Some(idx);
                                            self.index_confirm_delete_pane_id = None;
                                        }

                                        // 5. Page assignment badge `[P1]` / `[P2]`.
                                        let p_badge = format!("[P{}]", page_idx + 1);
                                        let (hit_rect, badge_resp) = ui.allocate_exact_size(
                                            vec2(40.0, MIN_HITBOX_SIZE),
                                            Sense::click(),
                                        );
                                        let badge_resp =
                                            badge_resp.on_hover_cursor(CursorIcon::PointingHand);
                                        let visual_badge = Rect::from_center_size(
                                            hit_rect.center(),
                                            vec2(38.0, 22.0),
                                        );
                                        let badge_bg = if badge_resp.hovered() {
                                            theme::HINT
                                        } else {
                                            theme::SEL_FILL
                                        };
                                        ui.painter().rect_filled(
                                            visual_badge,
                                            CornerRadius::same(0),
                                            badge_bg,
                                        );
                                        ui.painter().rect_stroke(
                                            visual_badge,
                                            CornerRadius::same(0),
                                            Stroke::new(theme::HAIRLINE, theme::INK),
                                            StrokeKind::Inside,
                                        );
                                        ui.painter().text(
                                            visual_badge.center(),
                                            egui::Align2::CENTER_CENTER,
                                            &p_badge,
                                            FontId::monospace(12.5),
                                            theme::INK,
                                        );
                                        if badge_resp.clicked() {
                                            self.index_selected_pane_id = Some(pane_id.clone());
                                            move_next_page = Some(idx);
                                            self.index_confirm_delete_pane_id = None;
                                        }

                                        // 6. Completion badge (`done/total`).
                                        let done_color = if total > 0 && done == total {
                                            theme::GREEN_BAR
                                        } else {
                                            theme::INK_SOFT
                                        };
                                        ui.label(
                                            mono(format!("{}/{}", done, total), 13.5)
                                                .strong()
                                                .color(done_color),
                                        );
                                    });

                                    // Crisp 1.0px subtle row separator between panes inside a page.
                                    if idx + 1 < end {
                                        let (row_rule, _) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), 1.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            row_rule,
                                            CornerRadius::same(0),
                                            theme::HINT,
                                        );
                                    }
                                }
                            }

                            ui.add_space(28.0);
                        },
                    );
                });
            });

        if let Some((pane_id, backup_title)) = start_edit {
            self.index_editing_pane_id = Some(pane_id.clone());
            self.index_edit_backup_title = Some(backup_title);
            self.index_focus_edit_id = Some(pane_id);
        } else if commit_edit {
            self.index_editing_pane_id = None;
            self.index_edit_backup_title = None;
        }
        if dirty {
            self.mark_dirty();
        }
        if let Some(title) = add_title {
            self.add_main_pane(&title);
            if focus_add_input {
                let id = Id::new("master_index_new_pane_input");
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
        if let Some(idx) = move_up {
            self.move_pane_up(idx);
        }
        if let Some(idx) = move_down {
            self.move_pane_down(idx);
        }
        if let Some(idx) = move_next_page {
            self.move_pane_to_next_page(idx);
        }
        if let Some(idx) = archive_idx {
            self.archive_pane_at(idx, Some(&ctx));
        }
        if let Some(idx) = delete_idx {
            self.delete_pane_at(idx, Some(&ctx));
        }
        if let Some(page) = open_page {
            self.current_page = page.min(self.total_pages().saturating_sub(1));
            self.show_master_index = false;
            self.focused_pane_id = None;
            self.index_confirm_delete_pane_id = None;
            self.index_editing_pane_id = None;
            self.index_edit_backup_title = None;
        }
    }
}

/// Crisp `0.0px` painter-drawn vector arrow button (`up = true` for `↑`, `false`
/// for `↓`) used for row reordering in `INDEX`. Never relies on Unicode arrow
/// font glyphs and allocates a generous `28.0 × 28.0` hitbox.
pub(super) fn index_arrow_button(ui: &mut Ui, up: bool, enabled: bool) -> egui::Response {
    let (hit_rect, resp) =
        ui.allocate_exact_size(vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE), Sense::click());
    let resp = if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    };
    let visual_rect = Rect::from_center_size(hit_rect.center(), vec2(22.0, 22.0));
    let bg = if enabled && resp.hovered() {
        theme::SEL_FILL
    } else {
        theme::PANE
    };
    let fg = if !enabled {
        theme::HINT
    } else if resp.hovered() {
        theme::INK
    } else {
        theme::INK_SOFT
    };
    let border = if enabled { theme::GRID } else { theme::HINT };
    ui.painter()
        .rect_filled(visual_rect, CornerRadius::same(0), bg);
    ui.painter().rect_stroke(
        visual_rect,
        CornerRadius::same(0),
        Stroke::new(theme::HAIRLINE, border),
        StrokeKind::Inside,
    );
    let c = visual_rect.center();
    let stroke = Stroke::new(1.4, fg);
    // Vertical shaft + crisp V arrowhead.
    ui.painter()
        .line_segment([pos2(c.x, c.y - 4.5), pos2(c.x, c.y + 4.5)], stroke);
    if up {
        ui.painter()
            .line_segment([pos2(c.x, c.y - 4.5), pos2(c.x - 3.5, c.y - 1.0)], stroke);
        ui.painter()
            .line_segment([pos2(c.x, c.y - 4.5), pos2(c.x + 3.5, c.y - 1.0)], stroke);
    } else {
        ui.painter()
            .line_segment([pos2(c.x, c.y + 4.5), pos2(c.x - 3.5, c.y + 1.0)], stroke);
        ui.painter()
            .line_segment([pos2(c.x, c.y + 4.5), pos2(c.x + 3.5, c.y + 1.0)], stroke);
    }
    resp
}

/// Crisp Arabic text action button (`حذف`, `أرشفة`, `تعديل`, `استعادة`) with a
/// generous `>=28px`-tall hitbox and zero missing-glyph boxes.
pub(super) fn index_text_action(
    ui: &mut Ui,
    label: &str,
    width: f32,
    idle_color: Color32,
    hover_color: Color32,
) -> egui::Response {
    let (hit_rect, resp) =
        ui.allocate_exact_size(vec2(width.max(MIN_HITBOX_SIZE), MIN_HITBOX_SIZE), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if resp.hovered() {
        let pill = Rect::from_center_size(hit_rect.center(), vec2(width, 22.0));
        ui.painter()
            .rect_filled(pill, CornerRadius::same(0), theme::SEL_FILL);
    }
    let color = if resp.hovered() {
        hover_color
    } else {
        idle_color
    };
    ui.painter().text(
        hit_rect.center(),
        egui::Align2::CENTER_CENTER,
        disp(label),
        if crate::rtl_text::contains_arabic(label) {
            FontId::proportional(13.0)
        } else {
            FontId::monospace(11.5)
        },
        color,
    );
    resp
}
