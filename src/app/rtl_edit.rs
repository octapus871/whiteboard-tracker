use super::*;

/// Layouter that shapes Arabic but keeps the logical string untouched, so
/// `TextEdit` cursor indices remain character-aligned with the original text.
pub(super) fn rtl_layouter(
    font_size: f32,
    multiline: bool,
) -> impl FnMut(&Ui, &dyn egui::TextBuffer, f32) -> Arc<Galley> {
    move |ui, text, wrap_width| {
        let mut job = egui::text::LayoutJob::default();
        if multiline {
            job.wrap.max_width = wrap_width;
        }
        job.append(
            &shape_arabic(text.as_str()),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(font_size),
                color: theme::TEXT,
                ..Default::default()
            },
        );
        let mut galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        // `epaint` lays out `Align::RIGHT` rows in `[-max_w, 0.0]`, while `egui::TextEdit`
        // with `.horizontal_align(Align::RIGHT)` offsets `galley_pos.x` by `inner.right() - galley.size().x`.
        // Shifting rows by `-galley.rect.min.x` puts the galley in `[0.0, max_w]` so every
        // line's right edge aligns pixel-exact flush against the right 16.0px padding!
        let shift_x = -galley.rect.min.x;
        if shift_x > 0.0 {
            let g = Arc::make_mut(&mut galley);
            for row in &mut g.rows {
                row.pos.x += shift_x;
            }
            g.rect = g.rect.translate(vec2(shift_x, 0.0));
            g.mesh_bounds = g.mesh_bounds.translate(vec2(shift_x, 0.0));
        }
        galley
    }
}

/// Wraps and shapes a multiline logical buffer for an RTL `TextEdit` while
/// strictly preserving a 1-to-1 character count (`output.chars().count() ==
/// input.chars().count()`): any wrapping ASCII space inside an Arabic paragraph
/// is replaced by `'\n'` so lines flow top-to-bottom in natural reading order
/// without mutating the underlying logical string or breaking cursor indices.
pub(super) fn shape_multiline_for_editor(
    ui: &Ui,
    text: &str,
    font_id: &FontId,
    wrap_width: f32,
) -> String {
    if text.is_empty() {
        return String::new();
    }
    let max_w = wrap_width.max(48.0);
    let measure_w = |segment: &str| -> f32 {
        let shaped = shape_arabic(segment);
        ui.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(shaped, font_id.clone(), Color32::TRANSPARENT)
                .size()
                .x
        })
    };

    let mut out = String::with_capacity(text.len() * 2);
    let paragraphs: Vec<&str> = text.split('\n').collect();
    for (p_idx, para) in paragraphs.iter().enumerate() {
        if p_idx > 0 {
            out.push('\n');
        }
        if para.is_empty() {
            continue;
        }
        if !crate::rtl_text::contains_arabic(para) || measure_w(para) <= max_w {
            out.push_str(&shape_arabic(para));
            continue;
        }
        // Greedy line wrap preserving exact character positions: each wrapping
        // ASCII space becomes `\n`, keeping 1-to-1 char count alignment.
        let mut line_start = 0usize;
        let mut last_space: Option<usize> = None;
        for (byte_idx, ch) in para.char_indices() {
            if ch == ' ' {
                let candidate = &para[line_start..byte_idx];
                if !candidate.is_empty() && measure_w(candidate) > max_w {
                    if let Some(sp) = last_space {
                        out.push_str(&shape_arabic(&para[line_start..sp]));
                        out.push('\n');
                        line_start = sp + 1;
                    }
                }
                last_space = Some(byte_idx);
            }
        }
        let rem = &para[line_start..];
        if !rem.is_empty() && measure_w(rem) > max_w {
            if let Some(sp) = last_space {
                if sp >= line_start {
                    out.push_str(&shape_arabic(&para[line_start..sp]));
                    out.push('\n');
                    line_start = sp + 1;
                }
            }
        }
        out.push_str(&shape_arabic(&para[line_start..]));
    }
    out
}

/// Maps a logical character index in `logical_text` to its visual character
/// index in `visual_text` (produced by [`shape_multiline_for_editor`]), supporting
/// pure Arabic, pure Latin, and mixed Arabic/Latin lines accurately.
pub(super) fn rtl_multiline_visual_cursor_index(
    logical_text: &str,
    visual_text: &str,
    logical_idx: usize,
) -> usize {
    let logical_chars: Vec<char> = logical_text.chars().collect();
    let total = logical_chars.len();
    let clamped = logical_idx.min(total);
    let mut line_start = 0usize;
    for vis_line in visual_text.split('\n') {
        let line_len = vis_line.chars().count();
        let line_end = (line_start + line_len).min(total);
        if clamped <= line_end {
            let col = clamped.saturating_sub(line_start);
            let slice_str: String = logical_chars[line_start..line_end].iter().collect();
            let visual_col =
                crate::rtl_text::logical_to_visual_cursor_in_line(&slice_str, col);
            return line_start + visual_col;
        }
        line_start = line_end + 1;
    }
    clamped
}

/// Maps a visual character index in `visual_text` (e.g. from a mouse click in the
/// shaped multiline galley) back to its logical character index in `logical_text`.
pub(super) fn rtl_multiline_logical_cursor_index(
    logical_text: &str,
    visual_text: &str,
    visual_idx: usize,
) -> usize {
    let logical_chars: Vec<char> = logical_text.chars().collect();
    let total = logical_chars.len();
    let clamped = visual_idx.min(total);
    let mut line_start = 0usize;
    for vis_line in visual_text.split('\n') {
        let line_len = vis_line.chars().count();
        let line_end = (line_start + line_len).min(total);
        if clamped <= line_end {
            let vis_col = clamped.saturating_sub(line_start);
            let slice_str: String = logical_chars[line_start..line_end].iter().collect();
            let logical_col =
                crate::rtl_text::visual_to_logical_cursor_in_line(&slice_str, vis_col);
            return line_start + logical_col;
        }
        line_start = line_end + 1;
    }
    clamped
}

/// Steps a logical cursor visually left (`delta == -1`) or right (`delta == 1`)
/// inside a multiline buffer whose wrapped visual lines are given by `visual_text`.
fn step_multiline_visual_cursor(
    logical_text: &str,
    visual_text: &str,
    logical_idx: usize,
    delta: isize,
) -> usize {
    let logical_chars: Vec<char> = logical_text.chars().collect();
    let total = logical_chars.len();
    let clamped = logical_idx.min(total);
    let mut line_start = 0usize;
    for vis_line in visual_text.split('\n') {
        let line_len = vis_line.chars().count();
        let line_end = (line_start + line_len).min(total);
        if clamped <= line_end {
            let col = clamped.saturating_sub(line_start);
            let slice_str: String = logical_chars[line_start..line_end].iter().collect();
            let next_col =
                crate::rtl_text::step_visual_cursor_in_line(&slice_str, col, delta);
            return line_start + next_col;
        }
        line_start = line_end + 1;
    }
    clamped
}

/// Frameless multiline RTL `TextEdit` for the architectural note card:
/// * Keeps the mutable `text` buffer purely logical UTF-8 (never mutates with bidi).
/// * Aligns every line flush against the right padding (`16.0px`).
/// * Handles visual `ArrowLeft`/`ArrowRight` navigation and mouse click hit-testing on Arabic lines.
/// * Paints an accurate blinking RTL caret at the true visual edge on pure & mixed lines.
#[allow(clippy::too_many_arguments)]
pub(super) fn rtl_multiline_text_edit(
    ui: &mut Ui,
    id: Id,
    text: &mut String,
    hint: &str,
    font_size: f32,
    margin: Margin,
    bg_color: Color32,
    text_color: Color32,
    is_rtl: bool,
) -> egui::text_edit::TextEditOutput {
    let has_arabic = crate::rtl_text::contains_arabic(text);
    let was_focused = ui.ctx().memory(|m| m.has_focus(id));
    let prev_cursor_range = egui::TextEdit::load_state(ui.ctx(), id)
        .and_then(|s| s.cursor.char_range());
    let (arrow_delta, shift_down, pointer_pressed_pos) = ui.ctx().input(|i| {
        let no_jump_mod = !i.modifiers.ctrl && !i.modifiers.command && !i.modifiers.alt;
        let delta = if no_jump_mod && i.key_pressed(Key::ArrowLeft) {
            -1isize
        } else if no_jump_mod && i.key_pressed(Key::ArrowRight) {
            1isize
        } else {
            0isize
        };
        let ptr = if i.pointer.primary_pressed() {
            i.pointer.interact_pos()
        } else {
            None
        };
        (delta, i.modifiers.shift, ptr)
    });

    let prev_cursor = ui.visuals().text_cursor.clone();
    if has_arabic {
        ui.visuals_mut().text_cursor.stroke = Stroke::NONE;
        ui.visuals_mut().text_cursor.preview = false;
    }

    let halign = if is_rtl { Align::RIGHT } else { Align::LEFT };
    let font = FontId::proportional(font_size);
    let mut shaped_snapshot = String::new();
    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
        let shaped = shape_multiline_for_editor(ui, buf.as_str(), &font, wrap_width);
        shaped_snapshot = shaped.clone();
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = wrap_width;
        job.halign = halign;
        job.append(
            &shaped,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: text_color,
                ..Default::default()
            },
        );
        let mut galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let shift_x = -galley.rect.min.x;
        if shift_x > 0.0 {
            let g = Arc::make_mut(&mut galley);
            for row in &mut g.rows {
                row.pos.x += shift_x;
            }
            g.rect = g.rect.translate(vec2(shift_x, 0.0));
            g.mesh_bounds = g.mesh_bounds.translate(vec2(shift_x, 0.0));
        }
        galley
    };

    let mut output = TextEdit::multiline(text)
        .id(id)
        .font(font.clone())
        .desired_width(f32::INFINITY)
        .desired_rows(4)
        .frame(false)
        .margin(margin)
        .background_color(bg_color)
        .text_color(text_color)
        .horizontal_align(halign)
        .layouter(&mut layouter)
        .show(ui);

    if has_arabic {
        ui.visuals_mut().text_cursor = prev_cursor.clone();
    }

    let has_focus = output.response.has_focus();
    let viewport_focused = ui.ctx().input(|i| i.focused);

    // Correct mouse click hit-testing and visual ArrowLeft/ArrowRight movement on Arabic text.
    let mut cursor_adjusted = false;
    if has_arabic && !text.is_empty() {
        if let Some(pos) = pointer_pressed_pos {
            if output.response.rect.contains(pos) {
                let vis_cursor = output.galley.cursor_from_pos(pos - output.galley_pos);
                let logical_idx =
                    rtl_multiline_logical_cursor_index(text, &shaped_snapshot, vis_cursor.index);
                let new_range = if shift_down {
                    let secondary = prev_cursor_range
                        .map(|r| r.secondary)
                        .unwrap_or_else(|| egui::text::CCursor::new(logical_idx));
                    egui::text::CCursorRange::two(secondary, egui::text::CCursor::new(logical_idx))
                } else {
                    egui::text::CCursorRange::one(egui::text::CCursor::new(logical_idx))
                };
                output.state.cursor.set_char_range(Some(new_range));
                output.state.clone().store(ui.ctx(), output.response.id);
                output.cursor_range = Some(new_range);
                cursor_adjusted = true;
            }
        } else if was_focused && has_focus && arrow_delta != 0 {
            let base_range = prev_cursor_range.unwrap_or_else(|| {
                egui::text::CCursorRange::one(egui::text::CCursor::new(text.chars().count()))
            });
            let next_logical = step_multiline_visual_cursor(
                text,
                &shaped_snapshot,
                base_range.primary.index,
                arrow_delta,
            );
            let new_range = if shift_down {
                egui::text::CCursorRange::two(
                    base_range.secondary,
                    egui::text::CCursor::new(next_logical),
                )
            } else {
                egui::text::CCursorRange::one(egui::text::CCursor::new(next_logical))
            };
            output.state.cursor.set_char_range(Some(new_range));
            output.state.clone().store(ui.ctx(), output.response.id);
            output.cursor_range = Some(new_range);
            cursor_adjusted = true;
        }
    }

    if text.is_empty() && !has_focus && !hint.is_empty() {
        let inner = output.response.rect - margin;
        let (anchor, align2) = if is_rtl {
            (pos2(inner.right(), inner.top()), egui::Align2::RIGHT_TOP)
        } else {
            (pos2(inner.left(), inner.top()), egui::Align2::LEFT_TOP)
        };
        let painter = ui.painter_at(output.text_clip_rect.expand(1.0));
        painter.text(anchor, align2, disp(hint), font, theme::MUTED);
    } else if has_arabic && has_focus && viewport_focused {
        let logical_idx = output
            .state
            .cursor
            .char_range()
            .map(|r| r.primary.index)
            .or_else(|| output.cursor_range.map(|r| r.primary.index))
            .unwrap_or_else(|| text.chars().count());
        let visual_idx = rtl_multiline_visual_cursor_index(text, &shaped_snapshot, logical_idx);
        let local = output
            .galley
            .pos_from_cursor(egui::text::CCursor::new(visual_idx));
        let caret_x = output.galley_pos.x + local.min.x;
        let caret_top = output.galley_pos.y + local.min.y;
        let caret_bottom = output.galley_pos.y + local.max.y;

        let now = ui.ctx().input(|i| i.time);
        let reset_blink = output.response.changed() || output.response.gained_focus() || cursor_adjusted;
        let blink_id = id.with("rtl_multiline_caret_time");
        let last_active = ui.ctx().data_mut(|d| {
            if reset_blink {
                d.insert_temp(blink_id, now);
                now
            } else {
                *d.get_temp_mut_or(blink_id, now)
            }
        });
        let elapsed = (now - last_active).max(0.0) as f32;
        let on_dur = prev_cursor.on_duration.max(0.1);
        let off_dur = prev_cursor.off_duration.max(0.1);
        let period = on_dur + off_dur;
        let visible = !prev_cursor.blink || (elapsed % period) < on_dur;
        if visible {
            let painter = ui.painter_at(output.text_clip_rect.expand(2.0));
            painter.line_segment(
                [pos2(caret_x, caret_top), pos2(caret_x, caret_bottom)],
                Stroke::new(1.5, theme::INK),
            );
        }
        if prev_cursor.blink {
            ui.ctx()
                .request_repaint_after(Duration::from_millis((on_dur * 500.0) as u64));
        }
    }

    output
}

/// Maps a logical character index inside a single-line string (pure Arabic, pure Latin,
/// or mixed Arabic/Latin) to its exact visual character index in the shaped galley.
pub(super) fn rtl_visual_cursor_index(text: &str, logical_idx: usize) -> usize {
    crate::rtl_text::logical_to_visual_cursor_in_line(text, logical_idx)
}

/// Single-line frameless `TextEdit` with right-to-left Arabic alignment,
/// bidi-accurate caret placement, visual arrow-key & mouse-click handling,
/// and non-overlapping shaped placeholder.
#[allow(clippy::too_many_arguments)]
pub(super) fn rtl_text_edit(
    ui: &mut Ui,
    id: Id,
    text: &mut String,
    hint: &str,
    font_size: f32,
    desired_width: f32,
    margin: Margin,
    text_color: Color32,
    is_rtl: bool,
) -> egui::text_edit::TextEditOutput {
    let is_arabic = !text.is_empty() && crate::rtl_text::contains_arabic(text);
    let was_focused = ui.ctx().memory(|m| m.has_focus(id));
    let prev_cursor_range = egui::TextEdit::load_state(ui.ctx(), id)
        .and_then(|s| s.cursor.char_range());
    let (arrow_delta, shift_down, pointer_pressed_pos) = ui.ctx().input(|i| {
        let no_jump_mod = !i.modifiers.ctrl && !i.modifiers.command && !i.modifiers.alt;
        let delta = if no_jump_mod && i.key_pressed(Key::ArrowLeft) {
            -1isize
        } else if no_jump_mod && i.key_pressed(Key::ArrowRight) {
            1isize
        } else {
            0isize
        };
        let ptr = if i.pointer.primary_pressed() {
            i.pointer.interact_pos()
        } else {
            None
        };
        (delta, i.modifiers.shift, ptr)
    });

    let prev_cursor = ui.visuals().text_cursor.clone();
    if is_arabic {
        ui.visuals_mut().text_cursor.stroke = Stroke::NONE;
        ui.visuals_mut().text_cursor.preview = false;
    }

    let halign = if is_rtl { Align::RIGHT } else { Align::LEFT };
    let font = FontId::proportional(font_size);
    let mut layouter = rtl_layouter(font_size, false);
    let mut output = TextEdit::singleline(text)
        .id(id)
        .font(font.clone())
        .desired_width(desired_width)
        .frame(false)
        .margin(margin)
        .background_color(Color32::TRANSPARENT)
        .text_color(text_color)
        .horizontal_align(halign)
        .layouter(&mut layouter)
        .show(ui);

    if is_arabic {
        ui.visuals_mut().text_cursor = prev_cursor.clone();
    }

    let has_focus = output.response.has_focus();
    let viewport_focused = ui.ctx().input(|i| i.focused);

    let mut cursor_adjusted = false;
    if is_arabic {
        if let Some(pos) = pointer_pressed_pos {
            if output.response.rect.contains(pos) {
                let vis_cursor = output.galley.cursor_from_pos(pos - output.galley_pos);
                let logical_idx =
                    crate::rtl_text::visual_to_logical_cursor_in_line(text, vis_cursor.index);
                let new_range = if shift_down {
                    let secondary = prev_cursor_range
                        .map(|r| r.secondary)
                        .unwrap_or_else(|| egui::text::CCursor::new(logical_idx));
                    egui::text::CCursorRange::two(secondary, egui::text::CCursor::new(logical_idx))
                } else {
                    egui::text::CCursorRange::one(egui::text::CCursor::new(logical_idx))
                };
                output.state.cursor.set_char_range(Some(new_range));
                output.state.clone().store(ui.ctx(), output.response.id);
                output.cursor_range = Some(new_range);
                cursor_adjusted = true;
            }
        } else if was_focused && has_focus && arrow_delta != 0 {
            let base_range = prev_cursor_range.unwrap_or_else(|| {
                egui::text::CCursorRange::one(egui::text::CCursor::new(text.chars().count()))
            });
            let next_logical = crate::rtl_text::step_visual_cursor_in_line(
                text,
                base_range.primary.index,
                arrow_delta,
            );
            let new_range = if shift_down {
                egui::text::CCursorRange::two(
                    base_range.secondary,
                    egui::text::CCursor::new(next_logical),
                )
            } else {
                egui::text::CCursorRange::one(egui::text::CCursor::new(next_logical))
            };
            output.state.cursor.set_char_range(Some(new_range));
            output.state.clone().store(ui.ctx(), output.response.id);
            output.cursor_range = Some(new_range);
            cursor_adjusted = true;
        }
    }

    if text.is_empty() && !has_focus && !hint.is_empty() {
        let inner = output.response.rect - margin;
        let (anchor, align2) = if is_rtl {
            (pos2(inner.right(), inner.center().y), egui::Align2::RIGHT_CENTER)
        } else {
            (pos2(inner.left(), inner.center().y), egui::Align2::LEFT_CENTER)
        };
        let painter = ui.painter_at(output.text_clip_rect.expand(1.0));
        painter.text(anchor, align2, disp(hint), font, theme::MUTED);
    } else if is_arabic && has_focus && viewport_focused {
        let logical_idx = output
            .state
            .cursor
            .char_range()
            .map(|r| r.primary.index)
            .or_else(|| output.cursor_range.map(|r| r.primary.index))
            .unwrap_or_else(|| text.chars().count());
        let visual_idx = rtl_visual_cursor_index(text, logical_idx);
        let local = output
            .galley
            .pos_from_cursor(egui::text::CCursor::new(visual_idx));
        let caret_x = output.galley_pos.x + local.min.x;
        let caret_top = output.galley_pos.y + local.min.y;
        let caret_bottom = output.galley_pos.y + local.max.y;

        let now = ui.ctx().input(|i| i.time);
        let reset_blink = output.response.changed() || output.response.gained_focus() || cursor_adjusted;
        let blink_id = id.with("rtl_caret_time");
        let last_active = ui.ctx().data_mut(|d| {
            if reset_blink {
                d.insert_temp(blink_id, now);
                now
            } else {
                *d.get_temp_mut_or(blink_id, now)
            }
        });
        let elapsed = (now - last_active).max(0.0) as f32;
        let on_dur = prev_cursor.on_duration.max(0.1);
        let off_dur = prev_cursor.off_duration.max(0.1);
        let period = on_dur + off_dur;
        let visible = !prev_cursor.blink || (elapsed % period) < on_dur;
        if visible {
            let painter = ui.painter_at(output.text_clip_rect.expand(2.0));
            painter.line_segment(
                [pos2(caret_x, caret_top), pos2(caret_x, caret_bottom)],
                Stroke::new(1.5, theme::INK),
            );
        }
        if prev_cursor.blink {
            ui.ctx()
                .request_repaint_after(Duration::from_millis((on_dur * 500.0) as u64));
        }
    }

    output
}

/// Always-live frameless `TextEdit` at the bottom of a pane.
///
/// Pressing `Enter` immediately appends the trimmed text as a new top-level
/// branch (recorded in the undo stack), clears the field, and retains focus.
pub(super) fn render_pane_branch_input(
    ui: &mut Ui,
    pane_id: &str,
    input: &mut String,
    lang: AppLang,
    actions: &mut PaneActions,
) {
    let s = tr(lang);
    let response = rtl_text_edit(
        ui,
        Id::new(("pane_branch_input", pane_id)),
        input,
        s.branch_placeholder,
        14.5,
        f32::INFINITY,
        Margin::symmetric(6, 5),
        theme::INK,
        lang.is_rtl(),
    )
    .response;

    let submitted = (response.has_focus() || response.lost_focus())
        && ui.input(|i| i.key_pressed(Key::Enter));
    if submitted {
        let trimmed = input.trim().to_string();
        if !trimmed.is_empty() {
            actions.append_branch = Some((pane_id.to_string(), trimmed));
            input.clear();
            response.request_focus();
        }
    }
}
