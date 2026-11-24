use super::*;

fn sample_data() -> AppData {
    let mut data = AppData::default();
    data.language = Some(AppLang::Ar);
    data.has_seen_onboarding = true;
    let mut pane = MainTaskPane::new("علاج بابا");
    pane.status = TaskStatus::Active;

    let mut child = TaskNode::new("كتابة اللائحة للمرافعة");
    child.status = TaskStatus::Completed;
    child.children.push(TaskNode::new("مراجعة مع المحامي"));

    let mut cancelled = TaskNode::new("التقديم على المناقصة");
    cancelled.status = TaskStatus::Cancelled;
    pane.subtasks.push(child);
    pane.subtasks.push(cancelled);

    data.current_day.panes.push(pane);
    data.current_day.panes.push(MainTaskPane::new("مهمة ثانية"));
    data
}

#[test]
fn renders_daily_tiled_ui_headless() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;

    // Two frames so fonts and layout settle.
    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let output = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let shapes = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
    assert!(!shapes.is_empty(), "expected the board to draw geometry");

    // Regression: pane and subtask text must actually be laid out.
    let text = rendered_text(&output);
    for expected in ["علاج بابا", "كتابة اللائحة للمرافعة", "مهمة ثانية"]
    {
        let shaped = crate::rtl_text::shape_arabic(expected);
        assert!(
            text.contains(&shaped),
            "missing rendered text {expected:?}; got {text:?}"
        );
    }
}

#[test]
fn renders_focused_pane_headless() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    app.focused_pane_id = Some(app.data.current_day.panes[0].id.clone());

    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let output = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let text = rendered_text(&output);
    assert!(
        text.contains(&crate::rtl_text::shape_arabic("علاج بابا")),
        "focused pane title not rendered"
    );
}

#[test]
fn nested_child_creation_pushes_into_subtask_and_focuses() {
    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;

    // Top-level subtask of pane 0.
    let subtask_id = app.data.current_day.panes[0].subtasks[0].id.clone();

    let new_id = app
        .add_child_under(&subtask_id)
        .expect("child must be created");
    let subtasks = &app.data.current_day.panes[0].subtasks[0];
    // sample_data's subtask already carries one child; the new one is appended.
    assert!(subtasks.children.iter().any(|n| n.id == new_id));
    assert!(!subtasks.collapsed);
    assert_eq!(app.editing_node_id.as_deref(), Some(new_id.as_str()));
    assert_eq!(app.next_focus_node_id.as_deref(), Some(new_id.as_str()));

    // Deeper nesting must recurse into grandchild vectors too.
    let child_id = subtasks
        .children
        .iter()
        .find(|n| n.id == new_id)
        .unwrap()
        .id
        .clone();
    assert!(app.add_child_under(&child_id).is_some());
    let grand = &app.data.current_day.panes[0].subtasks[0]
        .children
        .iter()
        .find(|n| n.id == new_id)
        .unwrap();
    assert_eq!(grand.children.len(), 1);
}

#[test]
fn nested_child_renders_after_add() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    let subtask_id = app.data.current_day.panes[0].subtasks[0].id.clone();
    let _ = app.add_child_under(&subtask_id);

    // Simulate the action arriving from the + button at end-of-frame.
    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let output = ctx.run(Default::default(), |ctx| app.draw(ctx));
    // The nested level must actually be rendered: the pre-existing
    // grandchild text proves the recursive branch rendered.
    let text = rendered_text(&output);
    assert!(
        text.contains(&crate::rtl_text::shape_arabic("مراجعة مع المحامي")),
        "nested level not rendered, got {text:?}"
    );
    // Connectors are smooth cubic Bezier curves plus solid arrowhead paths.
    let has_beziers = output
        .shapes
        .iter()
        .any(|c| matches!(c.shape, egui::Shape::CubicBezier(_)));
    let has_paths = output
        .shapes
        .iter()
        .any(|c| matches!(c.shape, egui::Shape::Path(_)));
    assert!(
        has_beziers && has_paths,
        "expected cubic Bezier curves and arrowhead geometry"
    );
}

#[test]
fn renders_zen_mode_headless() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    app.is_zen = true;

    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let output = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let text = rendered_text(&output);

    // Zen keeps the clock header and the board itself.
    assert!(
        text.contains(&shape_arabic("تركيز")),
        "zen header missing, got {text:?}"
    );
    assert!(
        text.contains(&crate::rtl_text::shape_arabic("علاج بابا")),
        "zen must still render today's tasks"
    );
    // Zen hides all navigation chrome.
    assert!(!text.contains(&shape_arabic("الأرشيف")), "nav tabs leaked into zen");
    assert!(!text.contains(&shape_arabic("محفوظ تلقائياً")), "footer leaked into zen");
}

/// The horizontal RTL mind-map must place child cards to the LEFT of their
/// parent card, vertically centred on the parent, and size cards dynamically
/// to fit their text up to 42% of the pane width.
#[test]
fn mind_map_columns_grow_to_the_left() {
    let m = mind_map_metrics(false, 1000.0);
    assert_eq!(m.font_size, 15.5);
    assert_eq!(m.font_size_deep, 14.5);
    assert_eq!(m.pad_x, 14.0);
    assert_eq!(m.pad_y, 8.0);
    assert_eq!(m.gap_y, 8.0);
    assert!((m.max_card_w - 380.0).abs() < 0.001);

    // A parent's children are centred on the parent's vertical position.
    let mut parent = TaskNode::new("parent");
    parent.children.push(TaskNode::new("a"));
    parent.children.push(TaskNode::new("b"));
    parent.children.push(TaskNode::new("c"));

    let short_node = TaskNode::new("مراجعة");
    let long_node = TaskNode::new(
        "كتابة اللائحة للمرافعة وتجهيز جميع المستندات القانونية المطلوبة للجلسة القادمة في المحكمة",
    );

    let root_nodes = vec![parent];
    let mut root_centers = Vec::new();
    let mut child_centers = Vec::new();
    let mut short_size = Vec2::ZERO;
    let mut long_size = Vec2::ZERO;
    let mut total_w = 0.0;
    {
        let ctx = egui::Context::default();
        crate::rtl_text::install_arabic_fonts(&ctx);
        crate::theme::install(&ctx);
        let root_nodes = &root_nodes;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                root_centers = stack_centers(ui, root_nodes, &m, 0, 300.0, None);
                child_centers =
                    stack_centers(ui, &root_nodes[0].children, &m, 1, root_centers[0], None);
                short_size = measure_card_size(ui, &short_node, &m, 0, false);
                long_size = measure_card_size(ui, &long_node, &m, 0, false);
                total_w = stack_width(ui, root_nodes, &m, 0, None);
            });
        });
    }
    assert_eq!(root_centers.len(), 1);

    let parent_center = root_centers[0];
    assert_eq!(child_centers.len(), 3);
    let mid = (child_centers[0] + child_centers[2]) * 0.5;
    assert!(
        (mid - parent_center).abs() < 0.001,
        "children must be centred on their parent"
    );
    // Siblings never overlap and are spaced by 8.0px.
    assert!(child_centers[1] - child_centers[0] >= m.min_card_h + m.gap_y - 0.01);

    // Dynamic sizing: short cards stay compact; long cards wrap at 42% width.
    assert!(
        short_size.x < long_size.x,
        "short card ({short_size:?}) must be narrower than long card ({long_size:?})"
    );
    assert!(
        long_size.x <= m.max_card_w + 0.01,
        "long card width {} exceeded 42% cap {}",
        long_size.x,
        m.max_card_w
    );
    assert!(
        long_size.y > short_size.y,
        "long card should wrap onto multiple lines ({long_size:?} vs {short_size:?})"
    );
    assert!(total_w > m.gap_x);
}

/// Paginated 4-pane-max grid layout: 1 full, 2 side-by-side (50/50), 3-4 clean 2x2.
#[test]
fn page_grid_layout_enforces_max_four_panes_in_two_columns() {
    let w = 1000.0;
    let h = 600.0;

    // 1 pane: full width and height.
    let one = compute_page_grid_layout(&[0], w, h);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].size, vec2(w, h));

    // 2 panes: 2 side-by-side columns (50% / 50%), master on the right.
    let two = compute_page_grid_layout(&[0, 1], w, h);
    assert_eq!(two.len(), 2);
    assert!(two[0].pos.x > two[1].pos.x);
    assert!((two[0].size.x - w * 0.5).abs() < 0.01);
    assert!((two[0].size.y - h).abs() < 0.01);

    // 3 panes: master full height on the right, 2 stacked on the left.
    let three = compute_page_grid_layout(&[0, 1, 2], w, h);
    assert_eq!(three.len(), 3);
    assert!(three[0].pos.x > three[1].pos.x);
    assert!((three[0].size.y - h).abs() < 0.01);
    assert!(three[1].pos.y < three[2].pos.y);
    assert!((three[1].size.y - h * 0.5).abs() < 0.01);

    // 4 panes: clean 2x2 grid (50% width × 50% height each).
    let four = compute_page_grid_layout(&[0, 1, 2, 3], w, h);
    assert_eq!(four.len(), 4);
    assert!(four.iter().all(|t| (t.size.x - w * 0.5).abs() < 0.01));
    assert!(four.iter().all(|t| (t.size.y - h * 0.5).abs() < 0.01));
    assert!(four[0].pos.x > four[1].pos.x);
    assert!(four[2].pos.x > four[3].pos.x);
    assert!(four[0].pos.y < four[2].pos.y);

    // Passing >4 indices is strictly capped at 4 panes per page.
    let capped = compute_page_grid_layout(&[0, 1, 2, 3, 4], w, h);
    assert_eq!(capped.len(), 4);
}

/// The fit-to-pane zoom must shrink (never enlarge) and stay in bounds.
#[test]
fn fit_scale_zooms_out_within_bounds() {
    // Already fits: no zoom out.
    assert_eq!(fit_scale(100.0, 100.0, 400.0, 400.0), 1.0);
    // Too tall: zoom out by the height ratio.
    let s = fit_scale(500.0, 100.0, 400.0, 400.0);
    assert!((s - 0.8).abs() < 0.001, "{s}");
    // Too wide: zoom out by the width ratio.
    let s = fit_scale(100.0, 500.0, 400.0, 400.0);
    assert!((s - 0.8).abs() < 0.001, "{s}");
    // A ratio below the 55% floor is clamped.
    assert_eq!(fit_scale(800.0, 100.0, 400.0, 400.0), 0.55);
    // Never below 55%, never above 100%.
    assert_eq!(fit_scale(10_000.0, 10_000.0, 100.0, 100.0), 0.55);
    assert_eq!(fit_scale(10.0, 10.0, 10_000.0, 10_000.0), 1.0);
}

#[test]
fn mind_map_metrics_respect_comfortable_width_and_zoom_bounds() {
    let m = mind_map_metrics(false, 500.0);
    assert!((m.max_card_w - 280.0).abs() < 0.01);
    assert_eq!(m.font_size, 15.5);
    assert_eq!(m.font_size_deep, 14.5);
    assert_eq!(m.pad_x, 14.0);
    assert_eq!(m.pad_y, 8.0);
    assert_eq!(m.gap_y, 8.0);
    let zoomed = m.scaled(0.75);
    assert!(zoomed.max_card_w < m.max_card_w);
    assert!(zoomed.font_size >= 10.0);
}

#[test]
fn rtl_caret_maps_arabic_end_to_left_leading_edge() {
    // Logical end of Arabic text (typing position) maps to visual index 0 (left edge).
    let ar = "سلام";
    assert_eq!(rtl_visual_cursor_index(ar, ar.chars().count()), 0);
    assert_eq!(rtl_visual_cursor_index(ar, 0), ar.chars().count());
    // Latin text stays left-to-right.
    assert_eq!(rtl_visual_cursor_index("hello", 5), 5);
    assert_eq!(rtl_visual_cursor_index("hello", 0), 0);

    // Hint with Arabic tanwin preserves exact char count and attaches combining marks cleanly.
    let hint = "+ اكتب فرعاً جديداً...";
    let shaped = shape_arabic(hint);
    assert_eq!(shaped.chars().count(), hint.chars().count());
}

#[test]
fn labels_are_non_selectable_so_no_ibeam_on_hover() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);
    assert!(
        !ctx.style().interaction.selectable_labels,
        "non-editable labels must not trigger I-beam text cursor"
    );
}

#[test]
fn hard_shadows_replace_status_icons_and_sel_legend_is_gone() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    let first_id = app.data.current_day.panes[0].subtasks[0].id.clone();
    app.selected_node_ids.insert(first_id);

    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let output = ctx.run(Default::default(), |ctx| app.draw(ctx));

    // Top bar must not display the old SEL legend even when items are selected.
    let text = rendered_text(&output);
    assert!(!text.contains("SEL "), "SEL legend should be removed: {text:?}");

    // Sample data has Active, Completed, and Cancelled nodes; all three hard
    // shadow colors must be painted as filled rectangles.
    let mut saw_amber = false;
    let mut saw_emerald = false;
    let mut saw_red = false;
    for clipped in &output.shapes {
        if let egui::Shape::Rect(r) = &clipped.shape {
            if r.fill == theme::AMBER_SHADOW {
                saw_amber = true;
            } else if r.fill == theme::GREEN_SHADOW {
                saw_emerald = true;
            } else if r.fill == theme::RED_SHADOW {
                saw_red = true;
            }
        }
    }
    assert!(saw_amber, "missing Amber (#F59E0B) hard shadow");
    assert!(saw_emerald, "missing Emerald (#10B981) hard shadow");
    assert!(saw_red, "missing Red (#EF4444) hard shadow");
}

#[test]
fn live_branch_input_appends_clears_and_retains_focus_on_enter() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    let pane_id = app.data.current_day.panes[0].id.clone();
    let initial_len = app.data.current_day.panes[0].subtasks.len();

    // First frame mounts the live TextEdit.
    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));

    // Focus the pane's live TextEdit, populate text, and press Enter.
    let input_id = Id::new(("pane_branch_input", pane_id.as_str()));
    ctx.memory_mut(|m| m.request_focus(input_id));
    app.pane_inputs
        .insert(pane_id.clone(), "فرع جديد مباشر".to_string());

    let mut raw = egui::RawInput::default();
    raw.events.push(egui::Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(raw, |ctx| app.draw(ctx));

    // Branch appended, input cleared, focus retained on the live TextEdit.
    assert_eq!(app.data.current_day.panes[0].subtasks.len(), initial_len + 1);
    assert_eq!(
        app.data.current_day.panes[0].subtasks.last().unwrap().title,
        "فرع جديد مباشر"
    );
    assert!(
        app.pane_inputs.get(&pane_id).unwrap().is_empty(),
        "input field should be cleared after Enter"
    );
    assert!(
        ctx.memory(|m| m.has_focus(input_id)),
        "live branch TextEdit should retain focus after Enter"
    );
}

fn rendered_text(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => Some(text.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn tree_helpers_insert_outdent_delete() {
    let mut nodes = vec![TaskNode::new("a")];
    let root_id = nodes[0].id.clone();

    let child = TaskNode::new("b");
    let child_id = child.id.clone();
    assert!(insert_child_recursive(&mut nodes, &root_id, child));
    assert_eq!(nodes[0].children.len(), 1);

    assert!(insert_sibling_in_nodes(
        &mut nodes,
        &child_id,
        TaskNode::new("c")
    ));
    assert_eq!(nodes[0].children.len(), 2);

    assert!(outdent_recursive(&mut nodes, &child_id));
    assert_eq!(nodes.len(), 2);

    assert!(delete_node_in_nodes(&mut nodes, &child_id));
    assert_eq!(nodes.len(), 1);
}

#[test]
fn dog_ear_indicator_renders_only_when_notes_non_empty() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;

    let count_dog_ears = |output: &egui::FullOutput| -> usize {
        output
            .shapes
            .iter()
            .filter(|c| match &c.shape {
                egui::Shape::Path(p) => {
                    p.closed
                        && p.fill == theme::INK
                        && p.points.len() == 3
                        && (p.points[1].x - p.points[0].x - 8.0).abs() < 0.01
                        && (p.points[2].y - p.points[0].y - 8.0).abs() < 0.01
                }
                _ => false,
            })
            .count()
    };

    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let empty_out = ctx.run(Default::default(), |ctx| app.draw(ctx));
    assert_eq!(count_dog_ears(&empty_out), 0, "whitespace/empty notes must not draw dog-ear");

    app.data.current_day.panes[0].subtasks[0].notes = "تفاصيل مهمة".to_string();
    let with_notes_out = ctx.run(Default::default(), |ctx| app.draw(ctx));
    assert_eq!(
        count_dog_ears(&with_notes_out),
        1,
        "expected one 8x8 #0F172A top-left dog-ear triangle"
    );
}

#[test]
fn inline_note_drawer_opens_on_n_and_ctrl_enter_and_closes_on_escape() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    let pane_id = app.data.current_day.panes[0].id.clone();
    let node_id = app.data.current_day.panes[0].subtasks[0].id.clone();
    app.selected_node_ids.insert(node_id.clone());
    app.focused_node_id = Some(node_id.clone());

    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));

    // Pressing `N` while a card is selected opens the embedded pane note drawer.
    let mut raw_n = egui::RawInput::default();
    raw_n.events.push(egui::Event::Key {
        key: Key::N,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let output = ctx.run(raw_n, |ctx| app.draw(ctx));
    assert_eq!(
        app.pane_note_drawers.get(&pane_id).map(String::as_str),
        Some(node_id.as_str())
    );

    let text = rendered_text(&output);
    assert!(
        text.contains(&shape_arabic("كتابة اللائحة للمرافعة")),
        "note card header missing branch title: {text:?}"
    );
    assert!(
        text.contains(&shape_arabic("ملاحظات")),
        "note card header missing localized NOTE tag: {text:?}"
    );
    assert!(
        text.contains(&shape_arabic("نسخ")),
        "note card header missing localized COPY action: {text:?}"
    );
    assert!(
        output.shapes.iter().any(|c| matches!(
            &c.shape,
            egui::Shape::Rect(r) if r.fill == theme::DRAWER_BG
        )),
        "expected #F8FAFC paper background on architectural note card"
    );
    assert!(
        output.shapes.iter().any(|c| matches!(
            &c.shape,
            egui::Shape::Rect(r)
                if r.stroke.color == theme::INK
                    && (r.stroke.width - 1.5).abs() < 0.05
                    && r.rect.height() > 60.0
        )),
        "expected 1.5px #0F172A border on architectural note card"
    );
    assert!(
        output.shapes.iter().any(|c| matches!(
            &c.shape,
            egui::Shape::Rect(r) if r.fill == theme::INK && r.rect.height() > 60.0
        )),
        "expected 3.0px solid #0F172A hard offset shadow behind note card"
    );
    assert!(
        output.shapes.iter().any(|c| matches!(
            &c.shape,
            egui::Shape::Rect(r)
                if r.fill == theme::SUBTLE_RULE && (r.rect.height() - 1.0).abs() < 0.05
        )),
        "expected 1.0px #E2E8F0 header divider inside note card"
    );

    // Pressing `Esc` closes the embedded drawer.
    let mut raw_esc = egui::RawInput::default();
    raw_esc.events.push(egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(raw_esc, |ctx| app.draw(ctx));
    assert!(app.pane_note_drawers.is_empty());

    // Pressing `Ctrl+Enter` while selected also opens the embedded drawer.
    let mut raw_ctrl_enter = egui::RawInput::default();
    raw_ctrl_enter.modifiers = egui::Modifiers::CTRL;
    raw_ctrl_enter.events.push(egui::Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let _ = ctx.run(raw_ctrl_enter, |ctx| app.draw(ctx));
    assert_eq!(
        app.pane_note_drawers.get(&pane_id).map(String::as_str),
        Some(node_id.as_str())
    );
}

#[test]
fn clicking_card_and_dog_ear_opens_inline_note_drawer() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    let pane_id = app.data.current_day.panes[0].id.clone();
    let node_id = app.data.current_day.panes[0].subtasks[0].id.clone();
    app.data.current_day.panes[0].subtasks[0].notes = "ملاحظة".to_string();

    // Settle layout and locate the card's exact top-left corner from its 8x8 dog-ear triangle.
    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let output = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let dog_ear_tl = output
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Path(p)
                if p.closed
                    && p.fill == theme::INK
                    && p.points.len() == 3
                    && (p.points[1].x - p.points[0].x - 8.0).abs() < 0.01 =>
            {
                Some(p.points[0])
            }
            _ => None,
        })
        .expect("dog-ear triangle should be painted");

    // Click inside the 16×16 top-left hit-zone (at +10, +10 from corner).
    let hit = dog_ear_tl + vec2(10.0, 10.0);
    let mut press = egui::RawInput::default();
    press.events.push(egui::Event::PointerMoved(hit));
    press.events.push(egui::Event::PointerButton {
        pos: hit,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(press, |ctx| app.draw(ctx));

    let mut release = egui::RawInput::default();
    release.events.push(egui::Event::PointerMoved(hit));
    release.events.push(egui::Event::PointerButton {
        pos: hit,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(release, |ctx| app.draw(ctx));

    assert_eq!(
        app.pane_note_drawers.get(&pane_id).map(String::as_str),
        Some(node_id.as_str()),
        "clicking top-left 16x16 hit-zone must open inline note drawer"
    );
}

#[test]
fn clean_top_bar_paginates_four_panes_and_master_index_manages_pages() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    for i in 1..=5 {
        let mut p = MainTaskPane::new(&format!("Pane {}", i));
        p.subtasks.push(TaskNode::new(&format!("HiddenBranch {}", i)));
        data.current_day.panes.push(p);
    }
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    assert_eq!(app.total_pages(), 2);
    assert_eq!(app.current_page, 0);

    // Page 1 shows panes 1..=4, clean top bar with `PAGE 1 / 2` and no ZEN button.
    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let out_p1 = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let text_p1 = rendered_text(&out_p1);
    assert!(text_p1.contains("HOME"));
    assert!(text_p1.contains("INDEX"));
    assert!(text_p1.contains("HARBOR"));
    assert!(!text_p1.contains("BOARD"));
    assert!(text_p1.contains("PAGE 1 / 2"), "got {text_p1:?}");
    assert!(!text_p1.contains("ZEN [Ctrl+H]"), "ZEN button must be removed");
    assert!(text_p1.contains("Pane 4"));
    assert!(!text_p1.contains("Pane 5"), "page 1 must show at most 4 panes");

    // PageDown flips to page 2 showing only Pane 5.
    let mut pgdn = egui::RawInput::default();
    pgdn.events.push(egui::Event::Key {
        key: Key::PageDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let out_p2 = ctx.run(pgdn, |ctx| app.draw(ctx));
    assert_eq!(app.current_page, 1);
    let text_p2 = rendered_text(&out_p2);
    assert!(text_p2.contains("PAGE 2 / 2"));
    assert!(text_p2.contains("Pane 5"));
    assert!(!text_p2.contains("Pane 1"));

    // Alt+Left flips back to page 1.
    let mut alt_left = egui::RawInput::default();
    alt_left.modifiers = egui::Modifiers::ALT;
    alt_left.events.push(egui::Event::Key {
        key: Key::ArrowLeft,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::ALT,
    });
    let _ = ctx.run(alt_left, |ctx| app.draw(ctx));
    assert_eq!(app.current_page, 0);

    // Adding a new main pane automatically jumps to the page containing it (page 2).
    app.add_main_pane("Pane 6");
    assert_eq!(app.current_page, 1);
    assert_eq!(app.total_pages(), 2);

    // Master Index view lists only main titles grouped into PAGE 01 / PAGE 02 (no branches).
    app.show_master_index = true;
    let out_idx = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let text_idx = rendered_text(&out_idx);
    assert!(text_idx.contains("PAGE 01"), "missing PAGE 01 divider: {text_idx:?}");
    assert!(text_idx.contains("PAGE 02"), "missing PAGE 02 divider: {text_idx:?}");
    assert!(text_idx.contains("[P1]"));
    assert!(text_idx.contains("[P2]"));
    assert!(text_idx.contains("Pane 1"));
    assert!(text_idx.contains("Pane 6"));
    assert!(
        !text_idx.contains("HiddenBranch 1"),
        "Master Index must not show branch trees"
    );
    // Verify 2.0px solid #0F172A horizontal page divider exists.
    assert!(
        out_idx.shapes.iter().any(|c| matches!(
            &c.shape,
            egui::Shape::Rect(r)
                if r.fill == theme::INK && (r.rect.height() - 2.0).abs() < 0.05
        )),
        "expected 2.0px #0F172A horizontal divider in Master Index"
    );

    // `[P1]` badge cycling moves Pane 1 (idx 0) directly onto Page 2 (idx 4), and `[P2]` cycles it back to Page 1 (idx 0).
    app.move_pane_to_next_page(0);
    assert_eq!(app.data.current_day.panes[4].title, "Pane 1");
    app.move_pane_to_next_page(4);
    assert_eq!(app.data.current_day.panes[0].title, "Pane 1");

    // `↑` / `↓` shifts panes across page boundaries (idx 3 -> idx 4 moves Pane 4 to Page 2).
    app.move_pane_down(3);
    assert_eq!(app.data.current_day.panes[4].title, "Pane 4");
    app.move_pane_up(4);
    assert_eq!(app.data.current_day.panes[3].title, "Pane 4");
}

#[test]
fn header_progress_hairline_fills_right_to_left_in_emerald() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    // Add a completed leaf subtask to pane 0 so pane 0 has 1 completed leaf, 1 active leaf,
    // and 1 cancelled leaf (excluded from denominator) -> ratio 1/2 (50%).
    let mut data = sample_data();
    let mut done_leaf = TaskNode::new("مهمة مكتملة");
    done_leaf.status = TaskStatus::Completed;
    data.current_day.panes[0].subtasks.push(done_leaf);
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let output = ctx.run(Default::default(), |ctx| app.draw(ctx));

    // Find 2.0px tall rectangles painted in #CBD5E1 (left) and #10B981 (right) at the same y.
    let mut left_tracks: Vec<Rect> = Vec::new();
    let mut right_fills: Vec<Rect> = Vec::new();
    for clipped in &output.shapes {
        if let egui::Shape::Rect(r) = &clipped.shape {
            if (r.rect.height() - 2.0).abs() < 0.05 {
                if r.fill == theme::HINT {
                    left_tracks.push(r.rect);
                } else if r.fill == theme::GREEN_SHADOW {
                    right_fills.push(r.rect);
                }
            }
        }
    }

    // At least one pair shares the same top y, with green flush on the right of the gray track.
    let matched = left_tracks.iter().any(|track| {
        right_fills.iter().any(|fill| {
            (track.top() - fill.top()).abs() < 0.1
                && (track.right() - fill.left()).abs() < 0.5
                && fill.right() > track.right()
        })
    });
    assert!(
        matched,
        "expected 2.0px header progress hairline filled right-to-left in #10B981 with #CBD5E1 left segment; tracks={left_tracks:?}, fills={right_fills:?}"
    );
}

#[test]
fn branch_folding_hides_descendants_and_shows_plus_n_badge() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    // Add a second child so parent has 2 descendants.
    app.data.current_day.panes[0].subtasks[0]
        .children
        .push(TaskNode::new("مراجعة نهائية"));
    app.data.current_day.panes[0].subtasks[0].collapsed = true;

    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let output = ctx.run(Default::default(), |ctx| app.draw(ctx));

    let text = rendered_text(&output);
    assert!(
        text.contains("+2"),
        "expected +2 collapsed fold handle for folded parent; got {text:?}"
    );
    assert!(
        output.shapes.iter().any(|c| match &c.shape {
            egui::Shape::Rect(r) => {
                r.fill == theme::INK
                    && (r.rect.height() - FOLD_HANDLE_SIZE).abs() < 0.1
                    && r.rect.width() >= FOLD_HANDLE_SIZE - 0.1
            }
            _ => false,
        }),
        "expected solid #0F172A 14.0px fold handle square for collapsed branch"
    );
    assert!(
        !text.contains(&shape_arabic("مراجعة مع المحامي")),
        "collapsed descendant text must be hidden"
    );
    assert!(
        !output
            .shapes
            .iter()
            .any(|c| matches!(c.shape, egui::Shape::CubicBezier(_))),
        "Bezier curves for collapsed branch must be hidden"
    );
}

#[test]
fn arabic_typing_while_focused_does_not_deadlock_rwlock() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    let pane_id = app.data.current_day.panes[0].id.clone();

    // Mount the live branch TextEdit.
    let mut raw = egui::RawInput::default();
    raw.focused = true;
    let _ = ctx.run(raw, |ctx| app.draw(ctx));

    // Focus the RTL TextEdit and type Arabic text.
    let input_id = Id::new(("pane_branch_input", pane_id.as_str()));
    ctx.memory_mut(|m| m.request_focus(input_id));

    let mut type_ar = egui::RawInput::default();
    type_ar.focused = true;
    type_ar.events.push(egui::Event::Text("سلام".to_string()));
    let _ = ctx.run(type_ar, |ctx| app.draw(ctx));

    // Next idle frame while still focused (`changed() == false`): previously
    // deadlocked in `data_mut` calling `response.gained_focus()` (`ctx.memory`).
    let mut idle = egui::RawInput::default();
    idle.focused = true;
    idle.time = Some(1.0);
    let _ = ctx.run(idle, |ctx| app.draw(ctx));

    assert_eq!(
        app.pane_inputs.get(&pane_id).map(String::as_str),
        Some("سلام")
    );

    // Also exercise multiline Arabic typing in the embedded pane note drawer.
    let node_id = app.data.current_day.panes[0].subtasks[0].id.clone();
    app.open_note_drawer(node_id, true);
    // Frame 2.0 mounts the drawer and requests focus on the notes editor;
    // frame 2.5 types Arabic text into the focused editor; frame 3.0 idles.
    for t in [2.0, 2.5, 3.0] {
        let mut step = egui::RawInput::default();
        step.focused = true;
        step.time = Some(t);
        if t == 2.5 {
            step.events
                .push(egui::Event::Text("ملاحظة عربية".to_string()));
        }
        let _ = ctx.run(step, |ctx| app.draw(ctx));
    }
    assert!(app.data.current_day.panes[0].subtasks[0]
        .notes
        .contains("ملاحظة عربية"));
}

#[test]
fn undo_stack_is_bounded_and_ctrl_z_restores_state_with_4s_toast() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;

    // Verify undo stack caps at 30 states.
    for _ in 0..45 {
        app.push_undo_snapshot();
    }
    assert_eq!(app.undo_stack.len(), MAX_UNDO_STACK);
    app.undo_stack.clear();

    let original_count = app.data.current_day.panes[0].subtasks.len();
    let victim_id = app.data.current_day.panes[0].subtasks[0].id.clone();

    // Delete a branch, then press Ctrl+Z at t = 10.0.
    app.delete_node(&victim_id);
    assert_eq!(app.data.current_day.panes[0].subtasks.len(), original_count - 1);

    let mut raw_undo = egui::RawInput::default();
    raw_undo.time = Some(10.0);
    raw_undo.modifiers = egui::Modifiers::CTRL;
    raw_undo.events.push(egui::Event::Key {
        key: Key::Z,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let out_undo = ctx.run(raw_undo, |ctx| app.draw(ctx));
    assert_eq!(
        app.data.current_day.panes[0].subtasks.len(),
        original_count,
        "Ctrl+Z must restore the deleted branch"
    );
    let text_undo = rendered_text(&out_undo);
    assert!(
        text_undo.contains(&shape_arabic("تم التراجع (Ctrl+Z)")),
        "expected 4-second undo toast in status bar; got {text_undo:?}"
    );

    // After 4.5s the toast vanishes cleanly.
    let mut raw_later = egui::RawInput::default();
    raw_later.time = Some(14.5);
    let out_later = ctx.run(raw_later, |ctx| app.draw(ctx));
    assert!(app.undo_toast.is_none());
    assert!(!rendered_text(&out_later).contains(&shape_arabic("تم التراجع (Ctrl+Z)")));
}

#[test]
fn hover_strip_is_zero_overlap() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    app.data.current_day.panes[0].subtasks[0].notes = "ملاحظة".to_string();

    // Idle frame: no hover strip rendered (zero clutter).
    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let idle_out = ctx.run(Default::default(), |ctx| app.draw(ctx));
    assert!(
        !rendered_text(&idle_out).contains("≡"),
        "hover strip must stay hidden when idle"
    );

    // Locate the card's top-left corner from its 8×8 dog-ear triangle.
    let card_tl = idle_out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Path(p)
                if p.closed
                    && p.fill == theme::INK
                    && p.points.len() == 3
                    && (p.points[1].x - p.points[0].x - 8.0).abs() < 0.01 =>
            {
                Some(p.points[0])
            }
            _ => None,
        })
        .expect("dog-ear triangle must be present");

    // Hover over the card body -> 22px #0F172A strip appears flush at `y - 22.0` (strictly above card_tl.y).
    let mut hover = egui::RawInput::default();
    hover
        .events
        .push(egui::Event::PointerMoved(card_tl + vec2(30.0, 16.0)));
    let hover_out = ctx.run(hover, |ctx| app.draw(ctx));
    let hover_text = rendered_text(&hover_out);
    assert!(hover_text.contains("✓"), "missing ✓ icon in hover strip");
    assert!(hover_text.contains("+"), "missing + icon in hover strip");
    assert!(hover_text.contains("≡"), "missing ≡ icon in hover strip");

    let strip_above_card = hover_out.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            r.fill == theme::INK
                && (r.rect.height() - HOVER_STRIP_H).abs() < 0.05
                && (r.rect.bottom() - card_tl.y).abs() < 0.1
        }
        _ => false,
    });
    assert!(
        strip_above_card,
        "expected 22.0px #0F172A hover strip flush against top outer edge (y - 22.0)"
    );
}

#[test]
fn adaptive_density_horizontal_gap_and_balanced_two_column_split() {
    // <= 7 nodes: spacious padding (14 H, 8 V, 8 gap_y) and 56.0px horizontal branch gap.
    let sparse = mind_map_metrics_for_density(600.0, 5);
    assert_eq!(sparse.gap_x, 56.0);
    assert_eq!(sparse.pad_x, 14.0);
    assert_eq!(sparse.pad_y, 8.0);
    assert_eq!(sparse.gap_y, 8.0);

    // > 7 nodes: compact padding (12 H, 5 V, 5 gap_y) and 56.0px horizontal branch gap.
    let dense = mind_map_metrics_for_density(600.0, 8);
    assert_eq!(dense.gap_x, 56.0);
    assert_eq!(dense.pad_x, 12.0);
    assert_eq!(dense.pad_y, 5.0);
    assert_eq!(dense.gap_y, 5.0);

    // Balanced split minimizes taller half's height.
    assert_eq!(find_balanced_split(&[40.0, 40.0, 40.0, 40.0], 5.0), 2);
    assert_eq!(find_balanced_split(&[180.0, 40.0, 40.0, 40.0], 5.0), 1);

    // Dense pane (> available height) auto-splits into 2 halves separated by 1.0px #E2E8F0 hairline,
    // and reserves the 38.0px bottom input strip with 1.0px #E2E8F0 top border.
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::Ar);
    data.has_seen_onboarding = true;
    let mut pane = MainTaskPane::new("مهمة كثيفة");
    for i in 1..=16 {
        let mut n = TaskNode::new(&format!("فرع {}", i));
        if i == 1 || i == 9 {
            n.children.push(TaskNode::new(&format!("تفصيل {}", i)));
        }
        pane.subtasks.push(n);
    }
    data.current_day.panes.push(pane);

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 500.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out = ctx.run(raw, |ctx| app.draw(ctx));

    // Subtle 1.0px #E2E8F0 vertical column divider exists.
    let has_vertical_split = out.shapes.iter().any(|c| match &c.shape {
        egui::Shape::LineSegment { points, stroke } => {
            stroke.color == theme::SUBTLE_RULE
                && (stroke.width - 1.0).abs() < 0.05
                && (points[0].x - points[1].x).abs() < 0.1
                && (points[1].y - points[0].y).abs() > 40.0
        }
        _ => false,
    });
    assert!(
        has_vertical_split,
        "expected 1.0px #E2E8F0 vertical hairline separating balanced 2-column pane split"
    );

    // Dedicated 38.0px bottom input strip top border (`1.0px` `#E2E8F0`).
    let has_input_border = out.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            r.fill == theme::SUBTLE_RULE
                && (r.rect.height() - 1.0).abs() < 0.05
                && r.rect.width() > 400.0
        }
        _ => false,
    });
    assert!(
        has_input_border,
        "expected 1.0px #E2E8F0 top border on dedicated 38.0px bottom input strip"
    );

    // Open note drawer and verify 10x10 painter-drawn vector X cross close button.
    let first_id = app.data.current_day.panes[0].subtasks[0].id.clone();
    app.open_note_drawer(first_id, false);
    let mut raw2 = egui::RawInput::default();
    raw2.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 500.0)));
    let out_drawer = ctx.run(raw2, |ctx| app.draw(ctx));
    let x_diagonals = out_drawer
        .shapes
        .iter()
        .filter(|c| match &c.shape {
            egui::Shape::LineSegment { points, .. } => {
                let dx = (points[1].x - points[0].x).abs();
                let dy = (points[1].y - points[0].y).abs();
                (dx - 10.0).abs() < 0.1 && (dy - 10.0).abs() < 0.1
            }
            _ => false,
        })
        .count();
    assert_eq!(
        x_diagonals, 2,
        "expected crisp 10x10 painter-drawn X cross (2 diagonals) in note drawer header"
    );
}

#[test]
fn multiline_arabic_wrapping_preserves_natural_top_to_bottom_order() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let phrase = "الأول الثاني الثالث الرابع الخامس السادس السابع الثامن";
    let mut wrapped = String::new();
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            wrapped = wrap_and_shape_rtl(ui, phrase, FontId::proportional(15.5), 140.0).0;
        });
    });

    let lines: Vec<&str> = wrapped.split('\n').collect();
    assert!(
        lines.len() >= 2,
        "expected Arabic phrase to wrap onto multiple lines; got {wrapped:?}"
    );
    // Line 0 (top) must contain the shaped first word ("الأول"), NOT the last word ("الثامن").
    let first_word = shape_arabic("الأول");
    let last_word = shape_arabic("الثامن");
    assert!(
        lines[0].contains(&first_word),
        "top line must hold the start of the Arabic sentence; lines={lines:?}"
    );
    assert!(
        lines.last().unwrap().contains(&last_word),
        "bottom line must hold the end of the Arabic sentence; lines={lines:?}"
    );
}

#[test]
fn root_only_numbering_hierarchy_and_focus_hides_bottom_placeholder() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    let pane_id = app.data.current_day.panes[0].id.clone();

    // Unfocused: placeholder visible; root cards have 01/02 badges, child has none;
    // child card uses #F8FAFC paper fill and 1.0px #64748B border.
    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let out_idle = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let text_idle = rendered_text(&out_idle);
    assert!(
        text_idle.contains(&shape_arabic("+ اكتب فرعاً جديداً...")),
        "placeholder should be visible while unfocused"
    );
    assert!(text_idle.contains("01"), "root card 1 must show 01 badge");
    assert!(text_idle.contains("02"), "root card 2 must show 02 badge");
    assert!(
        !text_idle.contains("\n03\n"),
        "child sub-branches must NOT be numbered (sample_data pane 0 has 2 roots + 1 child)"
    );

    let has_root_border = out_idle.shapes.iter().any(|c| matches!(
        &c.shape,
        egui::Shape::Rect(r)
            if r.stroke.color == theme::INK && (r.stroke.width - 1.5).abs() < 0.05
    ));
    let has_child_style = out_idle.shapes.iter().any(|c| matches!(
        &c.shape,
        egui::Shape::Rect(r) if r.fill == theme::DRAWER_BG
    )) && out_idle.shapes.iter().any(|c| matches!(
        &c.shape,
        egui::Shape::Rect(r)
            if r.stroke.color == theme::SLATE && (r.stroke.width - 1.0).abs() < 0.05
    ));
    assert!(has_root_border, "root cards must use 1.5px #0F172A border");
    assert!(
        has_child_style,
        "child sub-branches must use #F8FAFC fill and 1.0px #64748B border"
    );

    // Focusing the bottom input hides its placeholder immediately so caret never overlaps `+`.
    let input_id = Id::new(("pane_branch_input", pane_id.as_str()));
    ctx.memory_mut(|m| m.request_focus(input_id));
    // Clear pane 1 so only pane 0's focused input is checked cleanly.
    app.data.current_day.panes.truncate(1);
    let out_focused = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let text_focused = rendered_text(&out_focused);
    assert!(
        !text_focused.contains(&shape_arabic("+ اكتب فرعاً جديداً...")),
        "placeholder must hide immediately when bottom input has focus; got {text_focused:?}"
    );
}

#[test]
fn fold_handle_click_separation_and_parent_double_click_rename() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    app.data.current_day.panes.truncate(1);
    let parent_id = app.data.current_day.panes[0].subtasks[0].id.clone();

    // Warm-up frames: expanded parent shows subtle 14×14 handle (#FFFFFF fill, 1.2px #64748B border, −).
    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let out_expanded = ctx.run(Default::default(), |ctx| app.draw(ctx));
    assert!(
        rendered_text(&out_expanded).contains("−"),
        "expanded parent with children must render subtle − fold handle"
    );
    let handle_rect = out_expanded
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Rect(r)
                if r.stroke.color == theme::SLATE
                    && (r.stroke.width - 1.2).abs() < 0.05
                    && (r.rect.width() - FOLD_HANDLE_SIZE).abs() < 0.1
                    && (r.rect.height() - FOLD_HANDLE_SIZE).abs() < 0.1 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("expected 14.0x14.0 expanded fold handle straddling parent left border");
    assert!(
        out_expanded.shapes.iter().any(|c| match &c.shape {
            egui::Shape::Rect(r) => {
                r.fill == theme::PANE
                    && (r.rect.min.x - handle_rect.min.x).abs() < 0.1
                    && (r.rect.min.y - handle_rect.min.y).abs() < 0.1
                    && (r.rect.width() - FOLD_HANDLE_SIZE).abs() < 0.1
            }
            _ => false,
        }),
        "expanded fold handle must have a crisp #FFFFFF fill"
    );

    // 1. Click fold handle -> collapses children ONLY (no note card opened, no inline rename).
    let click_at = |pos: Pos2, t: f64, app: &mut WhiteboardApp| {
        let mut down = egui::RawInput::default();
        down.time = Some(t);
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = egui::RawInput::default();
        up.time = Some(t + 0.05);
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(up, |ctx| app.draw(ctx))
    };

    let _ = click_at(handle_rect.center(), 1.0, &mut app);
    assert!(
        app.data.current_day.panes[0].subtasks[0].collapsed,
        "clicking fold handle must collapse parent children"
    );
    assert!(
        app.pane_note_drawers.is_empty(),
        "clicking fold handle must NOT open the note card"
    );
    assert!(
        app.editing_node_id.is_none(),
        "clicking fold handle must NOT enter inline rename"
    );

    // 2. Click fold handle again -> expands children and records manual override.
    let _ = click_at(handle_rect.center(), 2.0, &mut app);
    assert!(
        !app.data.current_day.panes[0].subtasks[0].collapsed,
        "clicking collapsed fold handle must expand parent children"
    );
    assert!(
        app.manually_expanded_nodes.contains(&parent_id),
        "manually expanding a folded group must record override against auto-collapse"
    );
    assert!(
        app.pane_note_drawers.is_empty(),
        "re-expanding fold handle must NOT open note card"
    );

    // 3. Single-click card body -> opens its light paper note card after the double-click window.
    let card_body_pos = pos2(handle_rect.center().x + 48.0, handle_rect.center().y);
    let _ = click_at(card_body_pos, 3.0, &mut app);
    let mut after_delay = egui::RawInput::default();
    after_delay.time = Some(3.40);
    let _ = ctx.run(after_delay, |ctx| app.draw(ctx));
    assert_eq!(
        app.pane_note_drawers
            .get(&app.data.current_day.panes[0].id)
            .map(String::as_str),
        Some(parent_id.as_str()),
        "single-clicking card body must open its light paper note card"
    );
    assert!(
        app.editing_node_id.is_none(),
        "single-clicking card body must NOT start inline rename"
    );

    // 4. Close drawer, then double-click parent card body -> enters inline rename mode
    //    WITHOUT opening the Note Drawer.
    app.pane_note_drawers.clear();
    let _ = click_at(card_body_pos, 4.0, &mut app);
    assert!(
        app.pane_note_drawers.is_empty(),
        "first click of a double-click must NOT immediately open the Note Drawer"
    );
    let _ = click_at(card_body_pos, 4.12, &mut app);
    let mut post_dbl = egui::RawInput::default();
    post_dbl.time = Some(4.60);
    let _ = ctx.run(post_dbl, |ctx| app.draw(ctx));
    assert_eq!(
        app.editing_node_id.as_deref(),
        Some(parent_id.as_str()),
        "double-clicking parent card body must enter inline rename mode"
    );
    assert!(
        app.pane_note_drawers.is_empty(),
        "double-clicking a card must start inline rename without opening the Note Drawer"
    );
}

#[test]
fn auto_collapse_only_folds_completed_groups_when_overflowing() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::Ar);
    data.has_seen_onboarding = true;
    let mut pane = MainTaskPane::new("لوحة طويلة");

    // Group 0: 100% Completed children -> MUST auto-collapse on vertical overflow.
    let mut completed_group = TaskNode::new("مجموعة مكتملة");
    for i in 1..=5 {
        let mut c = TaskNode::new(&format!("منجز {}", i));
        c.status = TaskStatus::Completed;
        completed_group.children.push(c);
    }
    let completed_id = completed_group.id.clone();

    // Group 1: Active/in-progress children -> MUST NEVER auto-collapse.
    let mut active_group = TaskNode::new("مجموعة نشطة");
    for i in 1..=5 {
        active_group
            .children
            .push(TaskNode::new(&format!("نشط {}", i)));
    }

    // Group 2: 100% Completed children, but manually expanded by user -> MUST stay expanded.
    let mut manual_group = TaskNode::new("مجموعة مفتوحة يدوياً");
    for i in 1..=4 {
        let mut c = TaskNode::new(&format!("مكتمل يدوي {}", i));
        c.status = TaskStatus::Completed;
        manual_group.children.push(c);
    }
    let manual_id = manual_group.id.clone();

    pane.subtasks = vec![completed_group, active_group, manual_group];
    data.current_day.panes.push(pane);

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;
    app.manually_expanded_nodes.insert(manual_id);

    // Constrained viewport height forces vertical overflow so the auto-collapse policy triggers.
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 360.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let _ = ctx.run(raw, |ctx| app.draw(ctx));

    let subtasks = &app.data.current_day.panes[0].subtasks;
    assert_eq!(subtasks[0].id, completed_id);
    assert!(
        subtasks[0].collapsed,
        "100% completed group must auto-collapse when pane exceeds available vertical height"
    );
    assert!(
        !subtasks[1].collapsed,
        "active/in-progress group must NEVER auto-collapse"
    );
    assert!(
        !subtasks[2].collapsed,
        "manually expanded completed group must stay expanded"
    );
}

#[test]
fn split_columns_clip_strictly_with_gutter_compact_bezier_and_zero_overlap() {
    // Split-column metrics use a compact 30.0px Bezier span and cap child cards
    // so `root + 30px + child` fits cleanly inside one half-column.
    let split_m = mind_map_metrics_for_split_col(280.0, 12);
    assert_eq!(split_m.gap_x, SPLIT_BRANCH_GAP_X);
    assert_eq!(SPLIT_BRANCH_GAP_X, 30.0);
    assert_eq!(SPLIT_COL_GUTTER, 8.0);
    assert!(
        split_m.max_card_w + split_m.gap_x + split_m.child_max_card_w <= 280.0 - SHADOW_PAD + 0.1,
        "root ({}) + gap ({}) + child ({}) must fit within split column width",
        split_m.max_card_w,
        split_m.gap_x,
        split_m.child_max_card_w
    );

    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::Ar);
    data.has_seen_onboarding = true;
    let mut pane = MainTaskPane::new("لوحة منقسمة");
    for i in 1..=9 {
        let title = if i == 6 {
            "بسم الله الرحمن الرحيم".to_string()
        } else {
            format!("فرع رئيسي رقم {}", i)
        };
        let mut n = TaskNode::new(&title);
        if i == 2 {
            n.children
                .push(TaskNode::new(";koljm;' طويل جداً لاختبار التداخل بين الأعمدة"));
        }
        pane.subtasks.push(n);
    }
    data.current_day.panes.push(pane);

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(700.0, 360.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out = ctx.run(raw, |ctx| app.draw(ctx));

    // Locate the 1.0px #E2E8F0 center vertical divider.
    let mid_x = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::LineSegment { points, stroke }
                if stroke.color == theme::SUBTLE_RULE
                    && (stroke.width - 1.0).abs() < 0.05
                    && (points[0].x - points[1].x).abs() < 0.1
                    && (points[1].y - points[0].y).abs() > 40.0 =>
            {
                Some(points[0].x)
            }
            _ => None,
        })
        .expect("expected 1.0px #E2E8F0 vertical divider between split columns");

    // Every painted card rect and clip rect inside the columns must respect the 8.0px
    // inner gutter on both sides of `mid_x` (never straddling `mid_x - 8 .. mid_x + 8`).
    let gutter_left = mid_x - SPLIT_COL_GUTTER + 0.5;
    let gutter_right = mid_x + SPLIT_COL_GUTTER - 0.5;
    for clipped in &out.shapes {
        if let egui::Shape::Rect(r) = &clipped.shape {
            let is_card = r.fill == theme::DRAWER_BG
                || (r.stroke.color == theme::INK && (r.stroke.width - 1.5).abs() < 0.05)
                || (r.stroke.color == theme::SLATE && (r.stroke.width - 1.0).abs() < 0.05);
            if is_card {
                let visible = r.rect.intersect(clipped.clip_rect);
                if visible.is_positive() {
                    let crosses_gutter =
                        visible.left() < gutter_right && visible.right() > gutter_left;
                    assert!(
                        !crosses_gutter,
                        "card visible rect {visible:?} crossed the 16px center gutter [{gutter_left:.1}, {gutter_right:.1}] around mid_x={mid_x:.1}"
                    );
                }
            }
        }
    }

    // Active child tree in Right Column stays expanded (never auto-collapsed).
    assert!(
        !app.data.current_day.panes[0].subtasks[1].collapsed,
        "active child branch must remain expanded"
    );
}

#[test]
fn split_column_scroll_area_reserves_bottom_shadow_and_padding() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::Ar);
    data.has_seen_onboarding = true;
    let mut pane = MainTaskPane::new("لوحة عميقة");
    for i in 1..=14 {
        pane.subtasks.push(TaskNode::new(&format!("مهمة رقم {:02}", i)));
    }
    data.current_day.panes.push(pane);

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 320.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out = ctx.run(raw, |ctx| app.draw(ctx));

    // Verify the bottom card in the scrollable column does NOT get painted below its
    // clipped ScrollArea (all card clip rects stay above the 38px bottom input strip).
    let input_border_y = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Rect(r)
                if r.fill == theme::SUBTLE_RULE
                    && (r.rect.height() - 1.0).abs() < 0.05
                    && r.rect.width() > 300.0 =>
            {
                Some(r.rect.top())
            }
            _ => None,
        })
        .expect("expected bottom input strip top border");

    for clipped in &out.shapes {
        if let egui::Shape::Rect(r) = &clipped.shape {
            if r.stroke.color == theme::INK && (r.stroke.width - 1.5).abs() < 0.05 {
                assert!(
                    clipped.clip_rect.bottom() <= input_border_y + 0.5,
                    "card clip rect bottom ({}) must stay strictly above bottom input strip ({})",
                    clipped.clip_rect.bottom(),
                    input_border_y
                );
            }
        }
    }

    // Scroll the Left Column all the way down and verify the last card (`14`) is fully visible
    // with its 3.0px shadow + 12.0px bottom padding above `input_border_y`.
    let mut scroll_raw = egui::RawInput::default();
    scroll_raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 320.0)));
    scroll_raw
        .events
        .push(egui::Event::PointerMoved(pos2(200.0, 180.0)));
    scroll_raw.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -2000.0),
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(scroll_raw.clone(), |ctx| app.draw(ctx));
    let scrolled_out = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 320.0))),
            ..Default::default()
        },
        |ctx| app.draw(ctx),
    );

    let last_card_bottom = scrolled_out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r)
                if r.stroke.color == theme::INK
                    && (r.stroke.width - 1.5).abs() < 0.05
                    && r.rect.right() < 400.0 =>
            {
                Some(r.rect.bottom())
            }
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    assert!(
        last_card_bottom > 0.0
            && last_card_bottom + SHADOW_OFFSET.y + BOTTOM_SCROLL_PAD <= input_border_y + 1.0,
        "scrolled bottom card ({last_card_bottom:.1}) + 3px shadow + 12px pad must fit cleanly above input strip ({input_border_y:.1})"
    );
}

#[test]
fn sticky_hover_strip_bridge_foreground_click_and_300ms_grace_delay() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    app.data.current_day.panes.truncate(1);
    // Use the LOWER sibling (`subtasks[1]`, which sits directly beneath `subtasks[0]`)
    // to prove moving upward onto its `✓ + ≡` strip is NEVER stolen by the sibling above!
    app.data.current_day.panes[0].subtasks[1].status = TaskStatus::Active;
    app.data.current_day.panes[0].subtasks[1].notes = "ملاحظة".to_string();
    let lower_id = app.data.current_day.panes[0].subtasks[1].id.clone();

    let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
    let idle = ctx.run(Default::default(), |ctx| app.draw(ctx));

    let card_tl = idle
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Path(p)
                if p.closed
                    && p.fill == theme::INK
                    && p.points.len() == 3
                    && (p.points[1].x - p.points[0].x - 8.0).abs() < 0.01 =>
            {
                Some(p.points[0])
            }
            _ => None,
        })
        .expect("dog-ear triangle must locate lower card top-left");

    // 1. Hover lower card body -> centered foreground strip renders flush at `strip.bottom == card_tl.y`.
    let mut hover_card = egui::RawInput::default();
    hover_card
        .events
        .push(egui::Event::PointerMoved(card_tl + vec2(30.0, 16.0)));
    let _ = ctx.run(hover_card.clone(), |ctx| app.draw(ctx));
    let out_card = ctx.run(hover_card, |ctx| app.draw(ctx));
    let strip_rect = out_card
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Rect(r)
                if r.fill == theme::INK && (r.rect.height() - HOVER_STRIP_H).abs() < 0.05 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("expected sticky hover strip when hovering card");
    assert!(
        (strip_rect.bottom() - card_tl.y).abs() < 0.05,
        "strip must sit flush against card top edge with zero gap"
    );
    assert_eq!(
        app.active_hover_card.as_ref().map(|(id, _)| id.as_str()),
        Some(lower_id.as_str())
    );

    // 2. Move pointer into `card_rect.union(strip_rect).expand(8.0)` padded bridge.
    let mut hover_bridge = egui::RawInput::default();
    hover_bridge
        .events
        .push(egui::Event::PointerMoved(pos2(strip_rect.right() + 6.0, card_tl.y - 6.0)));
    let out_bridge = ctx.run(hover_bridge, |ctx| app.draw(ctx));
    assert!(
        rendered_text(&out_bridge).contains("≡"),
        "strip must stay visible inside card.union(strip).expand(8.0) bridge zone"
    );

    // 3. Move pointer directly onto the `✓` slot (which overlaps the sibling card above!)
    //    and click it -> lower card toggles to Completed and strip is NOT stolen by sibling.
    let check_center = pos2(strip_rect.left() + HOVER_STRIP_SLOT_W * 0.5, strip_rect.center().y);
    let mut hover_check = egui::RawInput::default();
    hover_check.events.push(egui::Event::PointerMoved(check_center));
    let _ = ctx.run(hover_check, |ctx| app.draw(ctx));
    assert_eq!(
        app.active_hover_card.as_ref().map(|(id, _)| id.as_str()),
        Some(lower_id.as_str()),
        "moving onto the active strip must NEVER be stolen by a neighboring card"
    );

    let mut press_check = egui::RawInput::default();
    press_check.events.push(egui::Event::PointerMoved(check_center));
    press_check.events.push(egui::Event::PointerButton {
        pos: check_center,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(press_check, |ctx| app.draw(ctx));

    let mut release_check = egui::RawInput::default();
    release_check.events.push(egui::Event::PointerMoved(check_center));
    release_check.events.push(egui::Event::PointerButton {
        pos: check_center,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(release_check, |ctx| app.draw(ctx));
    assert_eq!(
        app.data.current_day.panes[0].subtasks[1].status,
        TaskStatus::Completed,
        "clicking ✓ on foreground hover strip must toggle the card's status reliably"
    );

    // 4. Move pointer completely outside -> strip remains visible during the 300ms grace timer.
    let mut hover_far = egui::RawInput::default();
    hover_far
        .events
        .push(egui::Event::PointerMoved(pos2(5.0, 5.0)));
    app.active_hover_card = Some((lower_id.clone(), Instant::now() - Duration::from_millis(240)));
    let out_grace = ctx.run(hover_far.clone(), |ctx| app.draw(ctx));
    assert!(
        rendered_text(&out_grace).contains("≡"),
        "strip must stay visible at 240ms inside the 300ms grace window"
    );
    assert!(
        app.active_hover_card.is_some(),
        "active_hover_card must stay active during 300ms grace window"
    );

    // 5. After 300ms elapses, strip dismisses cleanly.
    app.active_hover_card = Some((lower_id, Instant::now() - Duration::from_millis(320)));
    let out_expired = ctx.run(hover_far, |ctx| app.draw(ctx));
    assert!(
        !rendered_text(&out_expired).contains("≡"),
        "strip must dismiss cleanly once the 300ms grace timer expires"
    );
    assert!(app.active_hover_card.is_none());
}

#[test]
fn responsive_top_bar_prevents_clock_and_page_switcher_overlap_across_widths() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    for i in 1..=5 {
        data.current_day
            .panes
            .push(MainTaskPane::new(&format!("Pane {}", i)));
    }
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let (time_now, _) = clock_strings();
    let date_now = short_date_string();

    let render_at_width = |w: f32, app: &mut WhiteboardApp| -> egui::FullOutput {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(w, 600.0)));
        let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
        ctx.run(raw, |ctx| app.draw(ctx))
    };

    let assert_no_top_bar_overlap = |out: &egui::FullOutput, w: f32| {
        let mut left_max_x = 0.0_f32;
        let mut page_min_x = f32::INFINITY;
        let mut page_max_x = 0.0_f32;
        let mut right_min_x = f32::INFINITY;

        for c in &out.shapes {
            if let egui::Shape::Text(t) = &c.shape {
                if t.pos.y >= 36.0 {
                    continue;
                }
                let txt = t.galley.text();
                let min_x = t.pos.x;
                let max_x = t.pos.x + t.galley.size().x;
                if txt == "HOME" || txt == "INDEX" || txt == "HARBOR" || txt == "AR" || txt == "EN" {
                    left_max_x = left_max_x.max(max_x);
                } else if txt == "‹" || txt == "›" || txt.starts_with("PAGE ") {
                    page_min_x = page_min_x.min(min_x);
                    page_max_x = page_max_x.max(max_x);
                } else if txt == "\u{d7}" || txt == "\u{25a1}" || txt == "\u{2014}" || txt == time_now || txt == date_now {
                    right_min_x = right_min_x.min(min_x);
                }
            }
        }
        assert!(
            page_min_x >= left_max_x + 10.0,
            "at width {w}: center page switcher ({page_min_x:.1}) collided with left zone ({left_max_x:.1})"
        );
        assert!(
            right_min_x >= page_max_x + 10.0,
            "at width {w}: right zone ({right_min_x:.1}) collided with center page switcher ({page_max_x:.1})"
        );
    };

    // 1. Wide window (>= 1050px): full date + time shown, zero overlap.
    let out_wide = render_at_width(1100.0, &mut app);
    let wide_txt = rendered_text(&out_wide);
    assert!(wide_txt.contains(&date_now), "expected full date at 1100px");
    assert!(wide_txt.contains(&time_now), "expected time at 1100px");
    assert!(wide_txt.contains("PAGE 1 / 2"));
    assert_no_top_bar_overlap(&out_wide, 1100.0);

    // 2. Medium window (820..1050px): compact time only, date hidden, zero overlap.
    let out_mid = render_at_width(900.0, &mut app);
    let mid_txt = rendered_text(&out_mid);
    assert!(!mid_txt.contains(&date_now), "date must hide below 1050px");
    assert!(mid_txt.contains(&time_now), "compact time must remain visible at 900px");
    assert!(mid_txt.contains("PAGE 1 / 2"));
    assert_no_top_bar_overlap(&out_mid, 900.0);

    // 3. Narrow window (< 820px): clock hidden completely, window controls + page switcher clean.
    let out_narrow = render_at_width(760.0, &mut app);
    let narrow_txt = rendered_text(&out_narrow);
    assert!(!narrow_txt.contains(&date_now), "date must hide below 820px");
    assert!(!narrow_txt.contains(&time_now), "time must hide below 820px");
    assert!(narrow_txt.contains("PAGE 1 / 2"));
    assert!(narrow_txt.contains("\u{d7}") && narrow_txt.contains("\u{25a1}") && narrow_txt.contains("\u{2014}"));
    assert_no_top_bar_overlap(&out_narrow, 760.0);
}

#[test]
fn top_bar_tabs_single_click_immediately_and_page_keys_work_while_input_focused() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    for i in 1..=5 {
        data.current_day.panes.push(MainTaskPane::new(&format!("مهمة {}", i)));
    }
    let pane0_id = data.current_day.panes[0].id.clone();
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_home = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    // Active HOME tab displays a crisp 2.0px #0F172A underline at bottom of top bar.
    let has_active_underline = out_home.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            r.fill == theme::INK
                && (r.rect.height() - 2.0).abs() < 0.05
                && r.rect.bottom() <= 36.5
                && r.rect.width() >= 30.0
                && r.rect.width() <= 100.0
        }
        _ => false,
    });
    assert!(has_active_underline, "expected 2.0px #0F172A active underline on HOME tab");

    // Even while the live bottom branch TextEdit has keyboard focus on HOME,
    // PageDown flips immediately to page 2.
    let input_id = Id::new(("pane_branch_input", pane0_id.as_str()));
    ctx.memory_mut(|m| m.request_focus(input_id));
    let mut pgdn = raw.clone();
    pgdn.events.push(egui::Event::Key {
        key: Key::PageDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(pgdn, |ctx| app.draw(ctx));
    assert_eq!(
        app.current_page, 1,
        "PageDown must flip pages immediately from anywhere on HOME"
    );

    // Single-clicking the `INDEX` tab switches immediately on the very first click.
    let index_pos = out_home
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "INDEX" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("INDEX tab must be rendered in top bar");

    let click_at = |pos: Pos2, app: &mut WhiteboardApp| {
        let mut down = raw.clone();
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = raw.clone();
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(up, |ctx| app.draw(ctx))
    };

    let out_index = click_at(index_pos, &mut app);
    assert!(
        app.show_master_index && app.active_mode == AppMode::LiveBoard,
        "single-clicking INDEX must open Master Index on the first click"
    );
    let idx_text = rendered_text(&out_index);
    assert!(!idx_text.contains('✕'), "broken ✕ glyph must not exist in INDEX");
    assert!(!idx_text.contains('✎'), "broken ✎ glyph must not exist in INDEX");
    assert!(idx_text.contains("ARCHIVE"), "expected ARCHIVE action in INDEX");
    assert!(idx_text.contains("DELETE"), "expected DELETE action in INDEX");
    assert!(idx_text.contains("EDIT"), "expected EDIT action in INDEX");

    // Single-clicking `HARBOR` switches immediately.
    let harbor_pos = out_index
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "HARBOR" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("HARBOR tab must be rendered in top bar");
    let _ = click_at(harbor_pos, &mut app);
    assert_eq!(app.active_mode, AppMode::Harbor);
    assert!(!app.show_master_index);

    // Single-clicking `HOME` switches back immediately.
    let home_pos = out_index
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "HOME" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("HOME tab must be rendered in top bar");
    let _ = click_at(home_pos, &mut app);
    assert_eq!(app.active_mode, AppMode::LiveBoard);
    assert!(!app.show_master_index);
}

#[test]
fn index_row_selection_keyboard_reorder_inline_rename_and_archive() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    for i in 1..=5 {
        data.current_day
            .panes
            .push(MainTaskPane::new(&format!("Pane {}", i)));
    }
    let pane4_id = data.current_day.panes[3].id.clone();
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;
    app.show_master_index = true;
    app.index_selected_pane_id = Some(pane4_id.clone());

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 640.0)));
    let out_sel = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    // Selected row renders #F1F5F9 fill + 1.5px #0F172A border.
    let has_sel_fill = out_sel.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => r.fill == theme::SEL_FILL && (r.rect.height() - 46.0).abs() < 0.5,
        _ => false,
    });
    let has_sel_border = out_sel.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            r.stroke.color == theme::INK
                && (r.stroke.width - 1.5).abs() < 0.05
                && (r.rect.height() - 46.0).abs() < 0.5
        }
        _ => false,
    });
    assert!(has_sel_fill && has_sel_border, "selected INDEX row must show #F1F5F9 fill + 1.5px #0F172A border");

    // Pressing Down arrow moves Pane 4 (index 3, end of Page 1) across page boundary to index 4 (Page 2).
    let mut down_key = raw.clone();
    down_key.events.push(egui::Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(down_key, |ctx| app.draw(ctx));
    assert_eq!(app.data.current_day.panes[4].id, pane4_id);

    // Pressing Up arrow moves it right back to index 3.
    let mut up_key = raw.clone();
    up_key.events.push(egui::Event::Key {
        key: Key::ArrowUp,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(up_key, |ctx| app.draw(ctx));
    assert_eq!(app.data.current_day.panes[3].id, pane4_id);

    // Pressing F2 enters inline rename; modifying title then pressing Esc restores original title.
    let mut f2_key = raw.clone();
    f2_key.events.push(egui::Event::Key {
        key: Key::F2,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(f2_key, |ctx| app.draw(ctx));
    assert_eq!(app.index_editing_pane_id.as_deref(), Some(pane4_id.as_str()));
    app.data.current_day.panes[3].title = "تغيير مؤقت".to_string();

    let mut esc_key = raw.clone();
    esc_key.events.push(egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(esc_key, |ctx| app.draw(ctx));
    assert!(app.index_editing_pane_id.is_none());
    assert_eq!(
        app.data.current_day.panes[3].title, "Pane 4",
        "pressing Esc during inline rename must restore original title"
    );

    // Archiving moves pane into Harbor archive and Ctrl+Z restores it cleanly.
    app.archive_pane_at(3, Some(&ctx));
    assert_eq!(app.data.current_day.panes.len(), 4);
    assert_eq!(app.data.archived_panes.len(), 1);
    assert_eq!(app.data.archived_panes[0].id, pane4_id);
    assert!(app.undo(Some(&ctx)));
    assert_eq!(app.data.current_day.panes.len(), 5);
    assert!(app.data.archived_panes.is_empty());
}

#[test]
fn note_drawer_aligns_arabic_flush_right_places_rtl_caret_and_has_clean_copy() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    app.data.current_day.panes.truncate(1);
    let node_id = app.data.current_day.panes[0].subtasks[0].id.clone();
    let logical_note = "سطر عربي أول\nسطر ثاني أطول قليلاً";
    app.data.current_day.panes[0].subtasks[0].notes = logical_note.to_string();
    app.open_note_drawer(node_id.clone(), true);

    let mut raw = egui::RawInput::default();
    raw.focused = true;
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out = ctx.run(raw, |ctx| app.draw(ctx));

    // 1. Mutable logical buffer must NEVER be mutated with reordered bidi glyphs.
    assert_eq!(
        app.data.current_day.panes[0].subtasks[0].notes,
        logical_note
    );

    // 2. Clean COPY button renders with NO tiny 7x8 square icon box next to it.
    assert!(rendered_text(&out).contains(&shape_arabic("نسخ")));
    let has_tiny_copy_box = out.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            (r.rect.width() - 7.0).abs() < 0.2 && (r.rect.height() - 8.0).abs() < 0.2
        }
        _ => false,
    });
    assert!(!has_tiny_copy_box, "tiny square icon next to COPY must be removed");

    // 3. Locate the #F8FAFC paper card rect and verify Arabic lines sit flush against right 16.0px padding.
    let paper_rect = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Rect(r)
                if r.fill == theme::DRAWER_BG && r.rect.width() > 400.0 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("expected #F8FAFC architectural note card");

    let expected_right = paper_rect.right() - 1.5 - 16.0;
    let note_text_shape = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.rows.len() >= 2 => Some(t),
            _ => None,
        })
        .expect("expected multiline Arabic galley in note drawer");
    for (i, row) in note_text_shape.galley.rows.iter().enumerate() {
        let row_right = note_text_shape.pos.x + row.rect().max.x;
        assert!(
            (row_right - expected_right).abs() <= 4.0,
            "Arabic note row {i} right edge ({row_right:.1}) must sit flush against right 16px padding ({expected_right:.1})"
        );
    }

    // 4. Verify multiline RTL cursor mapping places end-of-line cursor at left edge (visual col 0).
    let shaped = shape_arabic("مرحبا");
    assert_eq!(rtl_multiline_visual_cursor_index("مرحبا", &shaped, 5), 0);
    assert_eq!(rtl_multiline_visual_cursor_index("مرحبا", &shaped, 0), 5);
    let two_lines_shaped = format!("{}\n{}", shape_arabic("مرحبا"), shape_arabic("عالم"));
    assert_eq!(
        rtl_multiline_visual_cursor_index("مرحبا\nعالم", &two_lines_shaped, 10),
        6
    );
}

#[test]
fn daily_profiles_migrate_legacy_sort_newest_first_auto_create_and_carry_over_active_only() {
    // 1. Legacy JSON save file (no `profiles` field, only `current_day.panes`) migrates cleanly.
    let mut legacy = AppData::default();
    legacy
        .current_day
        .panes
        .push(MainTaskPane::new("مهمة قديمة محفوظة"));
    let json = serde_json::to_string(&legacy).expect("serialize legacy");
    let mut loaded: AppData = serde_json::from_str(&json).expect("deserialize legacy");
    assert!(loaded.profiles.is_empty());

    loaded.ensure_profiles_for_date("2026-11-21", 1_795_200_000, "السبت — مهام اليوم".into());
    assert_eq!(loaded.profiles.len(), 1);
    assert_eq!(loaded.profiles[0].date_str, "2026-11-21");
    assert_eq!(loaded.profiles[0].panes.len(), 1);
    assert_eq!(loaded.profiles[0].panes[0].title, "مهمة قديمة محفوظة");

    // Add mixed Active / Completed / Cancelled tasks to yesterday's profile.
    let mut p_mixed = MainTaskPane::new("لوحة مختلطة");
    let mut active_task = TaskNode::new("مهمة نشطة");
    active_task.status = TaskStatus::Active;
    let mut done_task = TaskNode::new("مهمة منجزة");
    done_task.status = TaskStatus::Completed;
    let mut cancel_task = TaskNode::new("مهمة ملغاة");
    cancel_task.status = TaskStatus::Cancelled;
    p_mixed.subtasks = vec![active_task, done_task, cancel_task];

    let mut p_all_done = MainTaskPane::new("لوحة منتهية");
    let mut d = TaskNode::new("منتهي");
    d.status = TaskStatus::Completed;
    p_all_done.subtasks.push(d);

    loaded.profiles[0].panes = vec![p_mixed, p_all_done];

    // 2. Startup on the next day (today) prepends a fresh empty today profile at index 0
    //    without duplicating yesterday's tasks automatically.
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    loaded.ensure_profiles_for_date(&today, 1_795_286_400, "الأحد — مهام اليوم".into());
    assert_eq!(loaded.profiles.len(), 2);
    assert_eq!(loaded.profiles[0].date_str, today);
    assert_eq!(loaded.profiles[1].date_str, "2026-11-21");
    assert_eq!(loaded.active_profile_idx, 0);
    assert!(
        loaded.current_day.panes.is_empty(),
        "new day profile must start clean without unconsented task duplication"
    );

    // 3. One-click carry-over copies ONLY active tasks/panes and pushes an Undo snapshot.
    loaded.has_seen_onboarding = true;
    let mut app = WhiteboardApp::from_data(loaded);
    app.show_quick_entry_modal = false;
    let carried = app.carry_over_active_from_previous(None);
    assert_eq!(carried, 1, "only the pane with active tasks should be carried over");
    assert_eq!(app.data.current_day.panes.len(), 1);
    assert_eq!(app.data.current_day.panes[0].title, "لوحة مختلطة");
    assert_eq!(app.data.current_day.panes[0].subtasks.len(), 1);
    assert_eq!(app.data.current_day.panes[0].subtasks[0].title, "مهمة نشطة");
    assert_eq!(app.undo_stack.len(), 1);

    // Undo reverts the carry-over cleanly.
    assert!(app.undo(None));
    assert!(app.data.current_day.panes.is_empty());

    // Delete guard: deleting older profile leaves 1 profile, and deleting the last profile is blocked.
    let prev_count = app.data.profiles.len();
    assert!(prev_count >= 2);
    while app.data.profiles.len() > 1 {
        let last = app.data.profiles.len() - 1;
        app.delete_profile_at(last, None);
    }
    assert_eq!(app.data.profiles.len(), 1);
    app.delete_profile_at(0, None);
    assert_eq!(
        app.data.profiles.len(),
        1,
        "must never delete the last remaining profile"
    );
}

#[test]
fn first_run_quick_guide_displays_dismisses_and_reopens_from_status_bar() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    // Fresh state with language already chosen has `has_seen_onboarding == false` -> Quick Guide opens automatically.
    let mut fresh = AppData::default();
    fresh.language = Some(AppLang::Ar);
    fresh.current_day.panes.push(MainTaskPane::new("لوحة أولى"));
    assert!(!fresh.has_seen_onboarding);

    let mut app = WhiteboardApp::from_data(fresh);
    assert!(app.show_onboarding_modal);

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_guide = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let guide_text = rendered_text(&out_guide);
    assert!(
        guide_text.contains(&shape_arabic("دليل الاستخدام السريع")),
        "expected Quick Guide title on first run"
    );
    assert!(
        guide_text.contains(&shape_arabic("ابدأ العمل")),
        "expected high-contrast CTA button on Quick Guide"
    );

    // Locate and click `ابدأ العمل` button -> sets `has_seen_onboarding = true` and hides guide.
    let cta_pos = out_guide
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == shape_arabic("ابدأ العمل") => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("CTA button text must be painted");

    let click_at = |pos: Pos2, app: &mut WhiteboardApp| {
        let mut down = raw.clone();
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = raw.clone();
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(up, |ctx| app.draw(ctx))
    };

    let _ = click_at(cta_pos, &mut app);
    assert!(app.data.has_seen_onboarding);
    assert!(!app.show_onboarding_modal);
    let out_after = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    assert!(!rendered_text(&out_after).contains(&shape_arabic("دليل الاستخدام السريع")));

    // Locate and click subtle `?` button in bottom status bar -> reopens Quick Guide.
    let help_pos = out_after
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "?" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("? help button must be present in bottom status bar");
    let _ = click_at(help_pos, &mut app);
    assert!(app.show_onboarding_modal, "clicking ? must reopen the Quick Guide");
}

#[test]
fn first_launch_language_selector_persists_choice_and_top_bar_toggles_bilingual_ui() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    // 1. Fresh state has `language == None` -> centered 460px Swiss language modal appears first.
    let mut fresh = AppData::default();
    let mut pane = MainTaskPane::new("Daily Plan");
    pane.subtasks.push(TaskNode::new("Write specification"));
    fresh.current_day.panes.push(pane);
    assert!(fresh.language.is_none());

    let mut app = WhiteboardApp::from_data(fresh);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 660.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_lang = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let lang_text = rendered_text(&out_lang);
    assert!(
        lang_text.contains(&shape_arabic("اختر لغة الواجهة")),
        "language modal missing Arabic title; got {lang_text:?}"
    );
    assert!(
        lang_text.contains("SELECT INTERFACE LANGUAGE"),
        "language modal missing English title; got {lang_text:?}"
    );
    assert!(
        lang_text.contains(&shape_arabic("العربية")),
        "language modal missing العربية button"
    );
    assert!(
        lang_text.contains("English"),
        "language modal missing English button"
    );
    let has_lang_fill = out_lang.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            r.fill == theme::PANE && (r.rect.width() - 460.0).abs() < 1.0
        }
        _ => false,
    });
    let has_lang_border = out_lang.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            (r.rect.width() - 460.0).abs() < 1.0
                && r.stroke.color == theme::INK
                && (r.stroke.width - 1.5).abs() < 0.05
        }
        _ => false,
    });
    let has_lang_shadow = out_lang.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => {
            r.fill == theme::INK && (r.rect.width() - 460.0).abs() < 1.0
        }
        _ => false,
    });
    assert!(
        has_lang_fill && has_lang_border && has_lang_shadow,
        "expected centered 460px Swiss 0.0px language selector modal card with 1.5px border and 4px shadow"
    );

    let click_at = |pos: Pos2, app: &mut WhiteboardApp| {
        let mut down = raw.clone();
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = raw.clone();
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(up, |ctx| app.draw(ctx))
    };

    // 2. Click `English` -> persists `Some(AppLang::En)` and proceeds cleanly to English Quick Guide.
    let en_pos = out_lang
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "English" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("English choice button must be painted");
    let _ = click_at(en_pos, &mut app);
    assert_eq!(app.data.language, Some(AppLang::En));

    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_guide = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let guide_text = rendered_text(&out_guide);
    assert!(guide_text.contains("QUICK START GUIDE"));
    assert!(guide_text.contains("START WORKING"));

    // Dismiss English Quick Guide.
    let start_pos = out_guide
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "START WORKING" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("START WORKING CTA must be painted");
    let _ = click_at(start_pos, &mut app);
    app.dirty = false;
    let out_en = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let en_text = rendered_text(&out_en);
    for expected in [
        "HOME",
        "INDEX",
        "HARBOR",
        "DONE",
        "ACTIVE",
        "CANCELLED",
        "TOTAL",
        "AUTO-SAVED",
        "+ Add new branch...",
        "AR",
        "EN",
    ] {
        assert!(
            en_text.contains(expected),
            "English workspace missing {expected:?}; got {en_text:?}"
        );
    }

    // 3. Single-click top-bar `AR | EN` toggle -> switches live to Arabic RTL with zero StartDrag conflict.
    let toggle_pos = out_en
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "AR" && t.pos.y < 36.0 => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("top-bar AR | EN toggle must be painted");
    let out_after_toggle = click_at(toggle_pos, &mut app);
    assert_eq!(app.data.language, Some(AppLang::Ar));
    assert!(
        !out_after_toggle
            .viewport_output
            .values()
            .flat_map(|v| &v.commands)
            .any(|c| matches!(c, egui::ViewportCommand::StartDrag)),
        "clicking AR | EN toggle must NEVER trigger ViewportCommand::StartDrag"
    );

    app.dirty = false;
    let out_ar = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let ar_text = rendered_text(&out_ar);
    for expected in [
        "الرئيسية",
        "الفهرس",
        "الأرشيف",
        "مكتمل",
        "نشط",
        "ملغى",
        "المجموع",
        "محفوظ تلقائياً",
        "+ اكتب فرعاً جديداً...",
    ] {
        assert!(
            ar_text.contains(&shape_arabic(expected)),
            "Arabic workspace missing {expected:?}; got {ar_text:?}"
        );
    }

    // 4. Profiles modal renders localized strings in both Arabic and English.
    app.show_profiles_modal = true;
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let prof_ar = rendered_text(&ctx.run(raw.clone(), |ctx| app.draw(ctx)));
    assert!(prof_ar.contains(&shape_arabic("+ بروفايل جديد")));
    assert!(prof_ar.contains(&shape_arabic("ترحيل النشط من الأمس")));

    app.set_language(AppLang::En);
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let prof_en = rendered_text(&ctx.run(raw.clone(), |ctx| app.draw(ctx)));
    assert!(prof_en.contains("+ NEW PROFILE"));
    assert!(prof_en.contains("CARRY OVER ACTIVE"));
}

#[test]
fn bilingual_directional_mirroring_flips_tree_badges_fold_handles_dog_ear_and_notes() {
    // 1. Page grid mirrors pane 0 from right (RTL) to left (LTR).
    let rtl_grid = compute_page_grid_layout_dir(&[0, 1], 1000.0, 600.0, true);
    let ltr_grid = compute_page_grid_layout_dir(&[0, 1], 1000.0, 600.0, false);
    assert!(rtl_grid[0].pos.x > rtl_grid[1].pos.x, "RTL must place pane 0 on right");
    assert!(ltr_grid[0].pos.x < ltr_grid[1].pos.x, "LTR must place pane 0 on left");

    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::Ar);
    data.has_seen_onboarding = true;
    let mut pane = MainTaskPane::new("Alpha Project");
    let mut parent = TaskNode::new("ParentRoot");
    parent.status = TaskStatus::Completed;
    parent.notes = "Line one\nLine two".to_string();
    parent.children.push(TaskNode::new("ChildLeaf"));
    let mut done_leaf = TaskNode::new("DoneLeaf");
    done_leaf.status = TaskStatus::Completed;
    parent.children.push(done_leaf);
    pane.subtasks.push(parent);
    data.current_day.panes.push(pane);

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0)));

    let find_text_x = |out: &egui::FullOutput, needle: &str| -> f32 {
        out.shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == needle => Some(t.pos.x),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing text {needle:?}"))
    };

    // 2. Arabic RTL: child extends to LEFT of parent; `01` badge on RIGHT of parent text;
    //    dog-ear at top-left; fold handle on LEFT border; emerald hairline on RIGHT.
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_ar = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let ar_parent_x = find_text_x(&out_ar, "ParentRoot");
    let ar_child_x = find_text_x(&out_ar, "ChildLeaf");
    let ar_badge_x = find_text_x(&out_ar, "01");
    assert!(
        ar_child_x < ar_parent_x,
        "RTL child ({ar_child_x}) must sit to the LEFT of parent ({ar_parent_x})"
    );
    assert!(
        ar_badge_x > ar_parent_x,
        "RTL root `01` badge ({ar_badge_x}) must sit to the RIGHT of parent text ({ar_parent_x})"
    );
    let has_tl_dog_ear = out_ar.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Path(p) => {
            p.closed
                && p.fill == theme::INK
                && p.points.len() == 3
                && (p.points[1].x - p.points[0].x - 8.0).abs() < 0.05
                && (p.points[2].y - p.points[0].y - 8.0).abs() < 0.05
        }
        _ => false,
    });
    assert!(has_tl_dog_ear, "RTL dog-ear triangle must sit at top-left corner");

    // 3. Switch to English LTR: child extends to RIGHT of parent; `01` badge on LEFT of parent text;
    //    dog-ear at top-right; fold handle on RIGHT border; emerald hairline on LEFT.
    app.set_language(AppLang::En);
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_en = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let en_parent_x = find_text_x(&out_en, "ParentRoot");
    let en_child_x = find_text_x(&out_en, "ChildLeaf");
    let en_badge_x = find_text_x(&out_en, "01");
    assert!(
        en_child_x > en_parent_x,
        "LTR child ({en_child_x}) must sit to the RIGHT of parent ({en_parent_x})"
    );
    assert!(
        en_badge_x < en_parent_x,
        "LTR root `01` badge ({en_badge_x}) must sit to the LEFT of parent text ({en_parent_x})"
    );
    let has_tr_dog_ear = out_en.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Path(p) => {
            p.closed
                && p.fill == theme::INK
                && p.points.len() == 3
                && (p.points[0].x - p.points[1].x - 8.0).abs() < 0.05
                && (p.points[2].y - p.points[0].y - 8.0).abs() < 0.05
        }
        _ => false,
    });
    assert!(has_tr_dog_ear, "LTR dog-ear triangle must sit at top-right corner");

    // Verify emerald progress hairline fills left-to-right in English (`fill.left <= track.left`).
    let mut tracks = Vec::new();
    let mut fills = Vec::new();
    for clipped in &out_en.shapes {
        if let egui::Shape::Rect(r) = &clipped.shape {
            if (r.rect.height() - 2.0).abs() < 0.05 {
                if r.fill == theme::HINT {
                    tracks.push(r.rect);
                } else if r.fill == theme::GREEN_SHADOW {
                    fills.push(r.rect);
                }
            }
        }
    }
    assert!(
        tracks.iter().any(|t| fills.iter().any(|f| {
            (t.top() - f.top()).abs() < 0.1
                && (f.right() - t.left()).abs() < 0.5
                && f.left() < t.left()
        })),
        "LTR header progress hairline must fill left-to-right in emerald"
    );

    // 4. Open note drawer in English LTR -> NOTE & COPY header and left-aligned multiline lines.
    let parent_id = app.data.current_day.panes[0].subtasks[0].id.clone();
    app.open_note_drawer(parent_id, false);
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_note = ctx.run(raw, |ctx| app.draw(ctx));
    let note_text = rendered_text(&out_note);
    assert!(note_text.contains("NOTE"), "English note drawer missing NOTE tag");
    assert!(note_text.contains("COPY"), "English note drawer missing COPY button");
    let paper_rect = out_note
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Rect(r) if r.fill == theme::DRAWER_BG && r.rect.width() > 400.0 => {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("expected #F8FAFC note card in English LTR");
    let ltr_galley = out_note
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.rows.len() >= 2 => Some(t),
            _ => None,
        })
        .expect("expected multiline LTR note galley");
    let expected_left = paper_rect.left() + 1.5 + 16.0;
    assert!(
        (ltr_galley.pos.x - expected_left).abs() <= 4.0,
        "LTR note text left edge ({:.1}) must align with left padding ({expected_left:.1})",
        ltr_galley.pos.x
    );
}

#[test]
fn top_bar_page_label_click_and_ctrl_arrows_flip_pages_with_zero_drag_conflict() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    for i in 1..=5 {
        data.current_day
            .panes
            .push(MainTaskPane::new(&format!("لوحة {}", i)));
    }
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    assert_eq!(app.current_page, 0);

    // 1. Clicking directly on `PAGE 1 / 2` advances to Page 2 on the very first click
    //    (even with a 2px pointer jitter between press and release — no StartDrag conflict!).
    let page_pos = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "PAGE 1 / 2" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("PAGE 1 / 2 label must be rendered");

    let mut press = raw.clone();
    press.events.push(egui::Event::PointerMoved(page_pos));
    press.events.push(egui::Event::PointerButton {
        pos: page_pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(press, |ctx| app.draw(ctx));

    let jittered = page_pos + vec2(2.0, 1.0);
    let mut release = raw.clone();
    release.events.push(egui::Event::PointerMoved(jittered));
    release.events.push(egui::Event::PointerButton {
        pos: jittered,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    let out_after_click = ctx.run(release, |ctx| app.draw(ctx));
    assert_eq!(
        app.current_page, 1,
        "clicking PAGE 1 / 2 with slight pointer jitter must flip to page 2 immediately"
    );
    assert!(
        !out_after_click
            .viewport_output
            .values()
            .flat_map(|v| &v.commands)
            .any(|c| matches!(c, egui::ViewportCommand::StartDrag)),
        "clicking PAGE 1 / 2 must NEVER trigger ViewportCommand::StartDrag"
    );

    // 2. Ctrl+Left flips back to Page 1, Ctrl+Right flips forward to Page 2.
    let mut ctrl_left = raw.clone();
    ctrl_left.modifiers = egui::Modifiers::CTRL;
    ctrl_left.events.push(egui::Event::Key {
        key: Key::ArrowLeft,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let _ = ctx.run(ctrl_left, |ctx| app.draw(ctx));
    assert_eq!(app.current_page, 0, "Ctrl+Left must flip back to page 1");

    let mut ctrl_right = raw.clone();
    ctrl_right.modifiers = egui::Modifiers::CTRL;
    ctrl_right.events.push(egui::Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let _ = ctx.run(ctrl_right, |ctx| app.draw(ctx));
    assert_eq!(app.current_page, 1, "Ctrl+Right must flip forward to page 2");
}

#[test]
fn creating_new_profile_starts_blank_keeps_modal_open_and_focuses_title() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut app = WhiteboardApp::from_data(sample_data());
    app.show_quick_entry_modal = false;
    app.show_profiles_modal = true;
    assert_eq!(app.data.current_day.panes.len(), 2);

    let mut raw = egui::RawInput::default();
    raw.focused = true;
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_modal = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    // 1. Click `+ بروفايل جديد` inside the modal.
    let new_btn_pos = out_modal
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == shape_arabic("+ بروفايل جديد") => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("+ بروفايل جديد button must be rendered inside Profiles modal");

    let click_at = |pos: Pos2, app: &mut WhiteboardApp| {
        let mut down = raw.clone();
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = raw.clone();
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(up, |ctx| app.draw(ctx))
    };

    let _ = click_at(new_btn_pos, &mut app);
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    // Modal MUST stay open so user can immediately name the new profile.
    assert!(
        app.show_profiles_modal,
        "clicking + بروفايل جديد must keep the Profiles modal open"
    );
    // New profile at index 0 MUST start completely blank (never cloning old profile tasks).
    assert_eq!(app.data.profiles.len(), 2);
    assert_eq!(app.data.active_profile_idx, 0);
    assert!(
        app.data.current_day.panes.is_empty(),
        "new profile must start completely blank"
    );
    assert!(
        app.data.profiles[0].panes.is_empty(),
        "new profile's stored panes must start completely blank"
    );
    assert_eq!(
        app.data.profiles[1].panes.len(),
        2,
        "previous profile must preserve its original 2 panes untouched"
    );

    // New profile's inline title TextEdit MUST automatically receive focus.
    let new_id = app.data.profiles[0].id.clone();
    let edit_id = Id::new(("profile_title_edit", new_id.as_str()));
    assert!(
        ctx.memory(|m| m.has_focus(edit_id)),
        "newly created profile's inline title TextEdit must automatically grab keyboard focus"
    );

    // 2. Clicking inside the modal card must NOT dismiss it.
    let _ = click_at(pos2(500.0, 320.0), &mut app);
    assert!(
        app.show_profiles_modal,
        "clicking inside modal_rect must NOT dismiss the Profiles modal"
    );

    // 3. Clicking strictly outside `modal_rect` closes the modal cleanly.
    let _ = click_at(pos2(10.0, 10.0), &mut app);
    assert!(
        !app.show_profiles_modal,
        "clicking outside modal_rect must dismiss the Profiles modal"
    );

    // 4. Deleting the active blank profile must NOT overwrite the surviving profile's tasks!
    app.delete_profile_at(0, Some(&ctx));
    assert_eq!(app.data.profiles.len(), 1);
    assert_eq!(
        app.data.current_day.panes.len(),
        2,
        "deleting the active profile must restore the surviving profile's original panes intact"
    );
}

#[test]
fn quick_entry_stages_on_enter_removes_items_and_creates_only_on_submit_or_ctrl_enter() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;

    let mut app = WhiteboardApp::from_data(data);
    assert!(app.show_quick_entry_modal, "empty board opens Quick Task Entry modal");
    assert!(app.quick_entry_list.is_empty());

    let mut raw = egui::RawInput::default();
    raw.focused = true;
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_empty = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    let click_at = |pos: Pos2, app: &mut WhiteboardApp| {
        let mut down = raw.clone();
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = raw.clone();
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(up, |ctx| app.draw(ctx))
    };

    // 1. When both `quick_entry_list` and input are empty, `CREATE BOARD` is disabled/dimmed
    //    and clicking it does NOT close the modal or create panes.
    let create_pos_empty = out_empty
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "CREATE BOARD" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("CREATE BOARD button must be rendered");
    let _ = click_at(create_pos_empty, &mut app);
    assert!(app.show_quick_entry_modal, "disabled CREATE BOARD must not close modal");
    assert!(app.data.current_day.panes.is_empty());

    // 2. Typing `/`-separated tasks and pressing Enter (without Ctrl) stages them into
    //    `quick_entry_list`, clears input, keeps modal open, and retains input focus.
    let input_id = Id::new("quick_entry_input");
    ctx.memory_mut(|m| m.request_focus(input_id));
    app.quick_entry_input = "Task Alpha / Task Beta".to_string();
    let mut press_enter = raw.clone();
    press_enter.events.push(egui::Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(press_enter, |ctx| app.draw(ctx));
    let out_staged = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    assert!(
        app.show_quick_entry_modal,
        "pressing Enter must stage tasks and keep the modal open"
    );
    assert!(
        app.data.current_day.panes.is_empty(),
        "pressing Enter must NOT create panes immediately"
    );
    assert_eq!(app.quick_entry_list, vec!["Task Alpha", "Task Beta"]);
    assert!(app.quick_entry_input.is_empty(), "input must clear after staging");
    assert!(
        ctx.memory(|m| m.has_focus(input_id)),
        "keyboard focus must stay on quick_entry_input after Enter"
    );
    let staged_text = rendered_text(&out_staged);
    assert!(staged_text.contains("[01]") && staged_text.contains("Task Alpha"));
    assert!(staged_text.contains("[02]") && staged_text.contains("Task Beta"));

    // 3. Clicking the `×` remove button on the first staged row removes "Task Alpha"
    //    while keeping the modal open.
    let rm_first_pos = out_staged
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "\u{d7}" && t.pos.y > 40.0 => {
                Some((t.pos.y, t.pos + t.galley.size() * 0.5))
            }
            _ => None,
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
        .expect("staged row remove button × must be rendered");
    let _ = click_at(rm_first_pos, &mut app);
    assert!(app.show_quick_entry_modal);
    assert_eq!(app.quick_entry_list, vec!["Task Beta"]);

    // 4. Typing a trailing task and pressing Ctrl+Enter creates panes for both
    //    staged items and remaining input, clears state, and closes modal.
    app.quick_entry_input = "Task Gamma".to_string();
    let mut ctrl_enter = raw.clone();
    ctrl_enter.modifiers = egui::Modifiers::CTRL;
    ctrl_enter.events.push(egui::Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let _ = ctx.run(ctrl_enter, |ctx| app.draw(ctx));
    assert!(!app.show_quick_entry_modal, "Ctrl+Enter must close the modal");
    assert!(app.quick_entry_list.is_empty());
    assert!(app.quick_entry_input.is_empty());
    let titles: Vec<&str> = app
        .data
        .current_day
        .panes
        .iter()
        .map(|p| p.title.as_str())
        .collect();
    assert_eq!(titles, vec!["Task Beta", "Task Gamma"]);

    // 5. Reopen modal, stage "Task Delta", and click CREATE BOARD -> appends pane & closes.
    app.show_quick_entry_modal = true;
    app.quick_entry_list.push("Task Delta".to_string());
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_ready = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let create_pos = out_ready
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "CREATE BOARD" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("enabled CREATE BOARD button must be rendered");
    let _ = click_at(create_pos, &mut app);
    assert!(!app.show_quick_entry_modal, "clicking CREATE BOARD must close the modal");
    assert!(app.quick_entry_list.is_empty());
    let titles: Vec<&str> = app
        .data
        .current_day
        .panes
        .iter()
        .map(|p| p.title.as_str())
        .collect();
    assert_eq!(titles, vec!["Task Beta", "Task Gamma", "Task Delta"]);
}

#[test]
fn cross_profile_undo_clears_stack_on_profile_load_and_never_overwrites_other_profile() {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    data.profiles = vec![
        DayProfile {
            id: "p-today".to_string(),
            date_str: today,
            title: "Today".to_string(),
            created_ts: 200,
            panes: vec![MainTaskPane::new("Today Original")],
        },
        DayProfile {
            id: "p-yesterday".to_string(),
            date_str: "2026-11-21".to_string(),
            title: "Yesterday".to_string(),
            created_ts: 100,
            panes: vec![MainTaskPane::new("Yesterday Pane")],
        },
    ];
    data.active_profile_idx = 0;

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    // Mutate Today so undo_stack has snapshots belonging to Today.
    app.add_main_pane("Today Extra");
    assert_eq!(app.undo_stack.len(), 1);
    assert_eq!(app.data.current_day.panes.len(), 2);

    // Switch to Yesterday -> undo_stack MUST be cleared.
    app.switch_to_profile(1, None);
    assert!(
        app.undo_stack.is_empty(),
        "switching profiles must clear undo_stack"
    );
    assert!(
        !app.undo(None),
        "Ctrl+Z immediately after switching profiles must be a no-op"
    );
    let yesterday_titles: Vec<&str> = app
        .data
        .current_day
        .panes
        .iter()
        .map(|p| p.title.as_str())
        .collect();
    assert_eq!(
        yesterday_titles,
        vec!["Yesterday Pane"],
        "undo must never overwrite Yesterday's panes with Today's snapshot"
    );

    // Mutate Yesterday, then create a new profile -> undo_stack MUST clear again.
    app.add_main_pane("Yesterday Added");
    assert_eq!(app.undo_stack.len(), 1);
    app.create_new_profile(None);
    assert!(
        app.undo_stack.is_empty(),
        "creating a new profile must clear undo_stack"
    );
    assert!(!app.undo(None));
    assert!(
        app.data.current_day.panes.is_empty(),
        "new blank profile must remain blank after undo attempt"
    );

    // Mutate the new profile, then delete it (loads surviving profile) -> undo_stack MUST clear.
    app.add_main_pane("Temp Pane");
    assert_eq!(app.undo_stack.len(), 1);
    app.delete_profile_at(0, None);
    assert!(
        app.undo_stack.is_empty(),
        "deleting active profile and loading another must clear undo_stack"
    );
    assert!(!app.undo(None));
}

#[test]
fn carry_over_only_pulls_from_chronologically_previous_profile_and_skips_duplicate_titles() {
    let mut day1_pane = MainTaskPane::new("Day 1 Task");
    day1_pane.subtasks.push(TaskNode::new("Sub 1"));

    let mut day2_pane_shared = MainTaskPane::new("Shared Title");
    day2_pane_shared.subtasks.push(TaskNode::new("Old Sub"));

    let mut day2_pane_unique = MainTaskPane::new("Day 2 Unique");
    day2_pane_unique.subtasks.push(TaskNode::new("Active Sub"));
    let mut done_sub = TaskNode::new("Done Sub");
    done_sub.status = TaskStatus::Completed;
    day2_pane_unique.subtasks.push(done_sub);

    let day3_existing = MainTaskPane::new("Shared Title");

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    // Sorted newest-first: index 0 = Day 3 (newest), index 1 = Day 2, index 2 = Day 1 (oldest).
    data.profiles = vec![
        DayProfile {
            id: "p-day3".to_string(),
            date_str: today,
            title: "Day 3".to_string(),
            created_ts: 300,
            panes: vec![day3_existing],
        },
        DayProfile {
            id: "p-day2".to_string(),
            date_str: "2026-11-21".to_string(),
            title: "Day 2".to_string(),
            created_ts: 200,
            panes: vec![day2_pane_shared, day2_pane_unique],
        },
        DayProfile {
            id: "p-day1".to_string(),
            date_str: "2026-11-20".to_string(),
            title: "Day 1".to_string(),
            created_ts: 100,
            panes: vec![day1_pane],
        },
    ];
    data.active_profile_idx = 0;

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    // 1. On Day 3 (newest): carries from Day 2, skipping "Shared Title" which already exists in Day 3.
    let carried_first = app.carry_over_active_from_previous(None);
    assert_eq!(
        carried_first, 1,
        "should carry only 'Day 2 Unique' and skip existing 'Shared Title'"
    );
    let day3_titles: Vec<&str> = app
        .data
        .current_day
        .panes
        .iter()
        .map(|p| p.title.as_str())
        .collect();
    assert_eq!(day3_titles, vec!["Shared Title", "Day 2 Unique"]);

    // 2. Repeating carry-over on Day 3 must NOT duplicate any panes.
    let undo_len_before = app.undo_stack.len();
    let carried_repeat = app.carry_over_active_from_previous(None);
    assert_eq!(
        carried_repeat, 0,
        "repeated carry-over clicks must skip already-present titles and return 0"
    );
    assert_eq!(app.data.current_day.panes.len(), 2);
    assert_eq!(app.undo_stack.len(), undo_len_before);

    // 3. On Day 2 (middle): pulls ONLY from Day 1 (older), never from Day 3 (newer).
    app.switch_to_profile(1, None);
    let carried_day2 = app.carry_over_active_from_previous(None);
    assert_eq!(carried_day2, 1);
    let day2_titles: Vec<&str> = app
        .data
        .current_day
        .panes
        .iter()
        .map(|p| p.title.as_str())
        .collect();
    assert_eq!(
        day2_titles,
        vec!["Shared Title", "Day 2 Unique", "Day 1 Task"]
    );

    // 4. On Day 1 (oldest): MUST NOT pull from Day 2 or Day 3 (newer profiles).
    app.switch_to_profile(2, None);
    assert!(app.previous_profile_index().is_none());
    let carried_oldest = app.carry_over_active_from_previous(None);
    assert_eq!(
        carried_oldest, 0,
        "oldest profile must never carry over tasks from a newer profile"
    );
    let day1_titles: Vec<&str> = app
        .data
        .current_day
        .panes
        .iter()
        .map(|p| p.title.as_str())
        .collect();
    assert_eq!(day1_titles, vec!["Day 1 Task"]);
}

#[test]
fn auto_save_debounces_throttles_retries_on_failure_and_renders_bottom_bar_error() {
    let mut data = sample_data();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let t0 = Instant::now();
    app.dirty = true;
    app.last_dirty_at = Some(t0);

    // Before 400ms debounce elapses, save must NOT run.
    let mut attempts = 0usize;
    assert!(!app.auto_save_if_due_with(t0 + Duration::from_millis(200), None, |_| {
        attempts += 1;
        Ok(())
    }));
    assert_eq!(attempts, 0);
    assert!(app.dirty);

    // After 400ms debounce elapses, a failing save stores `save_error` and keeps `dirty = true`.
    let t_fail = t0 + Duration::from_millis(450);
    assert!(!app.auto_save_if_due_with(t_fail, None, |_| {
        attempts += 1;
        Err("disk full".to_string())
    }));
    assert_eq!(attempts, 1);
    assert!(app.dirty);
    assert_eq!(app.save_error.as_deref(), Some("disk full"));

    // Retries within 5 seconds MUST be skipped (do not retry every frame).
    assert!(!app.auto_save_if_due_with(t_fail + Duration::from_secs(2), None, |_| {
        attempts += 1;
        Ok(())
    }));
    assert_eq!(attempts, 1, "must not retry before 5-second cooldown");

    // Bottom bar renders visible error message instead of "SAVING...".
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 640.0)));
    let out = ctx.run(raw, |ctx| app.draw(ctx));
    let text = rendered_text(&out);
    assert!(
        text.contains("SAVE ERROR") && text.contains("disk full"),
        "bottom bar must show visible save error; got {text:?}"
    );
    assert!(
        !text.contains("SAVING..."),
        "bottom bar must not show SAVING... while save_error is set"
    );

    // After 5 seconds, retry runs and clears `save_error` and `dirty` on success.
    assert!(app.auto_save_if_due_with(t_fail + Duration::from_secs(5), None, |_| {
        attempts += 1;
        Ok(())
    }));
    assert_eq!(attempts, 2);
    assert!(!app.dirty);
    assert!(app.save_error.is_none());
}

#[test]
fn load_error_shows_blocking_modal_with_backup_path_and_blocks_saves() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let err = LoadError {
        file_path: std::path::PathBuf::from("/tmp/wb/data.json"),
        message: "Failed to parse /tmp/wb/data.json: expected value".to_string(),
        backup_path: Some(std::path::PathBuf::from(
            "/tmp/wb/data.json.20261122_180000.bak",
        )),
        backup_error: None,
    };
    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    let mut app = WhiteboardApp::from_data_with_load_error(data, Some(err));

    // Saves must be blocked while load_error is active.
    app.dirty = true;
    let mut saved = false;
    assert!(!app.try_save_with(Instant::now(), |_| {
        saved = true;
        Ok(())
    }));
    assert!(!saved, "must never overwrite data.json while load_error is unresolved");

    // Warmup frame so egui Area initializes, then send Escape (must NOT dismiss the modal).
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    let mut esc_raw = raw.clone();
    esc_raw.events.push(egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let out = ctx.run(esc_raw, |ctx| app.draw(ctx));
    assert!(app.load_error.is_some(), "Escape must not dismiss load error modal");

    let text = rendered_text(&out);
    assert!(text.contains("DATA LOAD ERROR"), "expected load error title; got {text:?}");
    assert!(
        text.contains("data.json.20261122_180000.bak"),
        "modal must show where the .bak copy is; got {text:?}"
    );
    assert!(text.contains("expected value"), "modal must explain what happened");

    // Clicking START FRESH explicitly dismisses the load error modal.
    let fresh_pos = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "START FRESH" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("START FRESH button must be rendered");
    let mut down = raw.clone();
    down.events.push(egui::Event::PointerMoved(fresh_pos));
    down.events.push(egui::Event::PointerButton {
        pos: fresh_pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(down, |ctx| app.draw(ctx));
    let mut up = raw.clone();
    up.events.push(egui::Event::PointerMoved(fresh_pos));
    up.events.push(egui::Event::PointerButton {
        pos: fresh_pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(up, |ctx| app.draw(ctx));
    assert!(
        app.load_error.is_none(),
        "clicking START FRESH must dismiss load_error"
    );
}

#[test]
fn drag_release_over_buttons_does_not_trigger_clicks() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = sample_data();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    let index_pos = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "INDEX" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("INDEX tab must be rendered");

    // Press on canvas whitespace, drag onto INDEX tab, and release -> must NOT switch to INDEX.
    let start = pos2(500.0, 300.0);
    let mut press = raw.clone();
    press.events.push(egui::Event::PointerMoved(start));
    press.events.push(egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(press, |ctx| app.draw(ctx));

    let mut move_over = raw.clone();
    move_over.events.push(egui::Event::PointerMoved(index_pos));
    let _ = ctx.run(move_over, |ctx| app.draw(ctx));

    let mut release = raw.clone();
    release.events.push(egui::Event::PointerMoved(index_pos));
    release.events.push(egui::Event::PointerButton {
        pos: index_pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(release, |ctx| app.draw(ctx));

    assert!(
        !app.show_master_index,
        "releasing a drag over the INDEX top-bar tab must NOT activate it"
    );
}

#[test]
fn ctrl_arrows_do_not_flip_pages_while_text_edit_has_focus() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    for i in 1..=5 {
        data.current_day
            .panes
            .push(MainTaskPane::new(&format!("Pane {}", i)));
    }
    let pane0_id = data.current_day.panes[0].id.clone();
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.focused = true;
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    // Focus the bottom branch TextEdit of pane 0.
    let input_id = Id::new(("pane_branch_input", pane0_id.as_str()));
    ctx.memory_mut(|m| m.request_focus(input_id));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    assert!(ctx.wants_keyboard_input());

    // Ctrl+Right while TextEdit is focused must NOT flip to page 2.
    let mut ctrl_right = raw.clone();
    ctrl_right.modifiers = egui::Modifiers::CTRL;
    ctrl_right.events.push(egui::Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let _ = ctx.run(ctrl_right, |ctx| app.draw(ctx));
    assert_eq!(
        app.current_page, 0,
        "Ctrl+Right must NOT flip pages while a TextEdit has keyboard focus"
    );

    // Surrender focus -> Ctrl+Right flips to page 2, Ctrl+Left flips back to page 1.
    ctx.memory_mut(|m| m.surrender_focus(input_id));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let mut ctrl_right2 = raw.clone();
    ctrl_right2.modifiers = egui::Modifiers::CTRL;
    ctrl_right2.events.push(egui::Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let _ = ctx.run(ctrl_right2, |ctx| app.draw(ctx));
    assert_eq!(app.current_page, 1);
}

#[test]
fn pane_header_single_click_toggles_focus_and_double_click_does_not_double_fire() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = sample_data();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    let pane0_id = data.current_day.panes[0].id.clone();
    let pane0_title = data.current_day.panes[0].title.clone();
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out = ctx.run(raw.clone(), |ctx| app.draw(ctx));

    let shaped_title = disp(&pane0_title);
    let header_pos = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == shaped_title => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("pane header title must be rendered");

    let click_at = |pos: Pos2, t: f64, app: &mut WhiteboardApp| {
        let mut down = raw.clone();
        down.time = Some(t);
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = raw.clone();
        up.time = Some(t + 0.04);
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(up, |ctx| app.draw(ctx));
    };

    // Double-clicking the pane header fires focus toggle ONCE on click 1 and does NOT
    // immediately toggle back off on click 2.
    click_at(header_pos, 1.0, &mut app);
    assert_eq!(app.focused_pane_id.as_deref(), Some(pane0_id.as_str()));
    click_at(header_pos, 1.10, &mut app);
    assert_eq!(
        app.focused_pane_id.as_deref(),
        Some(pane0_id.as_str()),
        "second click of a double-click must not re-fire focus toggle"
    );

    // A distinct single click (>0.5s later) toggles focus back off.
    click_at(header_pos, 2.0, &mut app);
    assert!(
        app.focused_pane_id.is_none(),
        "single-clicking focused pane header must toggle focus off"
    );
}

#[test]
fn quick_entry_top_bar_plus_ctrl_n_backdrop_close_duplicates_inline_edit_and_onboarding() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut raw = egui::RawInput::default();
    raw.focused = true;
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 640.0)));

    let click_at = |pos: Pos2, app: &mut WhiteboardApp| {
        let mut down = raw.clone();
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = raw.clone();
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(up, |ctx| app.draw(ctx))
    };

    // 1. Dismissing onboarding on an EMPTY board automatically opens Quick Task Entry.
    let mut empty_data = AppData::default();
    empty_data.language = Some(AppLang::En);
    empty_data.has_seen_onboarding = false;
    let mut onboarding_app = WhiteboardApp::from_data(empty_data);
    assert!(onboarding_app.show_onboarding_modal);
    assert!(!onboarding_app.show_quick_entry_modal);

    let _ = ctx.run(raw.clone(), |ctx| onboarding_app.draw(ctx));
    let out_guide = ctx.run(raw.clone(), |ctx| onboarding_app.draw(ctx));
    let cta_pos = out_guide
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "START WORKING" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("START WORKING button must be rendered");
    let _ = click_at(cta_pos, &mut onboarding_app);
    assert!(!onboarding_app.show_onboarding_modal);
    assert!(
        onboarding_app.show_quick_entry_modal,
        "dismissing onboarding on an empty board must automatically open Quick Task Entry"
    );

    // 2. On a board that ALREADY has panes, top-bar `+` opens Quick Task Entry,
    //    clicking the dimmed backdrop closes it, and `Ctrl+N` reopens it.
    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    data.current_day.panes.push(MainTaskPane::new("Existing Pane"));
    let mut app = WhiteboardApp::from_data(data);
    assert!(!app.show_quick_entry_modal);

    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_board = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let plus_pos = out_board
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "+" && t.pos.y < 36.0 => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("top-bar + button must be rendered");
    let _ = click_at(plus_pos, &mut app);
    assert!(
        app.show_quick_entry_modal,
        "clicking top-bar + button must open Quick Task Entry even when board has panes"
    );

    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    // Clicking dimmed backdrop outside modal card closes it.
    let _ = click_at(pos2(10.0, 10.0), &mut app);
    assert!(
        !app.show_quick_entry_modal,
        "clicking outside Quick Task Entry modal card must close it"
    );

    // Ctrl+N reopens Quick Task Entry even when board already has panes.
    let mut ctrl_n = raw.clone();
    ctrl_n.modifiers = egui::Modifiers::CTRL;
    ctrl_n.events.push(egui::Event::Key {
        key: Key::N,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let _ = ctx.run(ctrl_n, |ctx| app.draw(ctx));
    assert!(
        app.show_quick_entry_modal,
        "Ctrl+N must open Quick Task Entry even when board already has panes"
    );

    // 3. Duplicate rejection against existing panes and staged items shows inline message.
    app.quick_entry_input = "Existing Pane".to_string();
    assert_eq!(app.stage_quick_entry_input(), 0);
    assert!(app.quick_entry_list.is_empty());
    assert!(
        app.quick_entry_error
            .as_deref()
            .is_some_and(|e| e.contains("Duplicate title") && e.contains("Existing Pane")),
        "staging an existing pane title must be rejected with inline message"
    );

    app.quick_entry_input = "Alpha / Beta / alpha".to_string();
    assert_eq!(app.stage_quick_entry_input(), 2);
    assert_eq!(app.quick_entry_list, vec!["Alpha", "Beta"]);
    assert!(
        app.quick_entry_error
            .as_deref()
            .is_some_and(|e| e.contains("alpha")),
        "duplicate within staged batch must be rejected with inline message"
    );

    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_staged = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    assert!(
        rendered_text(&out_staged).contains("Duplicate title:"),
        "inline duplicate message must render inside Quick Task Entry modal"
    );

    // 4. Clicking a staged row enters inline edit; Esc cancels without closing modal;
    //    editing + Enter saves (and rejects duplicates).
    let alpha_pos = out_staged
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "Alpha" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("staged row Alpha must be rendered");
    let _ = click_at(alpha_pos, &mut app);
    assert_eq!(app.quick_entry_editing_idx, Some(0));

    // Mutate text then press Esc -> restores "Alpha" and keeps modal open.
    app.quick_entry_list[0] = "Discarded Edit".to_string();
    let mut press_esc = raw.clone();
    press_esc.events.push(egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(press_esc, |ctx| app.draw(ctx));
    assert!(
        app.show_quick_entry_modal,
        "Esc during staged-row inline edit must keep modal open"
    );
    assert_eq!(app.quick_entry_editing_idx, None);
    assert_eq!(
        app.quick_entry_list[0], "Alpha",
        "Esc must restore the original staged row title"
    );

    // Click Alpha again (re-locate after error banner cleared), edit to duplicate "Beta",
    // press Enter -> rejected with inline error.
    let out_after_esc = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let alpha_pos_2 = out_after_esc
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "Alpha" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("staged row Alpha must still be rendered after Esc");
    let _ = click_at(alpha_pos_2, &mut app);
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    assert_eq!(app.quick_entry_editing_idx, Some(0));
    app.quick_entry_list[0] = "Beta".to_string();
    let mut press_enter = raw.clone();
    press_enter.events.push(egui::Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(press_enter.clone(), |ctx| app.draw(ctx));
    assert_eq!(
        app.quick_entry_editing_idx,
        Some(0),
        "duplicate inline edit must stay in edit mode with error"
    );
    assert!(app.quick_entry_error.is_some());

    // Change to unique "Alpha Updated" and press Enter -> saves cleanly.
    app.quick_entry_list[0] = "Alpha Updated".to_string();
    let _ = ctx.run(press_enter, |ctx| app.draw(ctx));
    assert_eq!(app.quick_entry_editing_idx, None);
    assert_eq!(app.quick_entry_list[0], "Alpha Updated");
    assert!(app.quick_entry_error.is_none());
}

#[test]
fn import_confirmation_backs_up_current_data_and_replaces_state() {
    let temp_dir = std::env::temp_dir().join(format!(
        "wb_import_confirm_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let mut current = AppData::default();
    current.language = Some(AppLang::En);
    current.has_seen_onboarding = true;
    current.current_day.panes.push(MainTaskPane::new("Old Current Pane"));
    let mut app = WhiteboardApp::from_data(current);
    app.show_quick_entry_modal = false;
    app.push_undo_snapshot();
    assert_eq!(app.undo_stack.len(), 1);

    let mut incoming = AppData::default();
    incoming.language = Some(AppLang::En);
    incoming.has_seen_onboarding = true;
    incoming.profiles = vec![DayProfile {
        id: "imported-day".to_string(),
        date_str: "2026-11-20".to_string(),
        title: "Imported Friday".to_string(),
        created_ts: 1_795_100_000,
        panes: vec![MainTaskPane::new("Imported Pane")],
    }];
    incoming.active_profile_idx = 0;

    app.show_data_io_modal = true;
    app.pending_import = Some(PendingImport {
        source_path: temp_dir.join("incoming.json"),
        data: incoming,
    });

    // Confirm import -> backs up current data to temp_dir first, then replaces state and clears undo_stack.
    let backup_dir = temp_dir.clone();
    let save_dir = temp_dir.clone();
    let bak_path = app
        .confirm_pending_import_with(
            |cur| crate::storage::backup_app_data_to_dir(cur, &backup_dir),
            |new_data| crate::storage::save_app_data_to_dir(new_data, &save_dir),
            None,
        )
        .expect("import confirmation should succeed");

    assert!(bak_path.exists(), "automatic .bak backup must exist on disk");
    let backed_up = crate::storage::import_app_data_from_path(&bak_path)
        .expect("backup file must be valid JSON");
    assert_eq!(
        backed_up.current_day.panes[0].title, "Old Current Pane",
        "automatic backup must preserve the previous data before replacing"
    );

    assert!(!app.show_data_io_modal);
    assert!(app.pending_import.is_none());
    assert!(app.undo_stack.is_empty(), "import must clear undo stack");
    assert_eq!(app.data.current_day.panes.len(), 1);
    assert_eq!(app.data.current_day.panes[0].title, "Imported Pane");
    assert!(app.undo_toast.is_some());

    let saved_on_disk = crate::storage::load_app_data_from_path(&temp_dir.join("data.json"))
        .expect("imported data must be saved to data.json");
    assert_eq!(saved_on_disk.current_day.panes[0].title, "Imported Pane");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn harbor_search_filters_and_two_click_permanent_delete_removes_archived_pane() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;

    let mut pane_a = MainTaskPane::new("Quarterly Budget");
    let mut sub_a = TaskNode::new("Audit invoices");
    sub_a.notes = "Check Q4 receipts".to_string();
    pane_a.subtasks.push(sub_a);

    let pane_b = MainTaskPane::new("Office Move");
    data.archived_panes = vec![pane_a, pane_b];

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;
    app.active_mode = AppMode::Harbor;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 640.0)));

    // 1. Search by subtask note text ("receipts") filters to ONLY "Quarterly Budget".
    app.harbor_search_query = "receipts".to_string();
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let out_filtered = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let filtered_txt = rendered_text(&out_filtered);
    assert!(filtered_txt.contains("Quarterly Budget"));
    assert!(!filtered_txt.contains("Office Move"));

    let click_at = |pos: Pos2, app: &mut WhiteboardApp| {
        let mut down = raw.clone();
        down.events.push(egui::Event::PointerMoved(pos));
        down.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down, |ctx| app.draw(ctx));

        let mut up = raw.clone();
        up.events.push(egui::Event::PointerMoved(pos));
        up.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(up, |ctx| app.draw(ctx))
    };

    // 2. First click on DELETE arms 2-click confirmation without deleting yet.
    let del_pos = out_filtered
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "DELETE" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("DELETE button must be rendered in Harbor");
    let _ = click_at(del_pos, &mut app);
    assert_eq!(app.data.archived_panes.len(), 2, "first click must NOT delete yet");
    assert!(app.harbor_confirm_delete_pane_id.is_some());

    // 3. Second click on CONFIRM? permanently deletes the archived pane.
    let out_confirm = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let confirm_pos = out_confirm
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "CONFIRM?" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("CONFIRM? pill must appear after first click");
    let _ = click_at(confirm_pos, &mut app);
    assert_eq!(app.data.archived_panes.len(), 1);
    assert_eq!(app.data.archived_panes[0].title, "Office Move");
}

#[test]
fn create_profile_for_arbitrary_date_sorts_chronologically_and_validates_format() {
    let mut data = AppData::default();
    data.language = Some(AppLang::En);
    data.has_seen_onboarding = true;
    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    // Invalid date format is rejected cleanly.
    assert!(app.create_profile_for_date("not-a-date", None).is_err());

    // Past date and future date profiles are created in strict chronological order (newest first).
    let past_id = app
        .create_profile_for_date("2020-05-10", None)
        .expect("valid past date");
    assert_eq!(app.active_profile_date(), "2020-05-10");
    assert_eq!(app.data.profiles.last().unwrap().id, past_id);

    let future_id = app
        .create_profile_for_date("2030-01-15", None)
        .expect("valid future date");
    assert_eq!(app.active_profile_date(), "2030-01-15");
    assert_eq!(app.data.profiles[0].id, future_id);
    assert!(app.data.profiles[0].title.contains("Tuesday"));
}

#[test]
fn rtl_mirrors_top_bar_bottom_progress_localizes_clock_and_enforces_28px_hitboxes() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let mut data = sample_data();
    data.language = Some(AppLang::Ar);
    data.has_seen_onboarding = true;
    let mut done_leaf = TaskNode::new("مهمة منجزة");
    done_leaf.status = TaskStatus::Completed;
    data.current_day.panes[0].subtasks.push(done_leaf);

    let mut app = WhiteboardApp::from_data(data);
    app.show_quick_entry_modal = false;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 640.0)));
    let _ = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let (ar_time_before, _) = clock_strings_in_lang(AppLang::Ar);
    let out_ar = ctx.run(raw.clone(), |ctx| app.draw(ctx));
    let (ar_time_after, _) = clock_strings_in_lang(AppLang::Ar);

    // 1. In Arabic RTL, navigation tab ("الرئيسية") sits on the RIGHT while window close `×` sits on the LEFT.
    let home_x = out_ar
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.pos.y < 36.0 && t.galley.text() == shape_arabic("الرئيسية") => {
                Some(t.pos.x)
            }
            _ => None,
        })
        .expect("Arabic HOME tab must render in top bar");
    let close_x = out_ar
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.pos.y < 36.0 && t.galley.text() == "\u{d7}" => Some(t.pos.x),
            _ => None,
        })
        .expect("window close button must render in top bar");
    assert!(
        home_x > close_x,
        "in AR RTL mode, tabs ({home_x}) must sit to the RIGHT of window controls ({close_x})"
    );

    // 2. Clock & date are localized in Arabic (`ص` or `م` suffix + Arabic weekday/month).
    assert!(ar_time_after.contains('ص') || ar_time_after.contains('م'));
    let rendered = rendered_text(&out_ar);
    let ar_short_date = short_date_string_in_lang(AppLang::Ar);
    assert!(
        rendered.contains(&disp(&ar_time_before)) || rendered.contains(&disp(&ar_time_after)),
        "expected Arabic clock ({ar_time_before:?} or {ar_time_after:?}) in rendered text: {rendered:?}"
    );
    assert!(
        rendered.contains(&disp(&ar_short_date)),
        "expected Arabic short date ({ar_short_date:?}) in rendered text: {rendered:?}"
    );

    // 3. Bottom bar 2px progress hairline fills right-to-left in Arabic (green on right, grid on left).
    let mut bottom_green: Option<Rect> = None;
    let mut bottom_grid: Option<Rect> = None;
    for c in &out_ar.shapes {
        if let egui::Shape::Rect(r) = &c.shape {
            if r.rect.top() > 600.0 && (r.rect.height() - 2.0).abs() < 0.05 {
                if r.fill == theme::GREEN_BAR {
                    bottom_green = Some(r.rect);
                } else if r.fill == theme::GRID {
                    bottom_grid = Some(r.rect);
                }
            }
        }
    }
    let (bg, gr) = (
        bottom_green.expect("bottom bar green progress hairline must exist"),
        bottom_grid.expect("bottom bar remaining grid hairline must exist"),
    );
    assert!(
        bg.left() >= gr.right() - 0.5 && bg.right() > gr.right(),
        "in AR RTL mode, bottom progress hairline must fill right-to-left (green={bg:?}, grid={gr:?})"
    );

    // 4. Enlarged hit targets are all >= 28px.
    assert!(HOVER_STRIP_H >= 28.0);
    assert!(HOVER_STRIP_SLOT_W >= 28.0);
    assert!(FOLD_HANDLE_HITBOX >= 28.0);
}

#[test]
fn arabic_text_edit_handles_visual_arrow_keys_and_mouse_clicks() {
    let ctx = egui::Context::default();
    crate::rtl_text::install_arabic_fonts(&ctx);
    crate::theme::install(&ctx);

    let id = Id::new("test_arabic_edit_input");
    let mut buf = "سلام".to_string(); // 4 chars
    let mut edit_rect = Rect::NOTHING;

    let mut raw = egui::RawInput::default();
    raw.focused = true;
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 120.0)));

    // Frame 1: focus edit and place logical cursor at end (4 = visual 0, far left).
    ctx.memory_mut(|m| m.request_focus(id));
    let _ = ctx.run(raw.clone(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let out = rtl_text_edit(ui, id, &mut buf, "", 16.0, 240.0, Margin::same(4), theme::INK, true);
            edit_rect = out.response.rect;
        });
    });
    if let Some(mut state) = egui::TextEdit::load_state(&ctx, id) {
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(4))));
        state.store(&ctx, id);
    }

    // Press ArrowRight -> moves visually right (toward logical 3).
    let mut press_right = raw.clone();
    press_right.events.push(egui::Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(press_right, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let _ = rtl_text_edit(ui, id, &mut buf, "", 16.0, 240.0, Margin::same(4), theme::INK, true);
        });
    });
    let after_right = egui::TextEdit::load_state(&ctx, id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| r.primary.index)
        .expect("cursor state after ArrowRight");
    assert_eq!(after_right, 3, "ArrowRight on Arabic text must step visually right (4 -> 3)");

    // Click near the right edge of the Arabic text -> maps to logical 0 (start of word).
    let right_click_pos = pos2(edit_rect.right() - 2.0, edit_rect.center().y);
    let mut click_right = raw.clone();
    click_right.events.push(egui::Event::PointerMoved(right_click_pos));
    click_right.events.push(egui::Event::PointerButton {
        pos: right_click_pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = ctx.run(click_right, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let _ = rtl_text_edit(ui, id, &mut buf, "", 16.0, 240.0, Margin::same(4), theme::INK, true);
        });
    });
    let after_click = egui::TextEdit::load_state(&ctx, id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| r.primary.index)
        .expect("cursor state after right-edge click");
    assert_eq!(
        after_click, 0,
        "clicking right edge of Arabic text must place logical cursor at 0 (start)"
    );
}
