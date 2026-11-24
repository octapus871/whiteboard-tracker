use super::*;

impl WhiteboardApp {
    /// Finds the pane ID that owns the given task node anywhere in its tree.
    pub(super) fn find_pane_id_for_node(&self, node_id: &str) -> Option<String> {
        fn contains(nodes: &[TaskNode], target_id: &str) -> bool {
            nodes
                .iter()
                .any(|n| n.id == target_id || contains(&n.children, target_id))
        }
        self.data
            .current_day
            .panes
            .iter()
            .find(|p| contains(&p.subtasks, node_id))
            .map(|p| p.id.clone())
    }

    /// Opens the embedded bottom split note drawer inside the node's owning pane.
    pub fn open_note_drawer(&mut self, node_id: String, focus_editor: bool) {
        if let Some(pane_id) = self.find_pane_id_for_node(&node_id) {
            self.pane_note_drawers.insert(pane_id, node_id.clone());
            if focus_editor {
                self.editing_node_id = None;
                self.focus_notes_editor = Some(node_id);
            }
        }
    }

    /// Closes the embedded note drawer for a single pane and surrenders editor focus.
    pub fn close_note_drawer_for_pane(&mut self, pane_id: &str, ctx: Option<&egui::Context>) {
        if let Some(node_id) = self.pane_note_drawers.remove(pane_id) {
            if self.focus_notes_editor.as_deref() == Some(node_id.as_str()) {
                self.focus_notes_editor = None;
            }
            if let Some(ctx) = ctx {
                let editor_id = Id::new(("pane_notes_editor", node_id.as_str()));
                ctx.memory_mut(|m| {
                    if m.has_focus(editor_id) {
                        m.surrender_focus(editor_id);
                    }
                });
            }
        }
    }

    /// Closes all open embedded note drawers and surrenders any focused drawer editor.
    pub fn close_all_note_drawers(&mut self, ctx: Option<&egui::Context>) {
        let editor_ids: Vec<Id> = self
            .pane_note_drawers
            .values()
            .map(|node_id| Id::new(("pane_notes_editor", node_id.as_str())))
            .collect();
        self.pane_note_drawers.clear();
        self.focus_notes_editor = None;
        if let Some(ctx) = ctx {
            if !editor_ids.is_empty() {
                ctx.memory_mut(|m| {
                    for editor_id in editor_ids {
                        if m.has_focus(editor_id) {
                            m.surrender_focus(editor_id);
                        }
                    }
                });
            }
        }
    }
}

/// Finds a mutable task node inside a slice of subtask trees.
pub(super) fn find_node_in_slice_mut<'a>(
    nodes: &'a mut [TaskNode],
    node_id: &str,
) -> Option<&'a mut TaskNode> {
    for node in nodes {
        if node.id == node_id {
            return Some(node);
        }
        if let Some(found) = find_node_in_slice_mut(&mut node.children, node_id) {
            return Some(found);
        }
    }
    None
}

/// Embedded light-mode Swiss architectural note card (`0.0px` corners, `#F8FAFC`
/// paper fill, `1.5px` `#0F172A` border, `3.0px` solid `#0F172A` offset shadow,
/// minimalist code/text-block header with `NOTE` tag, `COPY`, and vector `X`
/// close button, plus a frameless `16.0px`-padded RTL writing canvas).
pub(super) fn render_pane_note_drawer(
    ui: &mut Ui,
    rect: Rect,
    pane_id: &str,
    node: &mut TaskNode,
    lang: AppLang,
    focus_notes_editor: &mut Option<String>,
    actions: &mut PaneActions,
) {
    let s = tr(lang);
    let is_rtl = lang.is_rtl();
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(0), theme::PANE);

    let card_rect = Rect::from_min_max(
        pos2(rect.left() + 16.0, rect.top() + 6.0),
        pos2(rect.right() - 16.0, (rect.bottom() - 12.0).max(rect.top() + 64.0)),
    );

    // 1. Solid 3.0px #0F172A hard offset shadow behind the note card.
    painter.rect_filled(
        card_rect.translate(SHADOW_OFFSET),
        CornerRadius::same(0),
        theme::INK,
    );
    // 2. Inverted light-mode paper container (`#F8FAFC` fill, `1.5px` `#0F172A` border, `0.0px` corners).
    painter.rect_filled(card_rect, CornerRadius::same(0), theme::DRAWER_BG);
    painter.rect_stroke(
        card_rect,
        CornerRadius::same(0),
        Stroke::new(1.5, theme::INK),
        StrokeKind::Inside,
    );

    let inner_rect = card_rect.shrink(1.5);
    let top_align = if is_rtl { Align::Max } else { Align::Min };
    let mut card_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(inner_rect)
            .layout(Layout::top_down(top_align)),
    );
    card_ui.set_clip_rect(inner_rect);

    let node_id = node.id.clone();
    let now = ui.ctx().input(|i| i.time);
    let copy_time_id = Id::new(("note_copy_feedback", node_id.as_str()));
    let copied_until: f64 = ui
        .ctx()
        .data(|d| d.get_temp::<f64>(copy_time_id).unwrap_or(0.0));
    let show_copied = now < copied_until;

    // Minimalist top action header (`36.0px`), mirrored cleanly between RTL and LTR.
    let header_rect = Rect::from_min_size(inner_rect.left_top(), vec2(inner_rect.width(), 36.0));
    let header_inner = Rect::from_min_max(
        pos2(header_rect.left() + 14.0, header_rect.top()),
        pos2(header_rect.right() - 16.0, header_rect.bottom()),
    );
    let header_layout = if is_rtl {
        Layout::right_to_left(Align::Center)
    } else {
        Layout::left_to_right(Align::Center)
    };
    let trailing_layout = if is_rtl {
        Layout::left_to_right(Align::Center)
    } else {
        Layout::right_to_left(Align::Center)
    };
    let mut header_ui = card_ui.new_child(
        UiBuilder::new()
            .max_rect(header_inner)
            .layout(header_layout),
    );
    header_ui.set_clip_rect(header_inner);

    let controls_reserve = 172.0;
    let title_width = (header_inner.width() - controls_reserve).max(40.0);
    let branch_title = if node.title.trim().is_empty() {
        s.untitled
    } else {
        node.title.trim()
    };
    let title_font = FontId::proportional(15.5);
    let title_shaped = disp(branch_title);
    let (title_rect, _) =
        header_ui.allocate_exact_size(vec2(title_width, 32.0), Sense::hover());
    let title_galley = header_ui.fonts_mut(|f| {
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = title_width;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.halign = if is_rtl { Align::RIGHT } else { Align::LEFT };
        job.append(
            &title_shaped,
            0.0,
            egui::TextFormat {
                font_id: title_font,
                color: theme::INK,
                ..Default::default()
            },
        );
        f.layout_job(job)
    });
    let title_pos = pos2(
        if is_rtl {
            title_rect.right()
        } else {
            title_rect.left()
        },
        title_rect.center().y - title_galley.size().y * 0.5,
    );
    let title_painter = header_ui.painter_at(title_rect);
    title_painter.galley(title_pos, title_galley.clone(), theme::INK);
    title_painter.galley(title_pos + vec2(0.45, 0.0), title_galley, theme::INK);

    // Trailing action cluster (`X` close, `COPY`/`نسخ`, `NOTE`/`ملاحظات`).
    header_ui.with_layout(trailing_layout, |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;

        // 1. Crisp 10px × 10px painter-drawn X close button (28x28 hitbox).
        let (close_rect, close_response) =
            ui.allocate_exact_size(vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE), Sense::click());
        let close_response = close_response.on_hover_cursor(CursorIcon::PointingHand);
        let close_color = if close_response.hovered() {
            theme::INK
        } else {
            theme::INK_SOFT
        };
        let c = close_rect.center();
        let half = 5.0;
        let x_stroke = Stroke::new(1.6, close_color);
        ui.painter().line_segment(
            [pos2(c.x - half, c.y - half), pos2(c.x + half, c.y + half)],
            x_stroke,
        );
        ui.painter().line_segment(
            [pos2(c.x - half, c.y + half), pos2(c.x + half, c.y - half)],
            x_stroke,
        );
        if close_response.clicked() {
            actions.close_drawer_pane = Some(pane_id.to_string());
        }
        if close_response.has_focus() {
            draw_focus_ring(ui.painter(), close_rect);
        }
        close_response.on_hover_text(disp(s.tip_close_esc));

        // 2. Crisp Copy button (`COPY`/`COPIED` or `نسخ`/`تم النسخ`, >=28px hitbox).
        let copy_label = if show_copied { s.copied_btn } else { s.copy_btn };
        let copy_w = if show_copied { 58.0 } else { 44.0 };
        let (copy_rect, copy_response) =
            ui.allocate_exact_size(vec2(copy_w, MIN_HITBOX_SIZE), Sense::click());
        let copy_response = copy_response.on_hover_cursor(CursorIcon::PointingHand);
        let copy_color = if show_copied {
            theme::GREEN_BAR
        } else if copy_response.hovered() {
            theme::INK
        } else {
            theme::INK_SOFT
        };
        if copy_response.hovered() {
            let pill = Rect::from_center_size(copy_rect.center(), vec2(copy_w, 22.0));
            ui.painter()
                .rect_filled(pill, CornerRadius::same(0), theme::SEL_FILL);
        }
        ui.painter().text(
            copy_rect.center(),
            egui::Align2::CENTER_CENTER,
            disp(copy_label),
            if is_rtl {
                FontId::proportional(12.5)
            } else {
                FontId::monospace(11.5)
            },
            copy_color,
        );
        if copy_response.clicked() {
            ui.ctx().copy_text(node.notes.clone());
            ui.ctx()
                .data_mut(|d| d.insert_temp(copy_time_id, now + 1.5));
        }

        // 3. Subtle `NOTE` / `ملاحظات` tag (`12.0px` `#64748B`).
        if is_rtl {
            ui.label(RichText::new(disp(s.note_tag)).size(12.5).color(theme::SLATE));
        } else {
            ui.label(mono(s.note_tag, 12.0).color(theme::SLATE));
        }
    });

    // Advance card_ui cursor past header and draw the crisp `1.0px` `#E2E8F0` hairline.
    card_ui.allocate_exact_size(vec2(inner_rect.width(), 36.0), Sense::hover());
    let (div_rect, _) = card_ui.allocate_exact_size(
        Vec2::new(inner_rect.width(), 1.0),
        Sense::hover(),
    );
    card_ui
        .painter()
        .rect_filled(div_rect, CornerRadius::same(0), theme::SUBTLE_RULE);

    // Frameless distraction-free writing canvas (`16.0px` inner padding, `16.0px` font),
    // RTL-aligned in Arabic and LTR-aligned in English.
    let body_h = card_ui.available_height().max(32.0);
    ScrollArea::vertical()
        .id_salt(("pane_drawer_scroll", pane_id, node_id.as_str()))
        .auto_shrink([false, false])
        .max_height(body_h)
        .show(&mut card_ui, |ui| {
            let editor_output = rtl_multiline_text_edit(
                ui,
                Id::new(("pane_notes_editor", node_id.as_str())),
                &mut node.notes,
                s.note_placeholder,
                16.0,
                Margin::same(16),
                theme::DRAWER_BG,
                theme::INK,
                is_rtl,
            );

            if focus_notes_editor.as_deref() == Some(node_id.as_str()) {
                editor_output.response.request_focus();
                *focus_notes_editor = None;
            }
            if editor_output.response.changed() {
                actions.dirty = true;
            }
        });
}
