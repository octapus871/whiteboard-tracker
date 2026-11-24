use super::*;

/// Draws one pane inside its slot on the tiled canvas, splitting the bottom
/// `38%` into an embedded inline note drawer when a branch note is open.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_mind_map(
    ui: &mut Ui,
    rect: Rect,
    pane: &mut MainTaskPane,
    pane_idx: usize,
    is_selected: bool,
    zen: bool,
    focus: bool,
    lang: AppLang,
    pane_input: &mut String,
    open_drawer_node_id: Option<&str>,
    prev_hovered_card: Option<&str>,
    manually_expanded_nodes: &mut HashSet<String>,
    focus_notes_editor: &mut Option<String>,
    focused_pane_id: &mut Option<String>,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    selected_pane_ids: &mut HashSet<String>,
    scroll_seeded: &mut HashSet<String>,
    actions: &mut PaneActions,
) {
    let valid_drawer_id = open_drawer_node_id
        .filter(|id| find_node_in_slice_mut(&mut pane.subtasks, id).is_some())
        .map(str::to_string);

    let (top_rect, drawer_rect, drawer_h) = if valid_drawer_id.is_some() {
        let drawer_h = (rect.height() * 0.38)
            .max(96.0)
            .min((rect.height() - 120.0).max(96.0));
        let split_y = rect.bottom() - drawer_h;
        (
            Rect::from_min_max(rect.min, pos2(rect.right(), split_y)),
            Some(Rect::from_min_max(pos2(rect.left(), split_y), rect.max)),
            drawer_h,
        )
    } else {
        (rect, None, 0.0)
    };

    let mut child = ui.new_child(UiBuilder::new().max_rect(top_rect));
    child.set_clip_rect(top_rect);
    render_pane(
        &mut child,
        pane,
        pane_idx,
        is_selected,
        zen,
        focus,
        lang,
        drawer_h,
        pane_input,
        prev_hovered_card,
        manually_expanded_nodes,
        focused_pane_id,
        focused_node_id,
        editing_node_id,
        request_focus_id,
        selected_node_ids,
        selected_pane_ids,
        scroll_seeded,
        actions,
    );

    if let (Some(d_rect), Some(drawer_node_id)) = (drawer_rect, valid_drawer_id) {
        let pane_id = pane.id.clone();
        if let Some(node) = find_node_in_slice_mut(&mut pane.subtasks, &drawer_node_id) {
            render_pane_note_drawer(
                ui,
                d_rect,
                &pane_id,
                node,
                lang,
                focus_notes_editor,
                actions,
            );
        }
    }
}

/// Renders a single task pane: header + RTL progress hairline, horizontal-first
/// mind-map (auto-splitting into 2 balanced half-width columns when dense), and
/// a strict `38.0px` bottom input strip.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_pane(
    ui: &mut Ui,
    pane: &mut MainTaskPane,
    pane_idx: usize,
    is_selected: bool,
    zen: bool,
    _focus: bool,
    lang: AppLang,
    drawer_h: f32,
    pane_input: &mut String,
    prev_hovered_card: Option<&str>,
    manually_expanded_nodes: &mut HashSet<String>,
    focused_pane_id: &mut Option<String>,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    selected_pane_ids: &mut HashSet<String>,
    scroll_seeded: &mut HashSet<String>,
    actions: &mut PaneActions,
) {
    let is_rtl = lang.is_rtl();
    let s = tr(lang);
    let pane_outer_rect = ui.max_rect();
    let hover_clamp_rect = pane_outer_rect.shrink(4.0);

    // Pane surfaces are always pure white; only black ink marks structure.
    ui.painter()
        .rect_filled(pane_outer_rect, CornerRadius::same(theme::CARD_RADIUS), theme::PANE);
    if is_selected {
        ui.painter().rect_stroke(
            pane_outer_rect,
            CornerRadius::same(theme::CARD_RADIUS),
            Stroke::new(2.0, theme::GRID),
            StrokeKind::Inside,
        );
    }

    // Reserve a dedicated 38.0px bottom strip (#FFFFFF fill, 1.0px #E2E8F0 top border).
    let (upper_rect, input_strip_rect) = if !zen {
        let strip_h = BOTTOM_INPUT_HEIGHT.min((pane_outer_rect.height() * 0.35).max(BOTTOM_INPUT_HEIGHT));
        let split_y = (pane_outer_rect.bottom() - strip_h).max(pane_outer_rect.top());
        (
            Rect::from_min_max(pane_outer_rect.left_top(), pos2(pane_outer_rect.right(), split_y)),
            Some(Rect::from_min_max(
                pos2(pane_outer_rect.left(), split_y),
                pane_outer_rect.right_bottom(),
            )),
        )
    } else {
        (pane_outer_rect, None)
    };

    let pad = theme::PADDING as f32;
    let body_rect = Rect::from_min_max(
        pos2(upper_rect.left() + pad, upper_rect.top() + pad),
        pos2(upper_rect.right() - pad, (upper_rect.bottom() - 4.0).max(upper_rect.top() + pad)),
    );

    let top_align = if is_rtl { Align::Max } else { Align::Min };
    let mut body_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(body_rect)
            .layout(Layout::top_down(top_align)),
    );
    body_ui.set_clip_rect(upper_rect);

    let mut focus_toggle: Option<bool> = None;
    let (done, total) = pane.progress_counts();

    // Swiss architectural header: bold title on leading edge (right in RTL, left in LTR),
    // monospace index + progress badge on the opposite side.
    dir_row(&mut body_ui, 36.0, is_rtl, |ui| {
        let pane_id = pane.id.clone();
        let index = pane_idx + 1;
        let reserved = if zen { 132.0 } else { 160.0 };
        let title_width = (ui.available_width() - reserved).max(60.0);

        let title_color = match pane.status {
            TaskStatus::Active => theme::INK,
            TaskStatus::Completed | TaskStatus::Cancelled => theme::SLATE,
        };
        let mut title_text = RichText::new(disp(&pane.title))
            .size(20.0)
            .strong()
            .color(title_color);
        if pane.status == TaskStatus::Cancelled {
            title_text = title_text.strikethrough();
        }
        let title_response = ui
            .add_sized(
                Vec2::new(title_width, 32.0),
                Label::new(title_text)
                    .selectable(false)
                    .sense(Sense::click())
                    .halign(top_align)
                    .truncate(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        actions
            .pane_header_rects
            .push((pane_id.clone(), title_response.rect));

        if title_response.clicked() && !title_response.double_clicked() {
            let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            if ctrl {
                if selected_pane_ids.contains(&pane_id) {
                    selected_pane_ids.remove(&pane_id);
                } else {
                    selected_pane_ids.insert(pane_id.clone());
                }
            } else {
                focus_toggle = Some(focused_pane_id.as_deref() != Some(pane_id.as_str()));
            }
        }

        title_response.context_menu(|ui| {
            if ui
                .button(disp(s.ctx_delete_task))
                .on_hover_cursor(CursorIcon::PointingHand)
                .clicked()
            {
                actions.delete_pane = Some(pane_idx);
            }
        });

        if !zen {
            let (slot_rect, slot_response) = ui.allocate_exact_size(
                vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                Sense::click(),
            );
            let hovering = ui.input(|i| {
                i.pointer
                    .hover_pos()
                    .is_some_and(|p| title_response.rect.union(slot_rect).contains(p))
            });
            let slot_response = if hovering {
                ui.painter().text(
                    slot_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "×",
                    FontId::monospace(16.0),
                    theme::INK,
                );
                slot_response.on_hover_cursor(CursorIcon::PointingHand)
            } else {
                slot_response
            };
            if slot_response.clicked() {
                actions.delete_pane = Some(pane_idx);
            }
        }

        let badge_text = format!("[{:02}]  {}/{}", index, done, total);
        let font = FontId::monospace(14.0);
        let galley =
            ui.painter()
                .layout_no_wrap(badge_text.clone(), font.clone(), theme::INK_SOFT);
        let (badge_rect, badge_response) =
            ui.allocate_exact_size(galley.size() + vec2(10.0, 6.0), Sense::click());
        let badge_color = if badge_response.hovered() {
            theme::INK
        } else {
            theme::INK_SOFT
        };
        ui.painter().text(
            badge_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            &badge_text,
            font.clone(),
            badge_color,
        );
        ui.painter().text(
            badge_rect.left_center() + vec2(0.5, 0.0),
            egui::Align2::LEFT_CENTER,
            &badge_text,
            font,
            badge_color,
        );
        if badge_response.clicked() && !badge_response.double_clicked() {
            focus_toggle = Some(focused_pane_id.as_deref() != Some(pane_id.as_str()));
        }
        badge_response.on_hover_cursor(CursorIcon::PointingHand);
    });

    if let Some(toggle) = focus_toggle {
        *focused_pane_id = if toggle { Some(pane.id.clone()) } else { None };
    }

    body_ui.add_space(10.0);
    // Header-embedded 2.0px progress hairline: fills right-to-left in RTL (Ar) and
    // left-to-right in LTR (En) in Emerald Green (#10B981) with remaining #CBD5E1 track.
    let (rule_rect, _) =
        body_ui.allocate_exact_size(Vec2::new(body_ui.available_width(), 2.0), Sense::hover());
    let ratio = if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    };
    let done_w = rule_rect.width() * ratio;
    if is_rtl {
        let split_x = rule_rect.right() - done_w;
        if split_x > rule_rect.left() {
            body_ui.painter().rect_filled(
                Rect::from_min_max(rule_rect.left_top(), pos2(split_x, rule_rect.bottom())),
                CornerRadius::same(0),
                theme::HINT,
            );
        }
        if done_w > 0.0 {
            body_ui.painter().rect_filled(
                Rect::from_min_max(pos2(split_x, rule_rect.top()), rule_rect.right_bottom()),
                CornerRadius::same(0),
                theme::GREEN_SHADOW,
            );
        }
    } else {
        let split_x = rule_rect.left() + done_w;
        if done_w > 0.0 {
            body_ui.painter().rect_filled(
                Rect::from_min_max(rule_rect.left_top(), pos2(split_x, rule_rect.bottom())),
                CornerRadius::same(0),
                theme::GREEN_SHADOW,
            );
        }
        if split_x < rule_rect.right() {
            body_ui.painter().rect_filled(
                Rect::from_min_max(pos2(split_x, rule_rect.top()), rule_rect.right_bottom()),
                CornerRadius::same(0),
                theme::HINT,
            );
        }
    }
    body_ui.add_space(4.0);

    // Mind-map canvas strictly above the bottom 38.0px input strip.
    let tree_rect = body_ui.available_rect_before_wrap();
    let avail_w = tree_rect.width().max(60.0);
    let avail_h = tree_rect.height().max(40.0);
    let editing_snap = editing_node_id.clone();

    let single_base = mind_map_metrics_for_density(avail_w, total);

    let editing_in_pane = editing_snap
        .as_deref()
        .is_some_and(|id| find_node_in_slice_mut(&mut pane.subtasks, id).is_some());
    if !editing_in_pane {
        let initial_h = stack_height(
            &body_ui,
            &pane.subtasks,
            &single_base,
            0,
            editing_snap.as_deref(),
        );
        if initial_h + DENSE_TOP_MARGIN + SHADOW_OFFSET.y + BOTTOM_SCROLL_PAD > avail_h {
            if auto_collapse_completed_groups(
                &body_ui,
                &mut pane.subtasks,
                &single_base,
                avail_h,
                manually_expanded_nodes,
            ) {
                actions.dirty = true;
            }
        }
    }

    let single_unscaled_h = stack_height(
        &body_ui,
        &pane.subtasks,
        &single_base,
        0,
        editing_snap.as_deref(),
    );
    let bottom_clearance = SHADOW_OFFSET.y + BOTTOM_SCROLL_PAD;
    let needs_two_cols =
        pane.subtasks.len() >= 2 && (single_unscaled_h + DENSE_TOP_MARGIN + bottom_clearance) > avail_h;

    let mut hover_controls = Vec::new();
    let mut fold_handles = Vec::new();

    if needs_two_cols {
        let mid_x = tree_rect.center().x;
        let right_col_rect = Rect::from_min_max(
            pos2(mid_x + SPLIT_COL_GUTTER, tree_rect.top()),
            tree_rect.right_bottom(),
        );
        let left_col_rect = Rect::from_min_max(
            tree_rect.left_top(),
            pos2(mid_x - SPLIT_COL_GUTTER, tree_rect.bottom()),
        );
        let half_w = right_col_rect.width().min(left_col_rect.width()).max(80.0);

        let half_base = mind_map_metrics_for_split_col(half_w, total);
        let root_heights: Vec<f32> = pane
            .subtasks
            .iter()
            .map(|n| subtree_height(&body_ui, n, &half_base, 0, editing_snap.as_deref()))
            .collect();
        let split_idx = find_balanced_split(&root_heights, half_base.gap_y);

        let divider_top = rule_rect.bottom();
        let divider_bottom = input_strip_rect
            .map(|r| r.top())
            .unwrap_or(pane_outer_rect.bottom());
        ui.painter().line_segment(
            [
                pos2(mid_x, divider_top),
                pos2(mid_x, divider_bottom.max(divider_top)),
            ],
            Stroke::new(1.0, theme::SUBTLE_RULE),
        );

        let mut metrics = half_base;
        for _ in 0..3 {
            let (first_slice, second_slice) = pane.subtasks.split_at(split_idx);
            let fh = stack_height(&body_ui, first_slice, &metrics, 0, editing_snap.as_deref());
            let sh = stack_height(&body_ui, second_slice, &metrics, 0, editing_snap.as_deref());
            let fw = stack_width(&body_ui, first_slice, &metrics, 0, editing_snap.as_deref());
            let sw = stack_width(&body_ui, second_slice, &metrics, 0, editing_snap.as_deref());
            let req_h = fh.max(sh) + DENSE_TOP_MARGIN + bottom_clearance;
            let req_w = fw.max(sw) + SHADOW_PAD;
            let scale = fit_scale(req_h, req_w, avail_h, half_w);
            if (scale - 1.0).abs() < 0.005 {
                metrics = half_base;
                break;
            }
            metrics = half_base.scaled(scale);
        }

        let (first_nodes, second_nodes) = pane.subtasks.split_at_mut(split_idx);
        let first_h = stack_height(&body_ui, first_nodes, &metrics, 0, editing_snap.as_deref());
        let second_h = stack_height(&body_ui, second_nodes, &metrics, 0, editing_snap.as_deref());
        let first_content_h = (first_h + DENSE_TOP_MARGIN + bottom_clearance).max(avail_h);
        let second_content_h = (second_h + DENSE_TOP_MARGIN + bottom_clearance).max(avail_h);

        // In RTL, first group is in Right Column and second group in Left Column.
        // In LTR, first group is in Left Column and second group in Right Column.
        let (first_col_rect, second_col_rect) = if is_rtl {
            (right_col_rect, left_col_rect)
        } else {
            (left_col_rect, right_col_rect)
        };

        let mut first_col_ui = body_ui.new_child(UiBuilder::new().max_rect(first_col_rect));
        first_col_ui.set_clip_rect(first_col_rect);
        ScrollArea::vertical()
            .id_salt(("pane_scroll_2col_1", pane.id.as_str()))
            .auto_shrink([false, false])
            .max_height(avail_h)
            .show(&mut first_col_ui, |ui| {
                ui.set_clip_rect(first_col_rect);
                let col_w = first_col_rect.width();
                let (content_rect, _) =
                    ui.allocate_exact_size(vec2(col_w, first_content_h), Sense::hover());
                let col_anchor = if is_rtl { col_w - 2.0 } else { 2.0 };
                render_mind_map_nodes(
                    ui,
                    first_nodes,
                    0,
                    0,
                    DENSE_TOP_MARGIN + first_h * 0.5,
                    col_anchor,
                    content_rect.min.x,
                    content_rect.min.y,
                    &metrics,
                    zen,
                    lang,
                    hover_clamp_rect,
                    prev_hovered_card,
                    manually_expanded_nodes,
                    focused_node_id,
                    editing_node_id,
                    request_focus_id,
                    selected_node_ids,
                    &mut hover_controls,
                    &mut fold_handles,
                    actions,
                );
            });

        let mut second_col_ui = body_ui.new_child(UiBuilder::new().max_rect(second_col_rect));
        second_col_ui.set_clip_rect(second_col_rect);
        ScrollArea::vertical()
            .id_salt(("pane_scroll_2col_2", pane.id.as_str()))
            .auto_shrink([false, false])
            .max_height(avail_h)
            .show(&mut second_col_ui, |ui| {
                ui.set_clip_rect(second_col_rect);
                let col_w = second_col_rect.width();
                let (content_rect, _) =
                    ui.allocate_exact_size(vec2(col_w, second_content_h), Sense::hover());
                let col_anchor = if is_rtl { col_w - 2.0 } else { 2.0 };
                render_mind_map_nodes(
                    ui,
                    second_nodes,
                    0,
                    split_idx,
                    DENSE_TOP_MARGIN + second_h * 0.5,
                    col_anchor,
                    content_rect.min.x,
                    content_rect.min.y,
                    &metrics,
                    zen,
                    lang,
                    hover_clamp_rect,
                    prev_hovered_card,
                    manually_expanded_nodes,
                    focused_node_id,
                    editing_node_id,
                    request_focus_id,
                    selected_node_ids,
                    &mut hover_controls,
                    &mut fold_handles,
                    actions,
                );
            });
    } else {
        // Single wide horizontal tree across the full pane width (RTL for Ar, LTR for En).
        let mut metrics = single_base;
        for _ in 0..3 {
            let req_h = (stack_height(&body_ui, &pane.subtasks, &metrics, 0, editing_snap.as_deref())
                + DENSE_TOP_MARGIN
                + bottom_clearance)
                .max(metrics.min_card_h + DENSE_TOP_MARGIN + bottom_clearance);
            let req_w =
                stack_width(&body_ui, &pane.subtasks, &metrics, 0, editing_snap.as_deref())
                    + SHADOW_PAD;
            let scale = fit_scale(req_h, req_w, avail_h, avail_w);
            if (scale - 1.0).abs() < 0.005 {
                metrics = single_base;
                break;
            }
            metrics = single_base.scaled(scale);
        }

        let content_w =
            stack_width(&body_ui, &pane.subtasks, &metrics, 0, editing_snap.as_deref()) + SHADOW_PAD;
        let tree_h =
            stack_height(&body_ui, &pane.subtasks, &metrics, 0, editing_snap.as_deref());
        let content_h = (tree_h + DENSE_TOP_MARGIN + bottom_clearance).max(avail_h);
        let overflow_x = (content_w - avail_w).max(0.0);
        let unshrunk_avail_h = avail_h + drawer_h;
        let is_sparse = total <= 7 && tree_h <= unshrunk_avail_h * 0.72;
        let min_center = DENSE_TOP_MARGIN + tree_h * 0.5;
        let block_center_y = if is_sparse {
            let preferred = (unshrunk_avail_h * 0.34).max(min_center);
            let max_center = (avail_h - bottom_clearance - tree_h * 0.5).max(min_center);
            preferred.min(max_center)
        } else {
            min_center
        };

        let seeded = scroll_seeded.contains(&pane.id);
        let mut area = ScrollArea::both()
            .id_salt(("pane_scroll", pane.id.as_str()))
            .auto_shrink([false, false])
            .max_height(avail_h);
        if is_rtl && overflow_x > 1.0 && !seeded {
            area = area.horizontal_scroll_offset(overflow_x);
        }
        area.show(&mut body_ui, |ui| {
            let total_w = avail_w.max(content_w);
            let (content_rect, _) =
                ui.allocate_exact_size(vec2(total_w, content_h), Sense::hover());
            let origin = content_rect.min;
            let col_anchor = if is_rtl { total_w - 2.0 } else { 2.0 };
            render_mind_map_nodes(
                ui,
                &mut pane.subtasks,
                0,
                0,
                block_center_y,
                col_anchor,
                origin.x,
                origin.y,
                &metrics,
                zen,
                lang,
                hover_clamp_rect,
                prev_hovered_card,
                manually_expanded_nodes,
                focused_node_id,
                editing_node_id,
                request_focus_id,
                selected_node_ids,
                &mut hover_controls,
                &mut fold_handles,
                actions,
            );
        });
        if overflow_x > 1.0 && !seeded {
            scroll_seeded.insert(pane.id.clone());
        }
    }

    paint_fold_handles(ui, &fold_handles);
    actions.hover_controls.extend(hover_controls);

    // Render dedicated 38.0px bottom input strip (`#FFFFFF`, `1.0px` `#E2E8F0` top border).
    if let Some(strip_rect) = input_strip_rect {
        ui.painter()
            .rect_filled(strip_rect, CornerRadius::same(0), theme::PANE);
        ui.painter().rect_filled(
            Rect::from_min_max(
                strip_rect.left_top(),
                pos2(strip_rect.right(), strip_rect.top() + 1.0),
            ),
            CornerRadius::same(0),
            theme::SUBTLE_RULE,
        );
        let input_inner = Rect::from_min_max(
            pos2(strip_rect.left() + pad, strip_rect.top() + 2.0),
            pos2(strip_rect.right() - pad, strip_rect.bottom() - 2.0),
        );
        let mut input_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(input_inner)
                .layout(Layout::left_to_right(Align::Center)),
        );
        input_ui.set_clip_rect(input_inner);
        render_pane_branch_input(&mut input_ui, &pane.id, pane_input, lang, actions);
    }
}

/// Hard architectural offset shadow displacement (`dx: -3.0, dy: 3.0`).
pub(super) const SHADOW_OFFSET: Vec2 = Vec2::new(-3.0, 3.0);
/// Padding reserved around the mind-map canvas so hard shadows never clip.
pub(super) const SHADOW_PAD: f32 = 6.0;
/// Height of the zero-overlap contextual hover strip (`>= 28px` hit target).
pub(super) const HOVER_STRIP_H: f32 = MIN_HITBOX_SIZE;
/// Width of each action slot inside the 3-action hover strip (`✓`, `+`, `≡`, `>= 28px`).
pub(super) const HOVER_STRIP_SLOT_W: f32 = MIN_HITBOX_SIZE;
/// Top margin applied when top-aligning dense task trees inside a pane.
pub(super) const DENSE_TOP_MARGIN: f32 = 14.0;
/// Extra bottom padding reserved beneath the bottom card's 3.0px shadow inside scroll areas.
pub(super) const BOTTOM_SCROLL_PAD: f32 = 12.0;
/// Dedicated bottom input strip height (`#FFFFFF` fill, `1.0px` `#E2E8F0` top border).
pub(super) const BOTTOM_INPUT_HEIGHT: f32 = 38.0;
/// Generous horizontal breathing room between parent left edge and child right edge (single column).
pub(super) const BRANCH_GAP_X: f32 = 56.0;
/// Compact horizontal Bezier branch span inside 2-column split panes so child cards never cross columns.
pub(super) const SPLIT_BRANCH_GAP_X: f32 = 30.0;
/// Inner horizontal margin on each side of the center vertical divider (`16.0px` total gutter).
pub(super) const SPLIT_COL_GUTTER: f32 = 8.0;

/// Metrics driving the horizontal right-to-left mind-map layout.
#[derive(Clone, Copy)]
pub(super) struct MindMapMetrics {
    /// Maximum width of a root task card.
    pub(super) max_card_w: f32,
    /// Maximum width of a child sub-branch card (capped tighter inside split columns).
    pub(super) child_max_card_w: f32,
    /// Horizontal gap between parent and child cards (Bezier curves span this).
    pub(super) gap_x: f32,
    /// Minimum height of one task card.
    pub(super) min_card_h: f32,
    /// Vertical gap between two sibling branches (8.0px base).
    pub(super) gap_y: f32,
    /// Font size of a level-0 task (15.5px).
    pub(super) font_size: f32,
    /// Font size of a nested branch (14.5px).
    pub(super) font_size_deep: f32,
    /// Horizontal padding inside a card (14.0px base).
    pub(super) pad_x: f32,
    /// Vertical padding inside a card (8.0px base).
    pub(super) pad_y: f32,
}

impl MindMapMetrics {
    /// Scales every metric proportionally (used by the fit-to-pane zoom).
    pub(super) fn scaled(self, scale: f32) -> Self {
        Self {
            max_card_w: (self.max_card_w * scale).max(64.0),
            child_max_card_w: (self.child_max_card_w * scale).max(56.0),
            gap_x: (self.gap_x * scale).max(18.0),
            min_card_h: self.min_card_h * scale,
            gap_y: (self.gap_y * scale).max(4.0),
            font_size: (self.font_size * scale).clamp(10.0, 16.0),
            font_size_deep: (self.font_size_deep * scale).clamp(10.0, 16.0),
            pad_x: (self.pad_x * scale).max(7.0),
            pad_y: (self.pad_y * scale).max(3.5),
        }
    }
}

/// Zoom factor that makes a tree of the given size fit into the pane.
/// Never scales below 55% and never scales up.
pub(super) fn fit_scale(req_h: f32, req_w: f32, avail_h: f32, avail_w: f32) -> f32 {
    let mut scale = 1.0_f32;
    if req_h > avail_h && req_h > 0.0 {
        scale = scale.min(avail_h / req_h);
    }
    if req_w > avail_w && req_w > 0.0 {
        scale = scale.min(avail_w / req_w);
    }
    scale.clamp(0.55, 1.0)
}

/// Width of the crisp `0.0px` root-only index badge (`01`, `02`, `03`...) flush
/// inside the right edge of top-level root cards (`depth == 0`).
pub(super) const ROOT_BADGE_W: f32 = 24.0;

/// Computes adaptive mind-map metrics for a pane column of width `pane_w`:
/// * `<= 7` total nodes -> spacious (`14.0px` H, `8.0px` V, `8.0px` sibling gap).
/// * `> 7` total nodes -> compact (`12.0px` H, `5.0px` V, `5.0px` sibling gap).
/// * Root cards use `15.5px` Bold; child sub-branches use `14.5px` Regular.
/// * Generous `56.0px` horizontal Bezier gap and wider card cap so Arabic phrases
///   sit comfortably on 1–2 lines without cramped wrapping.
pub(super) fn mind_map_metrics_for_density(pane_w: f32, total_nodes: usize) -> MindMapMetrics {
    let dense = total_nodes > 7;
    let font_size = 15.5;
    let font_size_deep = 14.5;
    let (pad_x, pad_y, gap_y) = if dense {
        (12.0, 5.0, 5.0)
    } else {
        (14.0, 8.0, 8.0)
    };

    let max_card_w = (pane_w * 0.56)
        .clamp(180.0, 380.0)
        .min((pane_w - 20.0).max(120.0));

    MindMapMetrics {
        max_card_w,
        child_max_card_w: max_card_w,
        gap_x: BRANCH_GAP_X,
        min_card_h: font_size * 1.35 + pad_y * 2.0,
        gap_y,
        font_size,
        font_size_deep,
        pad_x,
        pad_y,
    }
}

/// Split-column mind-map metrics for one half of a 2-column pane (`col_w`):
/// * Uses a compact `30.0px` horizontal Bezier span (`SPLIT_BRANCH_GAP_X`).
/// * Caps root & child card widths so `root + 30px + child` fits strictly inside
///   `col_w` without touching the `8.0px` center gutter or crossing columns.
pub(super) fn mind_map_metrics_for_split_col(col_w: f32, total_nodes: usize) -> MindMapMetrics {
    let mut m = mind_map_metrics_for_density(col_w, total_nodes);
    let usable = (col_w - SHADOW_PAD - 2.0).max(110.0);
    let cards_budget = (usable - SPLIT_BRANCH_GAP_X).max(80.0);
    let root_cap = (cards_budget * 0.56).clamp(96.0, 240.0).min(usable);
    let child_cap = (cards_budget * 0.44)
        .clamp(72.0, 170.0)
        .min((usable - SPLIT_BRANCH_GAP_X - 72.0).max(64.0));
    m.max_card_w = root_cap;
    m.child_max_card_w = child_cap;
    m.gap_x = SPLIT_BRANCH_GAP_X;
    m
}

/// Finds the contiguous split index (`1..heights.len()`) that partitions root
/// groups into two balanced halves (Right Half `0..split`, Left Half `split..`)
/// minimizing the taller half's total vertical height.
pub(super) fn find_balanced_split(heights: &[f32], gap_y: f32) -> usize {
    let n = heights.len();
    if n <= 1 {
        return n;
    }
    let stack_h = |slice: &[f32]| -> f32 {
        if slice.is_empty() {
            0.0
        } else {
            slice.iter().sum::<f32>() + gap_y * (slice.len() as f32 - 1.0)
        }
    };
    let mut best_idx = 1;
    let mut best_max = f32::INFINITY;
    for split in 1..n {
        let worst = stack_h(&heights[..split]).max(stack_h(&heights[split..]));
        if worst < best_max {
            best_max = worst;
            best_idx = split;
        }
    }
    best_idx
}

/// Computes the standard (spacious) mind-map metrics for a pane of the given width.
#[cfg(test)]
pub(super) fn mind_map_metrics(_focus: bool, pane_w: f32) -> MindMapMetrics {
    mind_map_metrics_for_density(pane_w, 0)
}

/// The font size a task uses at the given tree depth.
pub(super) fn font_for_depth(m: &MindMapMetrics, depth: usize) -> f32 {
    if depth == 0 {
        m.font_size
    } else {
        m.font_size_deep
    }
}

/// The ink color an active task/branch card uses (`#0F172A` high contrast).
pub(super) fn ink_for_depth(_depth: usize) -> Color32 {
    theme::INK
}

/// Wraps logical Arabic/RTL text into lines fitting `max_width` *before*
/// applying RTL visual reversal on each line individually, so multi-line
/// Arabic paragraphs always read naturally top-to-bottom (never reversing line
/// order). Returns `(shaped_multiline_text, measured_size)`.
pub(super) fn wrap_and_shape_rtl(
    ui: &Ui,
    text: &str,
    font_id: FontId,
    max_width: f32,
) -> (String, Vec2) {
    if text.is_empty() {
        return (String::new(), Vec2::ZERO);
    }
    let max_w = max_width.max(32.0);
    if !crate::rtl_text::contains_arabic(text) {
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = max_w;
        job.halign = Align::RIGHT;
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id,
                ..Default::default()
            },
        );
        let size = ui.fonts_mut(|fonts| fonts.layout_job(job).size());
        return (text.to_string(), size);
    }

    let measure_line_w = |line: &str| -> f32 {
        let shaped = shape_arabic(line);
        ui.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(shaped, font_id.clone(), Color32::TRANSPARENT)
                .size()
                .x
        })
    };

    let mut visual_lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            visual_lines.push(String::new());
            continue;
        }
        if measure_line_w(trimmed) <= max_w {
            visual_lines.push(shape_arabic(trimmed));
            continue;
        }
        let mut current = String::new();
        for word in trimmed.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if current.is_empty() || measure_line_w(&candidate) <= max_w {
                current = candidate;
            } else {
                visual_lines.push(shape_arabic(&current));
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            visual_lines.push(shape_arabic(&current));
        }
    }

    let joined = visual_lines.join("\n");
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    job.halign = Align::RIGHT;
    job.append(
        &joined,
        0.0,
        egui::TextFormat {
            font_id,
            ..Default::default()
        },
    );
    let size = ui.fonts_mut(|fonts| fonts.layout_job(job).size());
    (joined, size)
}

/// Measures the dynamic `(width, height)` of a task/branch card to fit its
/// actual Arabic/English text content (reserving `ROOT_BADGE_W` on the right
/// edge for root cards' `01`/`02`/`03` index tab).
pub(super) fn measure_card_size(
    ui: &Ui,
    node: &TaskNode,
    m: &MindMapMetrics,
    depth: usize,
    is_editing: bool,
) -> Vec2 {
    let font_size = font_for_depth(m, depth);
    let cap_w = if depth == 0 {
        m.max_card_w
    } else {
        m.child_max_card_w
    };
    let badge_reserve = if depth == 0 { ROOT_BADGE_W } else { 0.0 };
    let max_text_w = (cap_w - m.pad_x * 2.0 - badge_reserve).max(40.0);
    let line_h = font_size * 1.35;

    let sample = if node.title.trim().is_empty() {
        if is_editing {
            "عنوان المهمة..."
        } else {
            " "
        }
    } else {
        node.title.as_str()
    };

    let text_size = if is_editing {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &shape_arabic(sample),
            0.0,
            egui::TextFormat {
                font_id: FontId::proportional(font_size),
                ..Default::default()
            },
        );
        ui.fonts_mut(|fonts| fonts.layout_job(job).size())
    } else {
        wrap_and_shape_rtl(ui, sample, FontId::proportional(font_size), max_text_w).1
    };

    // Small buffer (+4px, or +14px while editing for the caret) prevents
    // sub-pixel rounding from triggering an unintended extra line wrap.
    let extra_w = if is_editing { 14.0 } else { 4.0 };
    let inner_w = (text_size.x.ceil() + extra_w).clamp(32.0, max_text_w);
    let inner_h = text_size.y.max(line_h);

    vec2(
        (inner_w + m.pad_x * 2.0 + badge_reserve).min(cap_w),
        (inner_h + m.pad_y * 2.0).max(m.min_card_h),
    )
}

/// Zero-width-penalty `14.0px × 14.0px` fold handle straddling a parent card's
/// left border center (`card_rect.left_center()`).
pub(super) const FOLD_HANDLE_SIZE: f32 = 14.0;
/// Generous invisible click hitbox (`28.0px × 28.0px`) for the fold handle.
pub(super) const FOLD_HANDLE_HITBOX: f32 = MIN_HITBOX_SIZE;

/// Auto-collapses ONLY parent groups whose descendants are 100% Completed
/// (`TaskStatus::Completed`) when the pane exceeds `avail_h`, skipping any
/// group the user manually expanded. Never touches active/in-progress trees.
pub(super) fn auto_collapse_completed_groups(
    ui: &Ui,
    nodes: &mut [TaskNode],
    m: &MindMapMetrics,
    avail_h: f32,
    manually_expanded: &HashSet<String>,
) -> bool {
    let mut changed = false;
    for i in 0..nodes.len() {
        let cur_h =
            stack_height(ui, nodes, m, 0, None) + DENSE_TOP_MARGIN + SHADOW_OFFSET.y + BOTTOM_SCROLL_PAD;
        if cur_h <= avail_h {
            break;
        }
        let node = &mut nodes[i];
        if !node.is_folded()
            && node.all_children_completed()
            && !manually_expanded.contains(&node.id)
        {
            node.collapsed = true;
            changed = true;
        }
    }
    changed
}

/// Computes the crisp `0.0px` left-border-straddling fold handle dimensions
/// (`14.0px × 14.0px` base, widening slightly only for multi-digit `+N`).
pub(super) fn fold_handle_visual_size(ui: &Ui, is_folded: bool, hidden_count: usize) -> Vec2 {
    if !is_folded {
        return vec2(FOLD_HANDLE_SIZE, FOLD_HANDLE_SIZE);
    }
    let text = format!("+{}", hidden_count);
    let text_w = ui
        .painter()
        .layout_no_wrap(text, FontId::monospace(11.0), theme::PANE)
        .size()
        .x;
    vec2(
        FOLD_HANDLE_SIZE.max((text_w + 4.0).ceil()),
        FOLD_HANDLE_SIZE,
    )
}

/// Horizontal width a single node (with its whole branch) occupies.
/// The straddling `14px` fold handle adds virtually zero horizontal penalty.
pub(super) fn subtree_width(
    ui: &Ui,
    node: &TaskNode,
    m: &MindMapMetrics,
    depth: usize,
    editing_id: Option<&str>,
) -> f32 {
    let own_w = measure_card_size(ui, node, m, depth, editing_id == Some(node.id.as_str())).x;
    if node.children.is_empty() {
        return own_w;
    }
    if node.is_folded() {
        let half_handle = fold_handle_visual_size(ui, true, node.descendant_count()).x * 0.5;
        return own_w + half_handle + 2.0;
    }
    own_w + m.gap_x + stack_width(ui, &node.children, m, depth + 1, editing_id)
}

/// Horizontal width a stack of siblings (with their branches) occupies.
pub(super) fn stack_width(
    ui: &Ui,
    nodes: &[TaskNode],
    m: &MindMapMetrics,
    depth: usize,
    editing_id: Option<&str>,
) -> f32 {
    nodes
        .iter()
        .map(|n| subtree_width(ui, n, m, depth, editing_id))
        .fold(0.0_f32, f32::max)
}

/// Vertical space a single node (with its whole branch) occupies.
pub(super) fn subtree_height(
    ui: &Ui,
    node: &TaskNode,
    m: &MindMapMetrics,
    depth: usize,
    editing_id: Option<&str>,
) -> f32 {
    let own = measure_card_size(ui, node, m, depth, editing_id == Some(node.id.as_str())).y;
    if node.is_folded() || node.children.is_empty() {
        return own;
    }
    own.max(stack_height(ui, &node.children, m, depth + 1, editing_id))
}

/// Vertical space a stack of siblings occupies.
pub(super) fn stack_height(
    ui: &Ui,
    nodes: &[TaskNode],
    m: &MindMapMetrics,
    depth: usize,
    editing_id: Option<&str>,
) -> f32 {
    if nodes.is_empty() {
        return 0.0;
    }
    let sum: f32 = nodes
        .iter()
        .map(|n| subtree_height(ui, n, m, depth, editing_id))
        .sum();
    sum + m.gap_y * (nodes.len() as f32 - 1.0)
}

/// Vertical centers of a stack of sibling cards, centred on `block_center_y`.
pub(super) fn stack_centers(
    ui: &Ui,
    nodes: &[TaskNode],
    m: &MindMapMetrics,
    depth: usize,
    block_center_y: f32,
    editing_id: Option<&str>,
) -> Vec<f32> {
    let total = stack_height(ui, nodes, m, depth, editing_id);
    let mut cursor = block_center_y - total * 0.5;
    let mut centers = Vec::with_capacity(nodes.len());
    for node in nodes {
        let h = subtree_height(ui, node, m, depth, editing_id);
        centers.push(cursor + h * 0.5);
        cursor += h + m.gap_y;
    }
    centers
}

/// Where a task card sits, so the Bezier ink curves can attach to its edges.
#[derive(Clone, Copy)]
pub(super) struct NodeAnchor {
    /// Exact screen-space bounding rect of the card.
    rect: Rect,
    /// True while the pointer hovers this card or one of its descendants.
    hot: bool,
}

/// Zero-overlap contextual hover strip rendered in an `Order::Foreground`
/// interactive `Area` flush against the card's top edge (`strip_rect.bottom() == card_rect.top()`),
/// horizontally centered on `card_rect` (`22.0px` height, `#0F172A` fill, `#FFFFFF`
/// `13.5px` Bold icons: `✓`, `+`, `≡`).
#[derive(Clone)]
pub(super) struct HoverControl {
    pub(super) node_id: String,
    strip_rect: Rect,
    check_rect: Rect,
    plus_rect: Rect,
    notes_rect: Rect,
    is_completed: bool,
    /// Selection rank so a neighboring card behind the strip can never steal focus:
    /// `4` = pointer over active card's strip, `3` = pointer on active card,
    /// `2` = pointer on another card, `1` = pointer in 8px padded bridge, `0` = 300ms grace.
    pub(super) priority: u8,
}

/// Deferred visual for the `14.0px × 14.0px` border-straddling fold handle
/// (`[−]` when expanded, `[+N]` when collapsed), painted cleanly over Bezier origins
/// while staying strictly clipped to its column rect.
#[derive(Clone, Copy)]
pub(super) struct FoldHandleVisual {
    rect: Rect,
    clip_rect: Rect,
    is_folded: bool,
    hidden_count: usize,
    emphasized: bool,
}

/// Renders one column of the horizontal mind-map (RTL for Arabic, LTR for English)
/// and returns the anchors of the cards it produced so Bezier curves can attach cleanly.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_mind_map_nodes(
    ui: &mut Ui,
    nodes: &mut [TaskNode],
    depth: usize,
    root_index_offset: usize,
    block_center_y: f32,
    col_anchor: f32,
    origin_x: f32,
    origin_y: f32,
    m: &MindMapMetrics,
    zen: bool,
    lang: AppLang,
    hover_clamp_rect: Rect,
    prev_hovered_card: Option<&str>,
    manually_expanded_nodes: &mut HashSet<String>,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    hover_controls: &mut Vec<HoverControl>,
    fold_handles: &mut Vec<FoldHandleVisual>,
    actions: &mut PaneActions,
) -> Vec<NodeAnchor> {
    let mut cards = Vec::new();
    if nodes.is_empty() {
        return cards;
    }
    let is_rtl = lang.is_rtl();

    let editing_snap = editing_node_id.clone();
    let centers = stack_centers(
        ui,
        nodes,
        m,
        depth,
        block_center_y,
        editing_snap.as_deref(),
    );

    for (local_idx, (node, center)) in nodes.iter_mut().zip(centers).enumerate() {
        let is_editing = editing_node_id.as_deref() == Some(node.id.as_str());
        let card_size = measure_card_size(ui, node, m, depth, is_editing);
        // RTL: cards grow LEFT from `col_anchor`. LTR: cards grow RIGHT from `col_anchor`.
        let (x_min, x_max) = if is_rtl {
            (origin_x + col_anchor - card_size.x, origin_x + col_anchor)
        } else {
            (origin_x + col_anchor, origin_x + col_anchor + card_size.x)
        };
        let rect = Rect::from_min_max(
            pos2(x_min, origin_y + center - card_size.y * 0.5),
            pos2(x_max, origin_y + center + card_size.y * 0.5),
        );
        let root_number = if depth == 0 {
            Some(root_index_offset + local_idx + 1)
        } else {
            None
        };

        let mut anchor = render_node_card(
            ui,
            node,
            rect,
            depth,
            root_number,
            m,
            zen,
            lang,
            hover_clamp_rect,
            prev_hovered_card,
            manually_expanded_nodes,
            focused_node_id,
            editing_node_id,
            request_focus_id,
            selected_node_ids,
            hover_controls,
            fold_handles,
            actions,
        );

        if !node.is_folded() && !node.children.is_empty() {
            let child_col_anchor = if is_rtl {
                col_anchor - card_size.x - m.gap_x
            } else {
                col_anchor + card_size.x + m.gap_x
            };
            let child_cards = render_mind_map_nodes(
                ui,
                &mut node.children,
                depth + 1,
                0,
                center,
                child_col_anchor,
                origin_x,
                origin_y,
                m,
                zen,
                lang,
                hover_clamp_rect,
                prev_hovered_card,
                manually_expanded_nodes,
                focused_node_id,
                editing_node_id,
                request_focus_id,
                selected_node_ids,
                hover_controls,
                fold_handles,
                actions,
            );
            if child_cards.iter().any(|child| child.hot) {
                anchor.hot = true;
            }
            draw_mind_map_threads(ui, anchor, &child_cards, is_rtl);
        }

        cards.push(anchor);
    }

    cards
}

/// Draws a Swiss minimalist task card mirrored for `AppLang::Ar` (RTL) and `AppLang::En` (LTR):
/// * Root (`depth == 0`): `1.5px` `#0F172A` border, `#FFFFFF` fill, `15.5px` Bold
///   text, and flush leading-edge `0.0px` monospace number badge (`01`, `02`, ...).
/// * Child (`depth > 0`): `1.0px` `#64748B` border, `#F8FAFC` paper fill, `14.5px`
///   Regular text, and no number badge.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_node_card(
    ui: &mut Ui,
    node: &mut TaskNode,
    rect: Rect,
    depth: usize,
    root_number: Option<usize>,
    m: &MindMapMetrics,
    zen: bool,
    lang: AppLang,
    hover_clamp_rect: Rect,
    prev_hovered_card: Option<&str>,
    manually_expanded_nodes: &mut HashSet<String>,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    hover_controls: &mut Vec<HoverControl>,
    fold_handles: &mut Vec<FoldHandleVisual>,
    actions: &mut PaneActions,
) -> NodeAnchor {
    let is_rtl = lang.is_rtl();
    let s = tr(lang);
    let node_id = node.id.clone();
    let is_selected = selected_node_ids.contains(&node_id);
    let is_editing = editing_node_id.as_deref() == Some(node_id.as_str());
    let font_size = font_for_depth(m, depth);
    let has_children = !node.children.is_empty();
    let is_folded = has_children && node.is_folded();
    let hidden_count = if is_folded {
        node.descendant_count()
    } else {
        0
    };
    let has_notes = !node.notes.trim().is_empty();

    // Zero-width-penalty 14×14 architectural fold handle straddling the outgoing
    // branch edge (`left_center` in RTL, `right_center` in LTR).
    let fold_anchor_center = if is_rtl {
        rect.left_center()
    } else {
        rect.right_center()
    };
    let fold_handle_rects = if has_children {
        let vis_size = fold_handle_visual_size(ui, is_folded, hidden_count);
        let vis_rect = Rect::from_center_size(fold_anchor_center, vis_size);
        let hit_w = vis_size.x.max(FOLD_HANDLE_HITBOX);
        // Extend the 28px hitbox outward into the branch gutter so it covers 100% of
        // `vis_rect` without encroaching into the card's dog-ear note hit-zone.
        let outward = (hit_w - vis_size.x) * 0.5;
        let hit_nudge_x = if is_rtl { -outward } else { outward };
        let hit_rect = Rect::from_center_size(
            pos2(fold_anchor_center.x + hit_nudge_x, rect.center().y),
            vec2(hit_w, FOLD_HANDLE_HITBOX),
        );
        Some((vis_rect, hit_rect))
    } else {
        None
    };

    // Sticky zero-gap contextual hover strip flush against the card's top edge
    // (`strip_rect.bottom() == rect.top()`, zero gap, 3 actions: `✓`, `+`, `≡`),
    // horizontally centered on `card_rect` and clamped inside pane bounds.
    let strip_w = HOVER_STRIP_SLOT_W * 3.0;
    let preferred_strip_x = rect.center().x - strip_w * 0.5;
    let clamped_x = preferred_strip_x.clamp(
        hover_clamp_rect.left(),
        (hover_clamp_rect.right() - strip_w).max(hover_clamp_rect.left()),
    );
    let strip_rect = Rect::from_min_size(
        pos2(clamped_x, rect.top() - HOVER_STRIP_H),
        vec2(strip_w, HOVER_STRIP_H),
    );
    let check_rect = Rect::from_min_size(
        strip_rect.left_top(),
        vec2(HOVER_STRIP_SLOT_W, HOVER_STRIP_H),
    );
    let plus_rect = Rect::from_min_size(
        pos2(strip_rect.left() + HOVER_STRIP_SLOT_W, strip_rect.top()),
        vec2(HOVER_STRIP_SLOT_W, HOVER_STRIP_H),
    );
    let notes_rect = Rect::from_min_size(
        pos2(
            strip_rect.left() + HOVER_STRIP_SLOT_W * 2.0,
            strip_rect.top(),
        ),
        vec2(HOVER_STRIP_SLOT_W, HOVER_STRIP_H),
    );

    let mut card_hit_area = rect;
    if let Some((vis_rect, _)) = fold_handle_rects {
        card_hit_area = card_hit_area.union(vis_rect);
    }
    let col_clip = ui.clip_rect();
    let card_visible = rect.intersects(col_clip) && rect.top() >= col_clip.top() - 2.0;
    let is_sticky_card = prev_hovered_card == Some(node_id.as_str());
    let bridge_zone = rect.union(strip_rect).expand(HOVER_BRIDGE_PAD);
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let pointer_on_card =
        card_visible && hover_pos.is_some_and(|p| col_clip.contains(p) && card_hit_area.contains(p));
    let pointer_in_strip = card_visible
        && !zen
        && !is_editing
        && is_sticky_card
        && hover_pos.is_some_and(|p| strip_rect.expand(4.0).contains(p));
    let pointer_in_bridge = card_visible
        && !zen
        && !is_editing
        && is_sticky_card
        && hover_pos.is_some_and(|p| bridge_zone.contains(p));
    let hover_priority: Option<u8> = if !card_visible || zen || is_editing {
        None
    } else if pointer_in_strip {
        Some(4)
    } else if is_sticky_card && pointer_on_card {
        Some(3)
    } else if pointer_on_card {
        Some(2)
    } else if pointer_in_bridge {
        Some(1)
    } else if is_sticky_card {
        Some(0)
    } else {
        None
    };
    let hovering = hover_priority.is_some();

    // 1. Solid 3.0px hard offset shadow behind the card (Active / Completed / Cancelled).
    let painter = ui.painter();
    painter.rect_filled(
        rect.translate(SHADOW_OFFSET),
        CornerRadius::same(theme::CARD_RADIUS),
        theme::status_shadow_color(node.status),
    );

    // 2. Architectural visual hierarchy (Root `#FFFFFF` 1.5px `#0F172A`, Child `#F8FAFC` 1.0px `#64748B`).
    let is_root = depth == 0;
    let card_fill = if is_root {
        theme::PANE
    } else {
        theme::DRAWER_BG
    };
    painter.rect_filled(rect, CornerRadius::same(theme::CARD_RADIUS), card_fill);
    let (border_width, border_color) = if is_selected {
        (2.0, theme::INK)
    } else if is_root {
        (1.5, theme::INK)
    } else {
        (1.0, theme::SLATE)
    };
    painter.rect_stroke(
        rect,
        CornerRadius::same(theme::CARD_RADIUS),
        Stroke::new(border_width, border_color),
        StrokeKind::Inside,
    );

    // 2b. Root-only monospace number badge (`01`, `02`, `03`...) flush on the leading edge
    // (right edge in RTL Arabic, left edge in LTR English).
    if let Some(num) = root_number {
        let num_rect = if is_rtl {
            Rect::from_min_max(
                pos2(rect.right() - ROOT_BADGE_W, rect.top()),
                rect.right_bottom(),
            )
        } else {
            Rect::from_min_max(
                rect.left_top(),
                pos2(rect.left() + ROOT_BADGE_W, rect.bottom()),
            )
        };
        let num_text = format!("{:02}", num);
        let num_font = FontId::monospace(11.5);
        painter.rect_filled(num_rect, CornerRadius::same(0), theme::INK);
        painter.text(
            num_rect.center(),
            egui::Align2::CENTER_CENTER,
            &num_text,
            num_font.clone(),
            theme::PANE,
        );
        painter.text(
            num_rect.center() + vec2(0.5, 0.0),
            egui::Align2::CENTER_CENTER,
            &num_text,
            num_font,
            theme::PANE,
        );
    }

    // 3. Corner ink dog-ear indicator (8.0px × 8.0px solid #0F172A right-triangle)
    // flush inside the trailing top corner (top-left in RTL, top-right in LTR) when notes exist.
    if has_notes {
        if is_rtl {
            let tl = rect.left_top();
            painter.add(Shape::convex_polygon(
                vec![tl, pos2(tl.x + 8.0, tl.y), pos2(tl.x, tl.y + 8.0)],
                theme::INK,
                Stroke::NONE,
            ));
        } else {
            let tr_corner = rect.right_top();
            painter.add(Shape::convex_polygon(
                vec![
                    tr_corner,
                    pos2(tr_corner.x - 8.0, tr_corner.y),
                    pos2(tr_corner.x, tr_corner.y + 8.0),
                ],
                theme::INK,
                Stroke::NONE,
            ));
        }
    }

    let badge_reserve = if root_number.is_some() {
        ROOT_BADGE_W
    } else {
        0.0
    };
    let content_rect = if is_rtl {
        Rect::from_min_max(
            pos2(rect.left() + m.pad_x, rect.top() + m.pad_y),
            pos2(
                (rect.right() - m.pad_x - badge_reserve).max(rect.left() + m.pad_x + 12.0),
                rect.bottom() - m.pad_y,
            ),
        )
    } else {
        Rect::from_min_max(
            pos2(rect.left() + m.pad_x + badge_reserve, rect.top() + m.pad_y),
            pos2(
                (rect.right() - m.pad_x).max(rect.left() + m.pad_x + badge_reserve + 12.0),
                rect.bottom() - m.pad_y,
            ),
        )
    };

    if is_editing {
        let mut card_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(if is_rtl { Align::Max } else { Align::Min })),
        );
        card_ui.set_clip_rect(content_rect);

        let response = rtl_text_edit(
            &mut card_ui,
            Id::new(&node_id),
            &mut node.title,
            s.task_title_placeholder,
            font_size,
            content_rect.width().max(32.0),
            Margin::same(0),
            theme::INK,
            is_rtl,
        )
        .response;

        if response.changed() {
            actions.dirty = true;
        }

        if request_focus_id.as_deref() == Some(node_id.as_str()) {
            response.request_focus();
            *request_focus_id = None;
        }

        let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let enter_raw = (response.has_focus() || response.lost_focus())
            && ui.input(|i| i.key_pressed(Key::Enter));
        let ctrl_enter = enter_raw && ctrl;
        let enter = enter_raw && !ctrl;
        let lost = response.lost_focus() && !enter_raw;

        if response.lost_focus() || enter_raw {
            *editing_node_id = None;
        }
        if ui.input(|i| i.key_pressed(Key::Tab)) {
            *editing_node_id = None;
            if ui.input(|i| i.modifiers.shift) {
                actions.outdent = Some(node_id.clone());
                *focused_node_id = Some(node_id.clone());
            } else {
                actions.add_child = Some(node_id.clone());
            }
        }
        if ctrl_enter {
            actions.focus_open_drawer = Some(node_id.clone());
        } else if enter {
            if node.title.trim().is_empty() {
                actions.delete_node = Some(node_id.clone());
                *focused_node_id = None;
            } else {
                actions.add_sibling = Some(node_id.clone());
            }
        } else if lost && node.title.trim().is_empty() {
            actions.delete_node = Some(node_id.clone());
            *focused_node_id = None;
        }
    } else {
        let card_response = ui
            .interact(rect, ui.id().with(("card", &node_id)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);

        // Generous 28×28 invisible dog-ear hit-zone (top-left in RTL, top-right in LTR).
        let dog_ear_rect = if is_rtl {
            Rect::from_min_size(rect.left_top(), vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE))
        } else {
            Rect::from_min_size(
                pos2(rect.right() - MIN_HITBOX_SIZE, rect.top()),
                vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
            )
        };
        let dog_ear_response = ui
            .interact(
                dog_ear_rect,
                ui.id().with(("dog_ear", &node_id)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);

        // Zero-width-penalty border-straddling fold handle (`[−]` / `[+N]`).
        let mut fold_handle_clicked = false;
        if let Some((vis_rect, hit_rect)) = fold_handle_rects {
            let fold_tip = if is_folded {
                disp(s.tip_expand_branch)
            } else {
                disp(s.tip_collapse_branch)
            };
            let handle_response = ui
                .interact(
                    hit_rect,
                    ui.id().with(("fold_handle", &node_id)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(fold_tip);
            if handle_response.has_focus() {
                draw_focus_ring(&ui.painter_at(col_clip), hit_rect);
            }
            if handle_response.clicked() {
                fold_handle_clicked = true;
                node.collapsed = !node.collapsed;
                if node.collapsed {
                    manually_expanded_nodes.remove(&node_id);
                } else {
                    manually_expanded_nodes.insert(node_id.clone());
                }
                actions.dirty = true;
            }
            let final_folded = node.is_folded();
            let final_hidden = if final_folded {
                node.descendant_count()
            } else {
                0
            };
            let final_vis_rect = if fold_handle_clicked {
                let sz = fold_handle_visual_size(ui, final_folded, final_hidden);
                Rect::from_center_size(fold_anchor_center, sz)
            } else {
                vis_rect
            };
            fold_handles.push(FoldHandleVisual {
                rect: final_vis_rect,
                clip_rect: col_clip,
                is_folded: final_folded,
                hidden_count: final_hidden,
                emphasized: hovering || handle_response.hovered(),
            });
        }

        let text_color = match node.status {
            TaskStatus::Active => ink_for_depth(depth),
            TaskStatus::Completed | TaskStatus::Cancelled => theme::SLATE,
        };
        let font_id = FontId::proportional(font_size);
        let (wrapped_text, _) =
            wrap_and_shape_rtl(ui, &node.title, font_id.clone(), content_rect.width());
        let is_arabic_text = crate::rtl_text::contains_arabic(&node.title);
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = if is_arabic_text {
            f32::INFINITY
        } else {
            content_rect.width().max(32.0)
        };
        job.halign = if is_rtl { Align::RIGHT } else { Align::LEFT };
        let mut format = egui::TextFormat {
            font_id,
            color: text_color,
            ..Default::default()
        };
        if node.status == TaskStatus::Cancelled {
            format.strikethrough = Stroke::new(1.2, text_color);
        }
        job.append(&wrapped_text, 0.0, format);
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let text_pos = pos2(
            if is_rtl {
                content_rect.right()
            } else {
                content_rect.left()
            },
            content_rect.center().y - galley.size().y * 0.5,
        );
        let text_painter = ui.painter_at(content_rect.expand(1.0).intersect(col_clip));
        if is_root {
            text_painter.galley(text_pos, galley.clone(), text_color);
            text_painter.galley(text_pos + vec2(0.45, 0.0), galley, text_color);
        } else {
            text_painter.galley(text_pos, galley, text_color);
        }

        if fold_handle_clicked {
            // Do not open note card or trigger inline rename.
        } else if dog_ear_response.clicked() {
            selected_node_ids.clear();
            selected_node_ids.insert(node_id.clone());
            *focused_node_id = Some(node_id.clone());
            actions.focus_open_drawer = Some(node_id.clone());
        } else if card_response.double_clicked() {
            selected_node_ids.clear();
            selected_node_ids.insert(node_id.clone());
            *focused_node_id = Some(node_id.clone());
            *editing_node_id = Some(node_id.clone());
            *request_focus_id = Some(node_id.clone());
            actions.focus_node = Some(node_id.clone());
        } else if card_response.clicked() {
            if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
                if is_selected {
                    selected_node_ids.remove(&node_id);
                } else {
                    selected_node_ids.insert(node_id.clone());
                }
                *focused_node_id = Some(node_id.clone());
            } else {
                selected_node_ids.clear();
                selected_node_ids.insert(node_id.clone());
                *focused_node_id = Some(node_id.clone());
                actions.select_open_drawer = Some(node_id.clone());
            }
        }

        card_response.context_menu(|ui| {
            render_card_context_menu(
                ui,
                &node_id,
                lang,
                focused_node_id,
                editing_node_id,
                request_focus_id,
                actions,
            );
        });
        dog_ear_response.context_menu(|ui| {
            render_card_context_menu(
                ui,
                &node_id,
                lang,
                focused_node_id,
                editing_node_id,
                request_focus_id,
                actions,
            );
        });

        if has_notes {
            dog_ear_response.on_hover_text(disp(s.ctx_notes));
        }
    }

    if let Some(priority) = hover_priority {
        hover_controls.push(HoverControl {
            node_id: node_id.clone(),
            strip_rect,
            check_rect,
            plus_rect,
            notes_rect,
            is_completed: node.status == TaskStatus::Completed,
            priority,
        });
    }

    actions.node_rects.push((node_id, rect));

    NodeAnchor {
        rect,
        hot: hovering,
    }
}

/// Shared localized right-click context menu for a subtask card.
pub(super) fn render_card_context_menu(
    ui: &mut Ui,
    node_id: &str,
    lang: AppLang,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    actions: &mut PaneActions,
) {
    let s = tr(lang);
    if ui
        .button(disp(s.ctx_active))
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.set_node_status = Some((node_id.to_string(), TaskStatus::Active));
    }
    if ui
        .button(disp(s.ctx_completed))
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.set_node_status = Some((node_id.to_string(), TaskStatus::Completed));
    }
    if ui
        .button(disp(s.ctx_cancelled))
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.set_node_status = Some((node_id.to_string(), TaskStatus::Cancelled));
    }
    ui.separator();
    if ui
        .button(disp(s.ctx_notes))
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.focus_open_drawer = Some(node_id.to_string());
    }
    if ui
        .button(disp(s.ctx_edit))
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        *editing_node_id = Some(node_id.to_string());
        *focused_node_id = Some(node_id.to_string());
        *request_focus_id = Some(node_id.to_string());
        actions.focus_node = Some(node_id.to_string());
    }
    if ui
        .button(disp(s.ctx_add_branch))
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.add_child = Some(node_id.to_string());
    }
    if ui
        .button(disp(s.ctx_delete_task))
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.delete_node = Some(node_id.to_string());
    }
}

/// Draws smooth cubic Bezier ink curves (`1.4px` `#334155`) connecting parent
/// and child cards (right-to-left in Arabic, left-to-right in English).
pub(super) fn draw_mind_map_threads(ui: &Ui, parent: NodeAnchor, children: &[NodeAnchor], is_rtl: bool) {
    let painter = ui.painter();

    for child in children {
        let hot = parent.hot || child.hot;
        let color = if hot { theme::INK } else { theme::INK_SOFT };
        let stroke = Stroke::new(if hot { 1.75 } else { 1.4 }, color);

        let (p0, p3, c1, c2, arrow_base_x) = if is_rtl {
            let p0 = parent.rect.left_center();
            let p3 = child.rect.right_center();
            let dx = (p0.x - p3.x).abs().max(1.0);
            (
                p0,
                p3,
                pos2(p0.x - dx * 0.45, p0.y),
                pos2(p3.x + dx * 0.45, p3.y),
                p3.x + 6.5,
            )
        } else {
            let p0 = parent.rect.right_center();
            let p3 = child.rect.left_center();
            let dx = (p3.x - p0.x).abs().max(1.0);
            (
                p0,
                p3,
                pos2(p0.x + dx * 0.45, p0.y),
                pos2(p3.x - dx * 0.45, p3.y),
                p3.x - 6.5,
            )
        };

        let curve = CubicBezierShape {
            points: [p0, c1, c2, p3],
            closed: false,
            fill: Color32::TRANSPARENT,
            stroke: stroke.into(),
        };
        painter.add(curve);

        painter.add(Shape::convex_polygon(
            vec![
                p3,
                pos2(arrow_base_x, p3.y - 3.2),
                pos2(arrow_base_x, p3.y + 3.2),
            ],
            color,
            Stroke::NONE,
        ));
    }
}

/// Paints the zero-width-penalty `14.0px × 14.0px` fold handles (`[−]` / `[+N]`)
/// straddling each parent card's left border center cleanly over Bezier origins,
/// strictly clipped to each handle's column rect.
pub(super) fn paint_fold_handles(ui: &Ui, handles: &[FoldHandleVisual]) {
    let font = FontId::monospace(11.0);
    for handle in handles {
        let painter = ui.painter_at(handle.clip_rect);
        if handle.is_folded {
            let label = format!("+{}", handle.hidden_count);
            painter.rect_filled(handle.rect, CornerRadius::same(0), theme::INK);
            painter.text(
                handle.rect.center(),
                egui::Align2::CENTER_CENTER,
                &label,
                font.clone(),
                theme::PANE,
            );
            painter.text(
                handle.rect.center() + vec2(0.5, 0.0),
                egui::Align2::CENTER_CENTER,
                &label,
                font.clone(),
                theme::PANE,
            );
        } else {
            let border_color = if handle.emphasized {
                theme::INK
            } else {
                theme::SLATE
            };
            painter.rect_filled(handle.rect, CornerRadius::same(0), theme::PANE);
            painter.rect_stroke(
                handle.rect,
                CornerRadius::same(0),
                Stroke::new(1.2, border_color),
                StrokeKind::Inside,
            );
            painter.text(
                handle.rect.center(),
                egui::Align2::CENTER_CENTER,
                "−",
                font.clone(),
                border_color,
            );
        }
    }
}

/// Renders the active `0.0px` contextual hover strip (`22.0px` height, `#0F172A`
/// background, `#FFFFFF` `13.5px` Bold icons: `✓`, `+`, `≡`) inside an
/// `Order::Foreground` interactive `Area` so it is never clipped by a `ScrollArea`
/// or blocked by underlying cards. Returns `(any_slot_hovered, any_action_fired)`.
pub(super) fn render_foreground_hover_strip(
    ctx: &egui::Context,
    control: &HoverControl,
    lang: AppLang,
    selected_node_ids: &mut HashSet<String>,
    focused_node_id: &mut Option<String>,
    actions: &mut PaneActions,
) -> (bool, bool) {
    let s = tr(lang);
    let mut any_hovered = false;
    let mut any_acted = false;
    let mut check_hovered = false;
    let mut plus_hovered = false;
    let mut notes_hovered = false;
    let mut check_focused = false;
    let mut plus_focused = false;
    let mut notes_focused = false;

    let area_id = Id::new("hover_strip_fg");
    egui::Area::new(area_id)
        .order(egui::Order::Foreground)
        .fixed_pos(control.strip_rect.min)
        .default_size(control.strip_rect.size())
        .constrain(false)
        .interactable(true)
        .fade_in(false)
        .show(ctx, |ui| {
            let _ = ui.allocate_exact_size(control.strip_rect.size(), Sense::hover());
            let check_resp = ui
                .interact(
                    control.check_rect,
                    ui.id().with(("hover_check", control.node_id.as_str())),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(disp(s.tip_toggle_complete));
            let plus_resp = ui
                .interact(
                    control.plus_rect,
                    ui.id().with(("hover_plus", control.node_id.as_str())),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(disp(s.ctx_add_branch));
            let notes_resp = ui
                .interact(
                    control.notes_rect,
                    ui.id().with(("hover_notes", control.node_id.as_str())),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(disp(s.ctx_notes));

            check_hovered = check_resp.hovered();
            plus_hovered = plus_resp.hovered();
            notes_hovered = notes_resp.hovered();
            check_focused = check_resp.has_focus();
            plus_focused = plus_resp.has_focus();
            notes_focused = notes_resp.has_focus();
            any_hovered = check_hovered || plus_hovered || notes_hovered;

            if check_resp.clicked() {
                let next_status = if control.is_completed {
                    TaskStatus::Active
                } else {
                    TaskStatus::Completed
                };
                actions.set_node_status = Some((control.node_id.clone(), next_status));
                any_acted = true;
            }
            if plus_resp.clicked() {
                actions.add_child = Some(control.node_id.clone());
                any_acted = true;
            }
            if notes_resp.clicked() {
                selected_node_ids.clear();
                selected_node_ids.insert(control.node_id.clone());
                *focused_node_id = Some(control.node_id.clone());
                actions.focus_open_drawer = Some(control.node_id.clone());
                any_acted = true;
            }
        });

    // Paint directly onto the Foreground layer so the strip renders on the very first
    // hover frame with zero sizing-pass flicker and never clips inside a ScrollArea.
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, area_id));
    let font = FontId::monospace(14.0);
    painter.rect_filled(control.strip_rect, CornerRadius::same(0), theme::INK);

    for (seg_rect, hovered, focused) in [
        (control.check_rect, check_hovered, check_focused),
        (control.plus_rect, plus_hovered, plus_focused),
        (control.notes_rect, notes_hovered, notes_focused),
    ] {
        if hovered {
            painter.rect_filled(seg_rect, CornerRadius::same(0), theme::GRID);
        }
        if focused {
            draw_focus_ring(&painter, seg_rect);
        }
    }

    let sep_stroke = Stroke::new(1.0, theme::INK_SOFT);
    for sep_x in [control.plus_rect.left(), control.notes_rect.left()] {
        painter.line_segment(
            [
                pos2(sep_x, control.strip_rect.top() + 4.0),
                pos2(sep_x, control.strip_rect.bottom() - 4.0),
            ],
            sep_stroke,
        );
    }

    let check_color = if control.is_completed {
        theme::GREEN_SHADOW
    } else {
        theme::PANE
    };
    for (rect, glyph, color) in [
        (control.check_rect, "✓", check_color),
        (control.plus_rect, "+", theme::PANE),
        (control.notes_rect, "≡", theme::PANE),
    ] {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            font.clone(),
            color,
        );
        painter.text(
            rect.center() + vec2(0.5, 0.0),
            egui::Align2::CENTER_CENTER,
            glyph,
            font.clone(),
            color,
        );
    }

    (any_hovered, any_acted)
}
