use super::*;

impl WhiteboardApp {
    pub(super) fn render_harbor_placeholder(&mut self, ui: &mut Ui) {
        fn node_matches_query(node: &TaskNode, q: &str) -> bool {
            node.title.to_lowercase().contains(q)
                || node.notes.to_lowercase().contains(q)
                || node.children.iter().any(|c| node_matches_query(c, q))
        }
        fn pane_matches_query(pane: &MainTaskPane, q: &str) -> bool {
            pane.title.to_lowercase().contains(q)
                || pane.subtasks.iter().any(|n| node_matches_query(n, q))
        }

        let lang = self.active_lang();
        let s = tr(lang);
        let is_rtl = lang.is_rtl();
        let full_rect = ui.max_rect();
        ui.painter()
            .rect_filled(full_rect, CornerRadius::same(0), theme::PANE);

        if self.data.archived_panes.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                Frame::new()
                    .fill(theme::PANE)
                    .stroke(Stroke::new(theme::GRID_LINE, theme::GRID))
                    .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
                    .inner_margin(Margin::same(28))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(disp(s.harbor_title))
                                .size(20.0)
                                .strong()
                                .color(theme::INK),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(disp(s.harbor_empty))
                                .size(12.5)
                                .color(theme::TEXT_MUTED),
                        );
                    });
            });
            return;
        }

        let top_align = if is_rtl { Align::Max } else { Align::Min };
        let trailing_layout = if is_rtl {
            Layout::left_to_right(Align::Center)
        } else {
            Layout::right_to_left(Align::Center)
        };
        let mut restore_idx: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;
        let mut arm_delete_id: Option<String> = None;
        let mut clear_search = false;

        ScrollArea::vertical()
            .id_salt("harbor_archive_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                let content_w = (avail_w - 64.0).clamp(320.0, 860.0);
                let side_pad = ((avail_w - content_w) * 0.5).max(16.0);
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(side_pad);
                    ui.allocate_ui_with_layout(
                        vec2(content_w, 0.0),
                        Layout::top_down(top_align),
                        |ui| {
                            ui.set_min_width(content_w);
                            ui.label(
                                RichText::new(disp(s.harbor_title))
                                    .size(18.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.add_space(8.0);
                            let (rule, _) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 2.0),
                                Sense::hover(),
                            );
                            ui.painter()
                                .rect_filled(rule, CornerRadius::same(0), theme::INK);
                            ui.add_space(10.0);

                            // Search box filtering archived panes by title, subtasks, and notes.
                            Frame::new()
                                .fill(theme::DRAWER_BG)
                                .stroke(Stroke::new(1.0, theme::INK))
                                .corner_radius(CornerRadius::same(0))
                                .inner_margin(Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    dir_row(ui, 28.0, is_rtl, |ui| {
                                        let clear_reserve =
                                            if self.harbor_search_query.is_empty() { 0.0 } else { 32.0 };
                                        let search_w =
                                            (ui.available_width() - clear_reserve).max(120.0);
                                        let _ = rtl_text_edit(
                                            ui,
                                            Id::new("harbor_search_input"),
                                            &mut self.harbor_search_query,
                                            s.harbor_search_placeholder,
                                            14.0,
                                            search_w,
                                            Margin::symmetric(4, 2),
                                            theme::INK,
                                            is_rtl,
                                        );
                                        if !self.harbor_search_query.is_empty() {
                                            ui.with_layout(trailing_layout, |ui| {
                                                let (cl_rect, cl_resp) = ui.allocate_exact_size(
                                                    vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                                                    Sense::click(),
                                                );
                                                let cl_resp = cl_resp
                                                    .on_hover_cursor(CursorIcon::PointingHand);
                                                ui.painter().text(
                                                    cl_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "\u{d7}",
                                                    FontId::monospace(15.0),
                                                    if cl_resp.hovered() {
                                                        theme::INK
                                                    } else {
                                                        theme::SLATE
                                                    },
                                                );
                                                if cl_resp.clicked() {
                                                    clear_search = true;
                                                }
                                            });
                                        }
                                    });
                                });
                            ui.add_space(10.0);

                            let query_lower = self.harbor_search_query.trim().to_lowercase();
                            let matching_indices: Vec<usize> = self
                                .data
                                .archived_panes
                                .iter()
                                .enumerate()
                                .filter(|(_, pane)| {
                                    query_lower.is_empty()
                                        || pane_matches_query(pane, &query_lower)
                                })
                                .map(|(idx, _)| idx)
                                .collect();

                            if matching_indices.is_empty() {
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new(disp(s.harbor_no_matches))
                                        .size(13.0)
                                        .color(theme::TEXT_MUTED),
                                );
                            }

                            for (pos, idx) in matching_indices.iter().copied().enumerate() {
                                let pane = &self.data.archived_panes[idx];
                                let pane_id = pane.id.clone();
                                let (done, total) = pane.progress_counts();
                                let is_confirming_del = self
                                    .harbor_confirm_delete_pane_id
                                    .as_deref()
                                    == Some(pane_id.as_str());

                                dir_row(ui, 42.0, is_rtl, |ui| {
                                    ui.label(
                                        RichText::new(disp(&pane.title))
                                            .size(16.0)
                                            .strong()
                                            .color(theme::INK),
                                    );
                                    ui.with_layout(trailing_layout, |ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;

                                        // 1. 2-click permanent delete (`DELETE` → `CONFIRM?`).
                                        if is_confirming_del {
                                            let (c_rect, c_resp) = ui.allocate_exact_size(
                                                vec2(66.0, MIN_HITBOX_SIZE),
                                                Sense::click(),
                                            );
                                            let c_resp =
                                                c_resp.on_hover_cursor(CursorIcon::PointingHand);
                                            let pill = Rect::from_center_size(
                                                c_rect.center(),
                                                vec2(64.0, 22.0),
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
                                                arm_delete_id = Some(pane_id.clone());
                                            }
                                        }

                                        // 2. Restore (`استعادة` / `RESTORE`).
                                        let r = index_text_action(
                                            ui,
                                            s.action_restore,
                                            if is_rtl { 56.0 } else { 62.0 },
                                            theme::INK_SOFT,
                                            theme::INK,
                                        );
                                        if r.clicked() {
                                            restore_idx = Some(idx);
                                        }

                                        // 3. Leaf progress badge (`done/total`).
                                        ui.label(
                                            mono(format!("{}/{}", done, total), 13.0)
                                                .color(theme::SLATE),
                                        );
                                    });
                                });

                                if pos + 1 < matching_indices.len() {
                                    let (sep, _) = ui.allocate_exact_size(
                                        vec2(ui.available_width(), 1.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        sep,
                                        CornerRadius::same(0),
                                        theme::SUBTLE_RULE,
                                    );
                                }
                            }
                        },
                    );
                });
            });

        if clear_search {
            self.harbor_search_query.clear();
        }
        if let Some(id) = arm_delete_id {
            self.harbor_confirm_delete_pane_id = Some(id);
        }
        if let Some(idx) = delete_idx {
            self.delete_archived_pane(idx);
        } else if let Some(idx) = restore_idx {
            self.restore_archived_pane(idx);
        }
    }
}
