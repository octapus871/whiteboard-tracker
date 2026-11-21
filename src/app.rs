use crate::models::{AppData, MainTaskPane, TaskNode, TaskStatus};
use crate::rtl_text::shape_arabic;
use crate::storage::save_app_data;
use crate::theme;
use egui::epaint::CubicBezierShape;
use egui::{
    pos2, vec2, Align, Button, Color32, CornerRadius, CursorIcon, FontId, Frame, Galley, Id, Key,
    Label, Layout, Margin, Painter, Pos2, Rect, RichText, ScrollArea, Sense, Shape, Stroke,
    StrokeKind, TextEdit, Ui, UiBuilder, Vec2,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

pub struct WhiteboardApp {
    pub data: AppData,
    pub show_quick_entry_modal: bool,
    pub quick_entry_input: String,
    pub focus_quick_entry: bool,

    pub selected_node_ids: HashSet<String>,
    pub selected_pane_ids: HashSet<String>,

    pub focused_node_id: Option<String>,
    pub editing_node_id: Option<String>,

    /// Node that should receive keyboard focus on the next frame (newly created).
    pub next_focus_node_id: Option<String>,

    /// Per-pane always-live inline branch draft text.
    pub pane_inputs: HashMap<String, String>,

    /// When set, the canvas shows a single maximized pane (focus mode).
    pub focused_pane_id: Option<String>,

    /// Zen mode: every chrome element is hidden except the board itself.
    pub is_zen: bool,

    /// Panes whose horizontal scroll has already been anchored to the right
    /// edge (so the root column of the mind-map is visible first).
    pub scroll_seeded: HashSet<String>,

    pub is_lassoing: bool,
    pub lasso_start: Option<Pos2>,
    pub lasso_current: Option<Pos2>,

    /// Zero-indexed active page of the paginated 4-pane board.
    pub current_page: usize,
    /// When true, the full-canvas Master Index view is shown under `BOARD`.
    pub show_master_index: bool,
    /// Draft title input at the top of the Master Index view.
    pub index_new_pane_input: String,
    /// Pane awaiting a second click ("تأكيد؟") to confirm deletion in Master Index.
    pub index_confirm_delete_pane_id: Option<String>,
    /// Pane whose main title is being inline-edited in the Master Index.
    pub index_editing_pane_id: Option<String>,
    /// Requests keyboard focus on a Master Index title editor next frame.
    pub index_focus_edit_id: Option<String>,

    /// Per-pane embedded inline note drawer: maps `pane_id -> open_node_id`.
    pub pane_note_drawers: HashMap<String, String>,
    /// Node ID whose embedded drawer `TextEdit` should grab focus next frame.
    pub focus_notes_editor: Option<String>,

    pub active_mode: AppMode,
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    LiveBoard,
    Harbor,
}

/// Strict maximum number of panes rendered on a single board page (2x2 grid).
const PANES_PER_PAGE: usize = 4;

impl WhiteboardApp {
    pub fn new(cc: &eframe::CreationContext<'_>, data: AppData) -> Self {
        crate::rtl_text::install_arabic_fonts(&cc.egui_ctx);
        theme::install(&cc.egui_ctx);
        Self::from_data(data)
    }

    pub fn from_data(data: AppData) -> Self {
        let show_quick_entry_modal = data.current_day.panes.is_empty();

        Self {
            data,
            show_quick_entry_modal,
            quick_entry_input: String::new(),
            focus_quick_entry: show_quick_entry_modal,
            selected_node_ids: HashSet::new(),
            selected_pane_ids: HashSet::new(),
            focused_node_id: None,
            editing_node_id: None,
            next_focus_node_id: None,
            pane_inputs: HashMap::new(),
            focused_pane_id: None,
            is_zen: false,
            scroll_seeded: HashSet::new(),
            is_lassoing: false,
            lasso_start: None,
            lasso_current: None,
            current_page: 0,
            show_master_index: false,
            index_new_pane_input: String::new(),
            index_confirm_delete_pane_id: None,
            index_editing_pane_id: None,
            index_focus_edit_id: None,
            pane_note_drawers: HashMap::new(),
            focus_notes_editor: None,
            active_mode: AppMode::LiveBoard,
            dirty: false,
        }
    }

    pub fn auto_save(&mut self) {
        if self.dirty {
            if let Err(e) = save_app_data(&self.data) {
                eprintln!("Error saving data: {}", e);
            } else {
                self.dirty = false;
            }
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_selected_nodes_status(&mut self, status: TaskStatus) {
        if self.selected_node_ids.is_empty() && self.selected_pane_ids.is_empty() {
            return;
        }

        for pane in &mut self.data.current_day.panes {
            if self.selected_pane_ids.contains(&pane.id) {
                pane.status = status;
            }
            set_status_in_nodes(&mut pane.subtasks, &self.selected_node_ids, status);
        }
        self.mark_dirty();
    }

    pub fn cycle_selected_or_focused_status(&mut self) {
        let mut target_nodes = self.selected_node_ids.clone();
        if target_nodes.is_empty() && self.selected_pane_ids.is_empty() {
            if let Some(focused) = &self.focused_node_id {
                target_nodes.insert(focused.clone());
            } else {
                return;
            }
        }

        for pane in &mut self.data.current_day.panes {
            if self.selected_pane_ids.contains(&pane.id) {
                pane.status = pane.status.next();
            }
            cycle_status_in_nodes(&mut pane.subtasks, &target_nodes);
        }
        self.mark_dirty();
    }

    /// Total pages in the paginated 4-pane board (`>= 1`).
    pub fn total_pages(&self) -> usize {
        self.data
            .current_day
            .panes
            .len()
            .div_ceil(PANES_PER_PAGE)
            .max(1)
    }

    fn clamp_current_page(&mut self) {
        let max_page = self.total_pages().saturating_sub(1);
        if self.current_page > max_page {
            self.current_page = max_page;
        }
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.focused_pane_id = None;
        }
    }

    pub fn next_page(&mut self) {
        if self.current_page + 1 < self.total_pages() {
            self.current_page += 1;
            self.focused_pane_id = None;
        }
    }

    /// Creates a new main pane and automatically jumps `current_page` to the
    /// page containing it.
    pub fn add_main_pane(&mut self, title: &str) {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return;
        }
        self.data.current_day.panes.push(MainTaskPane::new(trimmed));
        self.current_page = (self.data.current_day.panes.len() - 1) / PANES_PER_PAGE;
        self.mark_dirty();
        self.auto_save();
    }

    /// Deletes the main pane at `idx`, cleaning up per-pane state and clamping `current_page`.
    pub fn delete_pane_at(&mut self, idx: usize, ctx: Option<&egui::Context>) {
        if idx < self.data.current_day.panes.len() {
            let removed = self.data.current_day.panes.remove(idx);
            self.selected_pane_ids.remove(&removed.id);
            self.pane_inputs.remove(&removed.id);
            self.close_note_drawer_for_pane(&removed.id, ctx);
            if self.focused_pane_id.as_deref() == Some(removed.id.as_str()) {
                self.focused_pane_id = None;
            }
            if self.index_confirm_delete_pane_id.as_deref() == Some(removed.id.as_str()) {
                self.index_confirm_delete_pane_id = None;
            }
            if self.index_editing_pane_id.as_deref() == Some(removed.id.as_str()) {
                self.index_editing_pane_id = None;
            }
            self.clamp_current_page();
            self.mark_dirty();
        }
    }

    /// Moves a pane up one slot (crossing page boundaries automatically).
    pub fn move_pane_up(&mut self, idx: usize) {
        if idx > 0 && idx < self.data.current_day.panes.len() {
            self.data.current_day.panes.swap(idx - 1, idx);
            self.mark_dirty();
        }
    }

    /// Moves a pane down one slot (crossing page boundaries automatically).
    pub fn move_pane_down(&mut self, idx: usize) {
        if idx + 1 < self.data.current_day.panes.len() {
            self.data.current_day.panes.swap(idx, idx + 1);
            self.mark_dirty();
        }
    }

    /// Moves a pane directly to the next page's slot (cycling across pages).
    pub fn move_pane_to_next_page(&mut self, idx: usize) {
        let len = self.data.current_day.panes.len();
        if idx >= len {
            return;
        }
        let total_pages = self.total_pages();
        if total_pages <= 1 {
            return;
        }
        let cur_page = idx / PANES_PER_PAGE;
        let target_page = (cur_page + 1) % total_pages;
        let pane = self.data.current_day.panes.remove(idx);
        let insert_idx = (target_page * PANES_PER_PAGE).min(self.data.current_day.panes.len());
        self.data.current_day.panes.insert(insert_idx, pane);
        self.mark_dirty();
    }

    /// Toggles Focus Mode for the pane at the given zero-based slot index on
    /// the current page (or global index when `idx >= 4`).
    fn focus_pane_by_index(&mut self, idx: usize) {
        let target_idx = if idx < PANES_PER_PAGE {
            self.current_page * PANES_PER_PAGE + idx
        } else {
            idx
        };
        if let Some(pane) = self.data.current_day.panes.get(target_idx) {
            let id = pane.id.clone();
            self.focused_pane_id = if self.focused_pane_id.as_deref() == Some(id.as_str()) {
                None
            } else {
                Some(id)
            };
        }
    }

    /// `Alt+1`..`Alt+9` (always) and `1`..`9` (when no `TextEdit` has focus)
    /// toggle Focus Mode for the matching pane. When nodes or panes are
    /// selected, `1`..`3` keep their status-assignment meaning instead.
    fn handle_pane_hotkeys(&mut self, ctx: &egui::Context) {
        if self.show_quick_entry_modal {
            return;
        }
        const DIGITS: [Key; 9] = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];
        let text_focus = ctx.wants_keyboard_input();
        let alt = ctx.input(|i| i.modifiers.alt);
        let has_selection =
            !self.selected_node_ids.is_empty() || !self.selected_pane_ids.is_empty();

        for (i, key) in DIGITS.iter().enumerate() {
            if !ctx.input(|inp| inp.key_pressed(*key)) {
                continue;
            }
            if alt {
                self.focus_pane_by_index(i);
            } else if !text_focus {
                if has_selection {
                    let status = match i {
                        0 => Some(TaskStatus::Active),
                        1 => Some(TaskStatus::Completed),
                        2 => Some(TaskStatus::Cancelled),
                        _ => None,
                    };
                    if let Some(status) = status {
                        self.set_selected_nodes_status(status);
                    }
                } else {
                    self.focus_pane_by_index(i);
                }
            }
        }
    }

    pub fn process_quick_entry(&mut self) {
        let input = self.quick_entry_input.trim();
        if input.is_empty() {
            self.show_quick_entry_modal = false;
            return;
        }

        let raw_tasks: Vec<&str> = input
            .split(&['/', '\n'][..])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for title in raw_tasks {
            self.data.current_day.panes.push(MainTaskPane::new(title));
        }

        if !self.data.current_day.panes.is_empty() {
            self.current_page = (self.data.current_day.panes.len() - 1) / PANES_PER_PAGE;
        }

        self.quick_entry_input.clear();
        self.show_quick_entry_modal = false;
        self.focus_quick_entry = false;
        self.mark_dirty();
        self.auto_save();
    }

    // ---------------------------------------------------------------------
    // Keyboard outlining
    // ---------------------------------------------------------------------

    fn handle_keyboard_navigation(&mut self, ctx: &egui::Context) {
        if self.show_quick_entry_modal || ctx.wants_keyboard_input() {
            return;
        }

        // Page navigation shortcuts: PageUp / PageDown and Alt+Left / Alt+Right.
        let (page_up, page_down, alt_left, alt_right) = ctx.input(|i| {
            (
                i.key_pressed(Key::PageUp),
                i.key_pressed(Key::PageDown),
                i.modifiers.alt && i.key_pressed(Key::ArrowLeft),
                i.modifiers.alt && i.key_pressed(Key::ArrowRight),
            )
        });
        if page_up || alt_left {
            self.prev_page();
            return;
        }
        if page_down || alt_right {
            self.next_page();
            return;
        }

        if self.show_master_index {
            return;
        }

        if ctx.input(|i| i.key_pressed(Key::Space)) {
            self.cycle_selected_or_focused_status();
            return;
        }

        let ctrl = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let alt = ctx.input(|i| i.modifiers.alt);
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        let n_key = ctx.input(|i| i.key_pressed(Key::N) && !ctrl && !alt);

        if (ctrl && enter) || n_key {
            if let Some(target_id) = self.selected_target_node_id() {
                self.open_note_drawer(target_id, true);
                return;
            }
        }

        let Some(focused) = self.focused_node_id.clone() else {
            return;
        };

        let tab = ctx.input(|i| i.key_pressed(Key::Tab));
        let shift = ctx.input(|i| i.modifiers.shift);
        let delete = ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));

        if enter && !ctrl {
            self.add_sibling_after(&focused);
        } else if tab && !shift {
            self.add_child_under(&focused);
        } else if tab && shift {
            self.outdent_node(&focused);
        } else if delete {
            self.delete_node(&focused);
        }
    }

    /// Finds the pane ID that owns the given task node anywhere in its tree.
    fn find_pane_id_for_node(&self, node_id: &str) -> Option<String> {
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

    /// Returns the primary selected node ID (preferring the focused node when
    /// it belongs to the current selection).
    fn selected_target_node_id(&self) -> Option<String> {
        if self.selected_node_ids.is_empty() {
            return None;
        }
        if let Some(focused) = &self.focused_node_id {
            if self.selected_node_ids.contains(focused) {
                return Some(focused.clone());
            }
        }
        fn first_selected(nodes: &[TaskNode], selected: &HashSet<String>) -> Option<String> {
            for node in nodes {
                if selected.contains(&node.id) {
                    return Some(node.id.clone());
                }
                if let Some(found) = first_selected(&node.children, selected) {
                    return Some(found);
                }
            }
            None
        }
        for pane in &self.data.current_day.panes {
            if let Some(found) = first_selected(&pane.subtasks, &self.selected_node_ids) {
                return Some(found);
            }
        }
        None
    }

    fn add_sibling_after(&mut self, target_id: &str) -> Option<String> {
        let new_node = TaskNode::new("");
        let new_id = new_node.id.clone();
        for pane in &mut self.data.current_day.panes {
            if insert_sibling_in_nodes(&mut pane.subtasks, target_id, new_node.clone()) {
                return self.begin_editing(new_id);
            }
        }
        None
    }

    fn add_child_under(&mut self, parent_id: &str) -> Option<String> {
        let new_node = TaskNode::new("");
        let new_id = new_node.id.clone();
        for pane in &mut self.data.current_day.panes {
            if insert_child_recursive(&mut pane.subtasks, parent_id, new_node.clone()) {
                return self.begin_editing(new_id);
            }
        }
        None
    }

    fn outdent_node(&mut self, target_id: &str) -> Option<String> {
        for pane in &mut self.data.current_day.panes {
            if outdent_recursive(&mut pane.subtasks, target_id) {
                self.mark_dirty();
                self.focused_node_id = Some(target_id.to_string());
                return Some(target_id.to_string());
            }
        }
        None
    }

    fn delete_node(&mut self, target_id: &str) -> Option<String> {
        for pane in &mut self.data.current_day.panes {
            if delete_node_in_nodes(&mut pane.subtasks, target_id) {
                self.selected_node_ids.remove(target_id);
                if self.focused_node_id.as_deref() == Some(target_id) {
                    self.focused_node_id = None;
                }
                if self.editing_node_id.as_deref() == Some(target_id) {
                    self.editing_node_id = None;
                }
                self.pane_note_drawers.retain(|_, v| v != target_id);
                if self.focus_notes_editor.as_deref() == Some(target_id) {
                    self.focus_notes_editor = None;
                }
                self.mark_dirty();
                return Some(target_id.to_string());
            }
        }
        None
    }

    /// Cancels the current edit. A freshly created node that is still empty is
    /// deleted again. Returns true when an edit was actually cancelled.
    fn cancel_editing(&mut self) -> bool {
        let Some(node_id) = self.editing_node_id.take() else {
            return false;
        };
        if self
            .find_node_title(&node_id)
            .is_some_and(|title| title.trim().is_empty())
        {
            self.delete_node(&node_id);
            self.focused_node_id = None;
        }
        true
    }

    /// Looks up a task title anywhere in the tree.
    fn find_node_title(&self, node_id: &str) -> Option<String> {
        fn search(nodes: &[TaskNode], node_id: &str) -> Option<String> {
            for node in nodes {
                if node.id == node_id {
                    return Some(node.title.clone());
                }
                if let Some(title) = search(&node.children, node_id) {
                    return Some(title);
                }
            }
            None
        }
        for pane in &self.data.current_day.panes {
            if let Some(title) = search(&pane.subtasks, node_id) {
                return Some(title);
            }
        }
        None
    }

    fn begin_editing(&mut self, node_id: String) -> Option<String> {
        self.focused_node_id = Some(node_id.clone());
        self.editing_node_id = Some(node_id.clone());
        self.next_focus_node_id = Some(node_id.clone());
        self.mark_dirty();
        Some(node_id)
    }

    fn apply_lasso(
        &mut self,
        lasso: Rect,
        node_rects: &[(String, Rect)],
        pane_header_rects: &[(String, Rect)],
    ) {
        if lasso.width() < 4.0 && lasso.height() < 4.0 {
            return;
        }

        self.selected_node_ids.clear();
        self.selected_pane_ids.clear();

        for (id, rect) in node_rects {
            if lasso.intersects(*rect) {
                self.selected_node_ids.insert(id.clone());
            }
        }
        for (id, rect) in pane_header_rects {
            if lasso.intersects(*rect) {
                self.selected_pane_ids.insert(id.clone());
            }
        }
    }
}

impl eframe::App for WhiteboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

impl WhiteboardApp {
    /// Draws the whole application. Separated from [`eframe::App::update`] so it
    /// can be exercised headlessly in tests.
    pub fn draw(&mut self, ctx: &egui::Context) {
        // Keep the live digital clock ticking at one frame per second.
        ctx.request_repaint_after(Duration::from_secs(1));

        // Escape always works - even while a text field has focus - so an
        // edit can be cancelled (and an empty new node removed again).
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.show_quick_entry_modal {
                self.show_quick_entry_modal = false;
                self.focus_quick_entry = false;
            } else if self.index_editing_pane_id.is_some() {
                self.index_editing_pane_id = None;
            } else if self.index_confirm_delete_pane_id.is_some() {
                self.index_confirm_delete_pane_id = None;
            } else if self.show_master_index {
                self.show_master_index = false;
            } else if !self.cancel_editing() {
                if !self.pane_note_drawers.is_empty() {
                    self.close_all_note_drawers(Some(ctx));
                } else if self.is_zen {
                    self.is_zen = false;
                } else if self.focused_pane_id.is_some() {
                    self.focused_pane_id = None;
                } else {
                    self.selected_node_ids.clear();
                    self.selected_pane_ids.clear();
                }
            }
        } else {
            if !ctx.wants_keyboard_input() {
                let close_shortcut = ctx
                    .input(|i| i.key_pressed(Key::Q) && (i.modifiers.ctrl || i.modifiers.command));
                let zen_toggle = ctx
                    .input(|i| i.key_pressed(Key::H) && (i.modifiers.ctrl || i.modifiers.command));
                if close_shortcut {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else if zen_toggle {
                    self.is_zen = !self.is_zen;
                }
            }

            if !self.show_master_index {
                self.handle_pane_hotkeys(ctx);
            }
        }

        self.clamp_current_page();
        self.handle_keyboard_navigation(ctx);

        if self.is_zen {
            // Pure live board: no nav bars, no footer, no action buttons.
            egui::CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(theme::CANVAS)
                        .inner_margin(Margin::same(0)),
                )
                .show(ctx, |ui| {
                    self.render_zen_header(ui);
                    self.render_daily_canvas(ui);
                });
        } else {
            egui::TopBottomPanel::top("top_bar")
                .exact_height(36.0)
                .frame(
                    Frame::new()
                        .fill(theme::PANE)
                        .inner_margin(Margin::symmetric(24, 0)),
                )
                .show(ctx, |ui| {
                    self.render_top_bar(ui);
                });

            // The status bar is laid out BEFORE the central panel so the
            // board never overlaps (or clips) the metric badges.
            egui::TopBottomPanel::bottom("bottom_bar")
                .exact_height(28.0)
                .frame(Frame::new().fill(theme::PANE).inner_margin(Margin::same(0)))
                .show(ctx, |ui| {
                    self.render_bottom_bar(ui);
                });

            egui::CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(theme::CANVAS)
                        .inner_margin(Margin::same(0)),
                )
                .show(ctx, |ui| match self.active_mode {
                    AppMode::LiveBoard => {
                        if self.show_master_index {
                            self.render_master_index(ui);
                        } else {
                            self.render_daily_canvas(ui);
                        }
                    }
                    AppMode::Harbor => self.render_harbor_placeholder(ui),
                });
        }

        if self.show_quick_entry_modal {
            self.render_quick_entry_modal(ctx);
        }

        if self.dirty {
            self.auto_save();
        }
    }
}

impl WhiteboardApp {
    /// Ultra-minimalist single-line top bar: `BOARD | HARBOR` on the left,
    /// centered `‹ PAGE 1 / N ›` page navigator, and clock + window controls on
    /// the right, over a 1px black hairline.
    fn render_top_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let bar_rect = ui.max_rect();
        let total_pages = self.total_pages();

        // Background drag region across the frameless top bar (interactive controls sit on top).
        let drag_response = ui.interact(bar_rect, ui.id().with("top_bar_drag"), Sense::drag());
        if drag_response.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // Left navigation: `BOARD | HARBOR` (or `← BACK [Esc]` when maximized).
        let left_rect = Rect::from_min_max(
            bar_rect.left_top(),
            pos2((bar_rect.center().x - 95.0).max(bar_rect.left() + 140.0), bar_rect.bottom()),
        );
        let mut left_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(left_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        left_ui.spacing_mut().item_spacing.x = 12.0;
        if self.focused_pane_id.is_some() {
            if text_link(&mut left_ui, "← BACK [Esc]", true).clicked() {
                self.focused_pane_id = None;
            }
        } else {
            if text_link(&mut left_ui, "BOARD", self.active_mode == AppMode::LiveBoard)
                .on_hover_text("TOGGLE MASTER INDEX / BOARD")
                .clicked()
            {
                if self.active_mode == AppMode::LiveBoard {
                    self.show_master_index = !self.show_master_index;
                    self.index_confirm_delete_pane_id = None;
                    self.index_editing_pane_id = None;
                } else {
                    self.active_mode = AppMode::LiveBoard;
                    self.show_master_index = false;
                }
            }
            left_ui.label(mono("|", 14.0).color(theme::DIVIDER));
            if text_link(&mut left_ui, "HARBOR", self.active_mode == AppMode::Harbor).clicked() {
                self.active_mode = AppMode::Harbor;
                self.show_master_index = false;
            }
        }

        // Center page navigator: `‹ PAGE 1 / N ›`.
        let center_w = 190.0;
        let center_rect = Rect::from_center_size(
            bar_rect.center(),
            vec2(center_w, bar_rect.height()),
        );
        let mut center_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(center_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        center_ui.with_layout(Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let can_prev = self.current_page > 0;
                let can_next = self.current_page + 1 < total_pages;

                let prev_color = if can_prev { theme::INK } else { theme::MUTED };
                let prev_resp = ui
                    .add(
                        Label::new(mono("‹", 16.0).strong().color(prev_color))
                            .selectable(false)
                            .sense(Sense::click()),
                    )
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text("PREVIOUS PAGE [PageUp / Alt+Left]");
                if prev_resp.clicked() && can_prev {
                    self.prev_page();
                }

                ui.label(
                    mono(
                        format!("PAGE {} / {}", self.current_page + 1, total_pages),
                        13.5,
                    )
                    .strong()
                    .color(theme::INK),
                );

                let next_color = if can_next { theme::INK } else { theme::MUTED };
                let next_resp = ui
                    .add(
                        Label::new(mono("›", 16.0).strong().color(next_color))
                            .selectable(false)
                            .sense(Sense::click()),
                    )
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text("NEXT PAGE [PageDown / Alt+Right]");
                if next_resp.clicked() && can_next {
                    self.next_page();
                }
            });
        });

        // Right cluster: window controls + digital clock.
        let right_rect = Rect::from_min_max(
            pos2((bar_rect.center().x + 95.0).min(bar_rect.right() - 180.0), bar_rect.top()),
            bar_rect.right_bottom(),
        );
        let mut right_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(right_rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        right_ui.spacing_mut().item_spacing.x = 12.0;
        if window_control(&mut right_ui, "\u{d7}", "CLOSE").clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if window_control(&mut right_ui, "\u{25a1}", "MAXIMIZE").clicked() {
            let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        if window_control(&mut right_ui, "\u{2014}", "MINIMIZE").clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        right_ui.add_space(10.0);
        let (time, _date) = clock_strings();
        let short_date = short_date_string();
        right_ui.label(mono(&time, 14.0).strong().color(theme::INK));
        right_ui.label(mono(short_date, 12.5).color(theme::SLATE));

        // 1px black hairline separating the strip from the board.
        let rule = Rect::from_min_max(
            pos2(bar_rect.left(), bar_rect.bottom() - theme::HAIRLINE),
            pos2(bar_rect.right(), bar_rect.bottom()),
        );
        ui.painter()
            .rect_filled(rule, CornerRadius::same(0), theme::GRID);
    }

    fn render_bottom_bar(&mut self, ui: &mut Ui) {
        let counts = self.data.current_day.calculate_counts();
        let rate = counts.completion_rate();

        // Continuous 2px progress hairline spanning the full window width:
        // green for the completed share, charcoal for the rest.
        let rect = ui.max_rect();
        let width = rect.width();
        let done = (rate / 100.0).clamp(0.0, 1.0);
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(rect.left(), rect.top()),
                pos2(rect.left() + width * done, rect.top() + 2.0),
            ),
            CornerRadius::same(0),
            theme::GREEN_BAR,
        );
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(rect.left() + width * done, rect.top()),
                pos2(rect.right(), rect.top() + 2.0),
            ),
            CornerRadius::same(0),
            theme::GRID,
        );

        ui.add_space(2.0);

        // Borderless monospace metrics separated by dots.
        ui.horizontal_centered(|ui| {
            ui.add_space(24.0);
            metric(ui, counts.completed_count, theme::GREEN_SHADOW, "DONE");
            ui.label(mono("\u{2022}", 12.5).color(theme::DIVIDER));
            metric(ui, counts.active_count, theme::AMBER_SHADOW, "ACTIVE");
            ui.label(mono("\u{2022}", 12.5).color(theme::DIVIDER));
            metric(ui, counts.cancelled_count, theme::RED_SHADOW, "CANCELLED");
            ui.label(mono("\u{2022}", 12.5).color(theme::DIVIDER));
            metric(ui, counts.total, theme::INK, "TOTAL");

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(24.0);
                if self.dirty {
                    ui.label(mono("SAVING...", 12.5).strong().color(theme::AMBER_INK));
                } else {
                    ui.label(mono("AUTO-SAVED", 12.5).color(theme::SLATE));
                }
            });
        });
    }

    fn render_quick_entry_modal(&mut self, ctx: &egui::Context) {
        egui::Window::new("QUICK TASK ENTRY")
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                    let (_, date) = clock_strings();
                    ui.label(mono(&date, 14.0).strong());
                    ui.label(
                        mono("ENTER A MAIN TASK, OR SEPARATE SEVERAL WITH /", 10.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(10.0);

                    let response = rtl_text_edit(
                        ui,
                        Id::new("quick_entry_input"),
                        &mut self.quick_entry_input,
                        "علاج بابا / كتابة اللائحة للمرافعة",
                        16.0,
                        f32::INFINITY,
                        Margin {
                            left: 0,
                            right: 10,
                            top: 10,
                            bottom: 10,
                        },
                        theme::INK,
                    )
                    .response;
                    if self.focus_quick_entry {
                        response.request_focus();
                        self.focus_quick_entry = false;
                    }
                    let enter = (response.has_focus() || response.lost_focus())
                        && ui.input(|i| i.key_pressed(Key::Enter));
                    if enter {
                        self.process_quick_entry();
                    }

                    ui.add_space(12.0);

                    let raw_tasks: Vec<&str> = self
                        .quick_entry_input
                        .split(&['/', '\n'][..])
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();

                    if !raw_tasks.is_empty() {
                        ui.label(
                            mono(format!("PREVIEW PANES ({})", raw_tasks.len()), 10.0)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(4.0);
                        ScrollArea::vertical().max_height(130.0).show(ui, |ui| {
                            for task in &raw_tasks {
                                rtl_row(ui, 20.0, |ui| {
                                    ui.label(
                                        RichText::new(shape_arabic(task))
                                            .size(14.0)
                                            .color(theme::TEXT),
                                    );
                                    let (rect, _) =
                                        ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 3.5, theme::SLATE);
                                });
                            }
                        });
                        ui.add_space(8.0);
                    }

                    ui.add_space(6.0);
                    rtl_row(ui, 34.0, |ui| {
                        let submit = pill_button(ui, "CREATE BOARD", theme::GRID, true);
                        if submit.clicked() {
                            self.process_quick_entry();
                        }
                        if !self.data.current_day.panes.is_empty()
                            && ui
                                .add(
                                    Button::new(mono("CANCEL", 13.0).color(theme::TEXT_MUTED))
                                        .frame(false),
                                )
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked()
                        {
                            self.show_quick_entry_modal = false;
                            self.focus_quick_entry = false;
                        }
                    });
                });
            });
    }

    /// Full-canvas Swiss architectural Master Index view (`#FFFFFF`, `0.0px`):
    /// lists only main pane titles grouped into pages of 4 separated by `2.0px`
    /// solid `#0F172A` dividers (`PAGE 01`, `PAGE 02`, ...), with inline add,
    /// 2-click delete confirmation, `↑`/`↓` reordering, `[P1]`/`[P2]` page
    /// cycling, and title editing.
    fn render_master_index(&mut self, ui: &mut Ui) {
        let full_rect = ui.max_rect();
        ui.painter()
            .rect_filled(full_rect, CornerRadius::same(0), theme::PANE);

        let mut open_page: Option<usize> = None;
        let mut move_up: Option<usize> = None;
        let mut move_down: Option<usize> = None;
        let mut move_next_page: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;
        let mut add_title: Option<String> = None;
        let mut focus_add_input = false;
        let mut dirty = false;

        let total_pages = self.total_pages();
        let pane_count = self.data.current_day.panes.len();
        let ctx = ui.ctx().clone();

        ScrollArea::vertical()
            .id_salt("master_index_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                let content_w = (avail_w - 64.0).clamp(320.0, 900.0);
                let side_pad = ((avail_w - content_w) * 0.5).max(16.0);

                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(side_pad);
                    ui.allocate_ui_with_layout(
                        vec2(content_w, 0.0),
                        Layout::top_down(Align::Max),
                        |ui| {
                            ui.set_min_width(content_w);

                            // Top inline RTL TextEdit to add a new main pane on Enter.
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
                                    "+ اسم المهمة الرئيسية الجديدة...",
                                    16.0,
                                    f32::INFINITY,
                                    Margin::symmetric(4, 4),
                                    theme::INK,
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
                                        mono("NO MAIN PANES YET — TYPE ABOVE AND PRESS ENTER", 13.0)
                                            .color(theme::SLATE),
                                    )
                                    .selectable(false)
                                    .halign(Align::Max),
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

                                // Page group header (`PAGE 01`, `PAGE 02`, ... in 14.0px Bold #0F172A)
                                // paired with a crisp 2.0px solid #0F172A horizontal divider line.
                                rtl_row(ui, 26.0, |ui| {
                                    let page_label = format!("PAGE {:02}", page_idx + 1);
                                    let page_resp = ui
                                        .add(
                                            Label::new(
                                                mono(&page_label, 14.0)
                                                    .strong()
                                                    .color(theme::INK),
                                            )
                                            .selectable(false)
                                            .sense(Sense::click()),
                                        )
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text("OPEN THIS PAGE");
                                    if page_resp.clicked() {
                                        open_page = Some(page_idx);
                                    }
                                    if page_idx == self.current_page {
                                        ui.label(
                                            mono("ACTIVE", 11.5).strong().color(theme::GREEN_BAR),
                                        );
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
                                    let (done, total) = count_pane_progress(&pane.subtasks);
                                    let is_editing =
                                        self.index_editing_pane_id.as_deref() == Some(pane_id.as_str());
                                    let is_confirming_del = self
                                        .index_confirm_delete_pane_id
                                        .as_deref()
                                        == Some(pane_id.as_str());

                                    rtl_row(ui, 46.0, |ui| {
                                        ui.spacing_mut().item_spacing.x = 10.0;

                                        // Rightmost: monospace index `[0N]`.
                                        ui.label(
                                            mono(format!("[{:02}]", idx + 1), 14.0)
                                                .strong()
                                                .color(theme::INK_SOFT),
                                        );

                                        // Title region: clickable 18.0px Bold title (opens page) or inline RTL TextEdit.
                                        let controls_w = if is_confirming_del { 292.0 } else { 248.0 };
                                        let title_w = (ui.available_width() - controls_w).max(120.0);

                                        if is_editing {
                                            let edit_resp = rtl_text_edit(
                                                ui,
                                                Id::new(("index_pane_title_edit", pane_id.as_str())),
                                                &mut pane.title,
                                                "اسم المهمة...",
                                                18.0,
                                                title_w,
                                                Margin::symmetric(6, 4),
                                                theme::INK,
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
                                            let finish = edit_resp.lost_focus()
                                                || ((edit_resp.has_focus()
                                                    || edit_resp.lost_focus())
                                                    && ui.input(|i| i.key_pressed(Key::Enter)));
                                            if finish {
                                                self.index_editing_pane_id = None;
                                            }
                                        } else {
                                            let title_display = if pane.title.trim().is_empty() {
                                                shape_arabic("بدون عنوان")
                                            } else {
                                                shape_arabic(&pane.title)
                                            };
                                            let title_resp = ui
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
                                                    .halign(Align::Max)
                                                    .truncate(),
                                                )
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text("CLICK: OPEN PAGE");
                                            if title_resp.clicked() {
                                                open_page = Some(page_idx);
                                            }
                                        }

                                        // Left-aligned controls cluster.
                                        ui.with_layout(
                                            Layout::left_to_right(Align::Center),
                                            |ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0;

                                                // 1. Delete (`✕` -> 2-click confirm `"تأكيد؟"`).
                                                if is_confirming_del {
                                                    let confirm_resp = Frame::new()
                                                        .fill(theme::RED_BAR)
                                                        .corner_radius(CornerRadius::same(0))
                                                        .inner_margin(Margin::symmetric(8, 3))
                                                        .show(ui, |ui| {
                                                            ui.add(
                                                                Label::new(
                                                                    RichText::new(shape_arabic(
                                                                        "تأكيد؟",
                                                                    ))
                                                                    .size(13.0)
                                                                    .strong()
                                                                    .color(theme::PANE),
                                                                )
                                                                .selectable(false)
                                                                .sense(Sense::click()),
                                                            )
                                                            .on_hover_cursor(
                                                                CursorIcon::PointingHand,
                                                            )
                                                        })
                                                        .inner;
                                                    if confirm_resp.clicked() {
                                                        delete_idx = Some(idx);
                                                    }
                                                } else {
                                                    let (del_rect, del_resp) =
                                                        ui.allocate_exact_size(
                                                            vec2(26.0, 26.0),
                                                            Sense::click(),
                                                        );
                                                    let del_resp = del_resp
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .on_hover_text("DELETE PANE");
                                                    let del_color = if del_resp.hovered() {
                                                        theme::RED_BAR
                                                    } else {
                                                        theme::SLATE
                                                    };
                                                    ui.painter().text(
                                                        del_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "✕",
                                                        FontId::monospace(14.0),
                                                        del_color,
                                                    );
                                                    if del_resp.clicked() {
                                                        self.index_confirm_delete_pane_id =
                                                            Some(pane_id.clone());
                                                    }
                                                }

                                                // 2. Inline title edit toggle (`✎`).
                                                let (edit_rect, edit_btn) = ui.allocate_exact_size(
                                                    vec2(26.0, 26.0),
                                                    Sense::click(),
                                                );
                                                let edit_btn = edit_btn
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text("EDIT TITLE");
                                                let edit_color = if is_editing || edit_btn.hovered()
                                                {
                                                    theme::INK
                                                } else {
                                                    theme::SLATE
                                                };
                                                ui.painter().text(
                                                    edit_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "✎",
                                                    FontId::proportional(14.0),
                                                    edit_color,
                                                );
                                                if edit_btn.clicked() {
                                                    self.index_confirm_delete_pane_id = None;
                                                    if is_editing {
                                                        self.index_editing_pane_id = None;
                                                    } else {
                                                        self.index_editing_pane_id =
                                                            Some(pane_id.clone());
                                                        self.index_focus_edit_id =
                                                            Some(pane_id.clone());
                                                    }
                                                }

                                                // 3. Reorder up (`↑`) and down (`↓`).
                                                let can_up = idx > 0;
                                                let can_down = idx + 1 < pane_count;

                                                let up_resp = index_icon_button(ui, "↑", can_up)
                                                    .on_hover_text("MOVE UP");
                                                if up_resp.clicked() && can_up {
                                                    move_up = Some(idx);
                                                    self.index_confirm_delete_pane_id = None;
                                                }

                                                let down_resp =
                                                    index_icon_button(ui, "↓", can_down)
                                                        .on_hover_text("MOVE DOWN");
                                                if down_resp.clicked() && can_down {
                                                    move_down = Some(idx);
                                                    self.index_confirm_delete_pane_id = None;
                                                }

                                                // 4. Page assignment badge `[P1]` / `[P2]` (cycles to next page).
                                                let p_badge = format!("[P{}]", page_idx + 1);
                                                let badge_resp = Frame::new()
                                                    .fill(theme::SEL_FILL)
                                                    .stroke(Stroke::new(
                                                        theme::HAIRLINE,
                                                        theme::INK,
                                                    ))
                                                    .corner_radius(CornerRadius::same(0))
                                                    .inner_margin(Margin::symmetric(6, 3))
                                                    .show(ui, |ui| {
                                                        ui.add(
                                                            Label::new(
                                                                mono(&p_badge, 12.5)
                                                                    .strong()
                                                                    .color(theme::INK),
                                                            )
                                                            .selectable(false)
                                                            .sense(Sense::click()),
                                                        )
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                    })
                                                    .inner
                                                    .on_hover_text("MOVE TO NEXT PAGE");
                                                if badge_resp.clicked() {
                                                    move_next_page = Some(idx);
                                                    self.index_confirm_delete_pane_id = None;
                                                }

                                                // 5. Completion badge (`done/total`).
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
                                            },
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
        if let Some(idx) = delete_idx {
            self.delete_pane_at(idx, Some(&ctx));
        }
        if let Some(page) = open_page {
            self.current_page = page.min(self.total_pages().saturating_sub(1));
            self.show_master_index = false;
            self.focused_pane_id = None;
            self.index_confirm_delete_pane_id = None;
            self.index_editing_pane_id = None;
        }
    }

    fn render_harbor_placeholder(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            Frame::new()
                .fill(theme::PANE)
                .stroke(Stroke::new(theme::GRID_LINE, theme::GRID))
                .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
                .inner_margin(Margin::same(28))
                .show(ui, |ui| {
                    ui.label(mono("HARBOR", 20.0).strong());
                    ui.add_space(6.0);
                    ui.label(
                        mono("REPORTS & PERSISTENT ARCHIVE ARRIVE IN PHASE FOUR.", 11.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
        });
    }

    fn render_daily_canvas(&mut self, ui: &mut Ui) {
        let panes_count = self.data.current_day.panes.len();
        if panes_count == 0 {
            let (_, date) = clock_strings();
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                Frame::new()
                    .fill(theme::PANE)
                    .stroke(Stroke::new(theme::GRID_LINE, theme::GRID))
                    .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
                    .inner_margin(Margin::same(28))
                    .show(ui, |ui| {
                        ui.label(mono(&date, 18.0).strong());
                        ui.add_space(8.0);
                        ui.label(mono("NO TASKS YET", 11.0).color(theme::TEXT_MUTED));
                        ui.add_space(14.0);
                        if !self.is_zen
                            && pill_button(ui, "+ ADD TASKS", theme::GRID, true).clicked()
                        {
                            self.show_quick_entry_modal = true;
                            self.quick_entry_input.clear();
                            self.focus_quick_entry = true;
                        }
                    });
            });
            return;
        }

        let mut actions = PaneActions::default();
        let mut request_focus_id = self.next_focus_node_id.take();

        let mut finish_lasso = false;
        let viewport_height = ui.available_height();

        let focused_idx = self
            .focused_pane_id
            .as_ref()
            .and_then(|id| self.data.current_day.panes.iter().position(|p| &p.id == id));

        ScrollArea::vertical()
            .id_salt("canvas_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width();
                let focus_mode = focused_idx.is_some();
                let (placements, content_height) = if let Some(idx) = focused_idx {
                    // Focus mode: a single pane fills the whole canvas.
                    let pane = &self.data.current_day.panes[idx];
                    let height = (120.0 + pane.subtasks.len() as f32 * 46.0).max(viewport_height);
                    (
                        vec![TilePlacement {
                            pane_idx: idx,
                            pos: Pos2::ZERO,
                            size: Vec2::new(width, height),
                        }],
                        height,
                    )
                } else {
                    // Strictly paginated 4-pane-max page layout (2x2 grid).
                    let page_start = self.current_page * PANES_PER_PAGE;
                    let page_end =
                        (page_start + PANES_PER_PAGE).min(self.data.current_day.panes.len());
                    let page_indices: Vec<usize> = (page_start..page_end).collect();
                    let tiles = compute_page_grid_layout(&page_indices, width, viewport_height);
                    (tiles, viewport_height)
                };

                let canvas_height = content_height.max(viewport_height);
                let (canvas_rect, canvas_response) =
                    ui.allocate_exact_size(Vec2::new(width, canvas_height), Sense::drag());

                // The board itself is black; every pane paints its white sheet
                // on top, leaving sharp 2px black grid lines between panes.
                ui.painter()
                    .rect_filled(canvas_rect, CornerRadius::same(0), theme::GRID);

                if canvas_response.drag_started() && !self.is_lassoing {
                    if let Some(pos) = canvas_response.interact_pointer_pos() {
                        self.is_lassoing = true;
                        self.lasso_start = Some(pos);
                        self.lasso_current = Some(pos);
                    }
                }

                if self.is_lassoing {
                    if let Some(pos) = ctx_pointer_pos(ui) {
                        self.lasso_current = Some(pos);
                    }
                    if canvas_response.drag_stopped() || ui.input(|i| i.pointer.any_released()) {
                        finish_lasso = true;
                    }
                }

                let origin = canvas_rect.min;
                for tile in placements {
                    let pane_idx = tile.pane_idx;
                    let tile_rect = Rect::from_min_size(origin + tile.pos.to_vec2(), tile.size);
                    // Sharp charcoal grid dividers between the panes.
                    draw_grid_dividers(ui.painter(), tile_rect, width, canvas_height);
                    let pane_rect = tile_rect;
                    let pane = &mut self.data.current_day.panes[pane_idx];
                    let is_selected = self.selected_pane_ids.contains(&pane.id);
                    let input = self.pane_inputs.entry(pane.id.clone()).or_default();
                    let open_drawer_node_id = self.pane_note_drawers.get(&pane.id).cloned();
                    render_mind_map(
                        ui,
                        pane_rect,
                        pane,
                        pane_idx,
                        is_selected,
                        self.is_zen,
                        focus_mode,
                        input,
                        open_drawer_node_id.as_deref(),
                        &mut self.focus_notes_editor,
                        &mut self.focused_pane_id,
                        &mut self.focused_node_id,
                        &mut self.editing_node_id,
                        &mut request_focus_id,
                        &mut self.selected_node_ids,
                        &mut self.selected_pane_ids,
                        &mut self.scroll_seeded,
                        &mut actions,
                    );
                }

                if self.is_lassoing {
                    if let (Some(start), Some(curr)) = (self.lasso_start, self.lasso_current) {
                        let rect = Rect::from_two_pos(start, curr);
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(0),
                            Color32::from_black_alpha(30),
                        );
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(0),
                            Stroke::new(1.2, theme::GRID),
                            StrokeKind::Outside,
                        );

                        if finish_lasso {
                            self.apply_lasso(rect, &actions.node_rects, &actions.pane_header_rects);
                        }
                    }
                }
            });

        if actions.dirty {
            self.dirty = true;
        }

        if let Some(idx) = actions.delete_pane {
            self.delete_pane_at(idx, Some(ui.ctx()));
        }

        if let Some(target_id) = actions.add_sibling {
            self.add_sibling_after(&target_id);
        }
        if let Some(target_id) = actions.add_child {
            self.add_child_under(&target_id);
        }
        if let Some(target_id) = actions.outdent {
            self.outdent_node(&target_id);
        }
        if let Some(target_id) = actions.delete_node {
            self.delete_node(&target_id);
        }
        if let Some(node_id) = actions.focus_node {
            self.next_focus_node_id = Some(node_id);
        }
        if let Some(node_id) = actions.select_open_drawer {
            self.open_note_drawer(node_id, false);
        }
        if let Some(node_id) = actions.focus_open_drawer {
            self.open_note_drawer(node_id, true);
        }
        if let Some(pane_id) = actions.close_drawer_pane {
            self.close_note_drawer_for_pane(&pane_id, Some(ui.ctx()));
        }

        if finish_lasso {
            self.is_lassoing = false;
            self.lasso_start = None;
            self.lasso_current = None;
        }
    }

    /// Slim whiteboard header used in Zen mode: digital clock + English date
    /// over a 1px charcoal rule, nothing else.
    fn render_zen_header(&mut self, ui: &mut Ui) {
        let (time, _date) = clock_strings();
        let short_date = short_date_string();
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, 36.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                let rect = ui.max_rect();
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(0), theme::PANE);
                let rule = Rect::from_min_max(
                    pos2(rect.left(), rect.bottom() - theme::HAIRLINE),
                    pos2(rect.right(), rect.bottom()),
                );
                ui.painter()
                    .rect_filled(rule, CornerRadius::same(0), theme::GRID);

                ui.add_space(24.0);
                ui.label(mono("ZEN", 14.0).strong().color(theme::SLATE));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(24.0);
                    ui.label(mono(&time, 14.0).strong().color(theme::INK));
                    ui.label(mono(short_date, 12.5).color(theme::SLATE));
                });
            },
        );
        ui.add_space(6.0);
    }
}

#[derive(Default)]
struct PaneActions {
    delete_pane: Option<usize>,
    add_sibling: Option<String>,
    add_child: Option<String>,
    outdent: Option<String>,
    delete_node: Option<String>,
    /// Open the embedded note drawer on card click without stealing card focus.
    select_open_drawer: Option<String>,
    /// Open the embedded note drawer and immediately focus the notes editor.
    focus_open_drawer: Option<String>,
    /// Close the embedded note drawer for the specified pane ID.
    close_drawer_pane: Option<String>,
    node_rects: Vec<(String, Rect)>,
    pane_header_rects: Vec<(String, Rect)>,
    dirty: bool,
    /// Node that must grab keyboard focus on the next frame (carried over).
    focus_node: Option<String>,
}

/// Crisp `0.0px` icon button used for `↑` / `↓` row reordering in the Master Index.
fn index_icon_button(ui: &mut Ui, glyph: &str, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let resp = if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    };
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
    ui.painter().rect_filled(rect, CornerRadius::same(0), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(0),
        Stroke::new(theme::HAIRLINE, border),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::monospace(13.0),
        fg,
    );
    resp
}

/// Placement of a pane on the tiled canvas, in canvas-local coordinates.
#[derive(Debug, Clone, Copy)]
struct TilePlacement {
    pane_idx: usize,
    pos: Pos2,
    size: Vec2,
}

/// Paginated 4-pane-max grid layout (RTL order, strictly `<= 4` panes per page):
///
/// * `1` pane -> full width & height.
/// * `2` panes -> 2 side-by-side columns (`50% / 50%`), first on the right.
/// * `3` panes -> clean 2-column layout (master full height on the right,
///   two stacked on the left).
/// * `4` panes -> clean `2x2` grid (2 columns × 2 rows, RTL top-to-bottom).
fn compute_page_grid_layout(
    page_indices: &[usize],
    width: f32,
    height: f32,
) -> Vec<TilePlacement> {
    let n = page_indices.len().min(PANES_PER_PAGE);
    let mut out = Vec::with_capacity(n);
    match n {
        0 => {}
        1 => out.push(TilePlacement {
            pane_idx: page_indices[0],
            pos: Pos2::ZERO,
            size: vec2(width, height),
        }),
        2 => {
            let half_w = width * 0.5;
            out.push(TilePlacement {
                pane_idx: page_indices[0],
                pos: pos2(half_w, 0.0),
                size: vec2(half_w, height),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[1],
                pos: Pos2::ZERO,
                size: vec2(half_w, height),
            });
        }
        3 => {
            let half_w = width * 0.5;
            let half_h = height * 0.5;
            out.push(TilePlacement {
                pane_idx: page_indices[0],
                pos: pos2(half_w, 0.0),
                size: vec2(half_w, height),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[1],
                pos: Pos2::ZERO,
                size: vec2(half_w, half_h),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[2],
                pos: pos2(0.0, half_h),
                size: vec2(half_w, half_h),
            });
        }
        _ => {
            let half_w = width * 0.5;
            let half_h = height * 0.5;
            out.push(TilePlacement {
                pane_idx: page_indices[0],
                pos: pos2(half_w, 0.0),
                size: vec2(half_w, half_h),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[1],
                pos: Pos2::ZERO,
                size: vec2(half_w, half_h),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[2],
                pos: pos2(half_w, half_h),
                size: vec2(half_w, half_h),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[3],
                pos: pos2(0.0, half_h),
                size: vec2(half_w, half_h),
            });
        }
    }
    out
}

/// Draws the sharp charcoal grid dividers around a pane.
fn draw_grid_dividers(painter: &Painter, tile: Rect, board_w: f32, board_h: f32) {
    let stroke = Stroke::new(1.5, theme::GRID);
    if tile.left() > 0.5 {
        painter.line_segment(
            [
                pos2(tile.left(), tile.top()),
                pos2(tile.left(), tile.bottom()),
            ],
            stroke,
        );
    }
    if tile.right() < board_w - 0.5 {
        painter.line_segment(
            [
                pos2(tile.right(), tile.top()),
                pos2(tile.right(), tile.bottom()),
            ],
            stroke,
        );
    }
    if tile.top() > 0.5 {
        painter.line_segment(
            [
                pos2(tile.left(), tile.top()),
                pos2(tile.right(), tile.top()),
            ],
            stroke,
        );
    }
    if tile.bottom() < board_h - 0.5 {
        painter.line_segment(
            [
                pos2(tile.left(), tile.bottom()),
                pos2(tile.right(), tile.bottom()),
            ],
            stroke,
        );
    }
}

/// Draws one pane inside its slot on the tiled canvas, splitting the bottom
/// `38%` into an embedded inline note drawer when a branch note is open.
#[allow(clippy::too_many_arguments)]
fn render_mind_map(
    ui: &mut Ui,
    rect: Rect,
    pane: &mut MainTaskPane,
    pane_idx: usize,
    is_selected: bool,
    zen: bool,
    focus: bool,
    pane_input: &mut String,
    open_drawer_node_id: Option<&str>,
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

    let (top_rect, drawer_rect) = if valid_drawer_id.is_some() {
        let drawer_h = (rect.height() * 0.38)
            .max(96.0)
            .min((rect.height() - 120.0).max(96.0));
        let split_y = rect.bottom() - drawer_h;
        (
            Rect::from_min_max(rect.min, pos2(rect.right(), split_y)),
            Some(Rect::from_min_max(pos2(rect.left(), split_y), rect.max)),
        )
    } else {
        (rect, None)
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
        pane_input,
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
                focus_notes_editor,
                actions,
            );
        }
    }
}

/// Finds a mutable task node inside a slice of subtask trees.
fn find_node_in_slice_mut<'a>(
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

/// Embedded bottom split note drawer inside the active pane (`38%` height,
/// `1.5px` `#0F172A` top border, `#F8FAFC` subtle paper fill, `0.0px` corners).
fn render_pane_note_drawer(
    ui: &mut Ui,
    rect: Rect,
    pane_id: &str,
    node: &mut TaskNode,
    focus_notes_editor: &mut Option<String>,
    actions: &mut PaneActions,
) {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(0), theme::DRAWER_BG);
    // Crisp 1.5px #0F172A top border separating the drawer from the mind-map.
    painter.rect_filled(
        Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + 1.5)),
        CornerRadius::same(0),
        theme::INK,
    );

    let inner_rect = Rect::from_min_max(
        pos2(rect.left() + 16.0, rect.top() + 3.5),
        pos2(rect.right() - 16.0, rect.bottom() - 8.0),
    );
    let mut drawer_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(inner_rect)
            .layout(Layout::top_down(Align::Max)),
    );
    drawer_ui.set_clip_rect(inner_rect);

    let node_id = node.id.clone();

    // Header (32.0px): `"ملاحظات: <Branch Title>"` on the right, `✕` close button on the left.
    rtl_row(&mut drawer_ui, 32.0, |ui| {
        let title_width = (ui.available_width() - 34.0).max(40.0);
        let branch_title = if node.title.trim().is_empty() {
            "بدون عنوان"
        } else {
            node.title.trim()
        };
        let header_text = format!("ملاحظات: {}", branch_title);
        ui.add_sized(
            vec2(title_width, 28.0),
            Label::new(
                RichText::new(shape_arabic(&header_text))
                    .size(15.0)
                    .strong()
                    .color(theme::INK),
            )
            .selectable(false)
            .halign(Align::Max)
            .truncate(),
        );

        let (close_rect, close_response) =
            ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
        let close_response = close_response.on_hover_cursor(CursorIcon::PointingHand);
        let close_color = if close_response.hovered() {
            theme::INK
        } else {
            theme::INK_SOFT
        };
        ui.painter().text(
            close_rect.center(),
            egui::Align2::CENTER_CENTER,
            "✕",
            FontId::monospace(14.0),
            close_color,
        );
        if close_response.clicked() {
            actions.close_drawer_pane = Some(pane_id.to_string());
        }
        close_response.on_hover_text("CLOSE NOTES [Esc]");
    });

    // Subtle 1.0px #CBD5E1 rule between header and note editor body.
    let (div_rect, _) = drawer_ui.allocate_exact_size(
        Vec2::new(drawer_ui.available_width(), 1.0),
        Sense::hover(),
    );
    drawer_ui
        .painter()
        .rect_filled(div_rect, CornerRadius::same(0), theme::HINT);
    drawer_ui.add_space(4.0);

    // Body: spacious RTL-aligned multiline TextEdit (16.0px font) with instant auto-save.
    let body_h = drawer_ui.available_height().max(32.0);
    ScrollArea::vertical()
        .id_salt(("pane_drawer_scroll", pane_id, node_id.as_str()))
        .auto_shrink([false, false])
        .max_height(body_h)
        .show(&mut drawer_ui, |ui| {
            let margin = Margin::symmetric(6, 6);
            let mut layouter = rtl_multiline_layouter(16.0);
            let editor_output = TextEdit::multiline(&mut node.notes)
                .id(Id::new(("pane_notes_editor", node_id.as_str())))
                .font(FontId::proportional(16.0))
                .desired_width(f32::INFINITY)
                .desired_rows(4)
                .frame(false)
                .margin(margin)
                .background_color(theme::DRAWER_BG)
                .text_color(theme::INK)
                .horizontal_align(Align::RIGHT)
                .layouter(&mut layouter)
                .show(ui);

            if focus_notes_editor.as_deref() == Some(node_id.as_str()) {
                editor_output.response.request_focus();
                *focus_notes_editor = None;
            }
            if editor_output.response.changed() {
                actions.dirty = true;
            }
            if node.notes.is_empty() {
                let caret_clearance = if editor_output.response.has_focus() {
                    6.0
                } else {
                    0.0
                };
                let inner = editor_output.response.rect - margin;
                let anchor = pos2(inner.right() - caret_clearance, inner.top());
                let painter = ui.painter_at(editor_output.text_clip_rect.expand(1.0));
                painter.text(
                    anchor,
                    egui::Align2::RIGHT_TOP,
                    shape_arabic("اكتب ملاحظاتك هنا..."),
                    FontId::proportional(16.0),
                    theme::MUTED,
                );
            }
        });
}

/// Renders a single task pane: a clean title header plus the horizontal
/// right-to-left mind-map of its sub-tasks.
#[allow(clippy::too_many_arguments)]
fn render_pane(
    ui: &mut Ui,
    pane: &mut MainTaskPane,
    pane_idx: usize,
    is_selected: bool,
    zen: bool,
    focus: bool,
    pane_input: &mut String,
    focused_pane_id: &mut Option<String>,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    selected_pane_ids: &mut HashSet<String>,
    scroll_seeded: &mut HashSet<String>,
    actions: &mut PaneActions,
) {
    // Pane surfaces are always pure white; only black ink marks structure.
    let frame = Frame::new()
        .fill(theme::PANE)
        .stroke(if is_selected {
            Stroke::new(2.0, theme::GRID)
        } else {
            Stroke::NONE
        })
        .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
        .inner_margin(Margin::same(theme::PADDING));

    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.set_min_height(ui.available_height());

        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            let mut focus_toggle: Option<bool> = None;
            let (done, total) = count_pane_progress(&pane.subtasks);

            // Swiss architectural header: bold Arabic title flush right, a
            // monospace index + progress badge on the left.
            rtl_row(ui, 36.0, |ui| {
                let pane_id = pane.id.clone();
                let index = pane_idx + 1;
                let reserved = if zen { 132.0 } else { 160.0 };
                let title_width = (ui.available_width() - reserved).max(60.0);

                // Title first -> rightmost item in the RTL row (20.0px Bold #0F172A).
                let title_color = match pane.status {
                    TaskStatus::Active => theme::INK,
                    TaskStatus::Completed | TaskStatus::Cancelled => theme::SLATE,
                };
                let mut title_text = RichText::new(shape_arabic(&pane.title))
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
                            .halign(Align::Max)
                            .truncate(),
                    )
                    .on_hover_cursor(CursorIcon::PointingHand);
                actions
                    .pane_header_rects
                    .push((pane_id.clone(), title_response.rect));

                if title_response.double_clicked() {
                    focus_toggle = Some(true);
                } else if title_response.clicked() {
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

                // Right-click context menu on the main task.
                title_response.context_menu(|ui| {
                    if ui
                        .button("DELETE TASK")
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        actions.delete_pane = Some(pane_idx);
                    }
                });

                if !zen {
                    // Hover-only delete affordance (no permanent clutter).
                    let (slot_rect, slot_response) =
                        ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
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

                // Monospace index + progress badge, leftmost in the header (14.0px Bold #334155).
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
                // Bold impression at 14.0px #334155 (darker on hover).
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
                if badge_response.clicked() {
                    focus_toggle = Some(focused_pane_id.as_deref() != Some(pane_id.as_str()));
                }
                badge_response
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text("CLICK OR ALT+N: FOCUS");

                title_response.on_hover_text("DOUBLE-CLICK: MAXIMIZE · CTRL+CLICK: SELECT");
            });

            if let Some(toggle) = focus_toggle {
                *focused_pane_id = if toggle { Some(pane.id.clone()) } else { None };
            }

            ui.add_space(10.0);
            // Header-embedded 2.0px progress hairline: right-to-left Emerald
            // Green (#10B981) proportional to completion ratio, with the
            // remaining left segment in #CBD5E1.
            let (rule_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 2.0), Sense::hover());
            let ratio = if total == 0 {
                0.0
            } else {
                (done as f32 / total as f32).clamp(0.0, 1.0)
            };
            let done_w = rule_rect.width() * ratio;
            let split_x = rule_rect.right() - done_w;
            if split_x > rule_rect.left() {
                ui.painter().rect_filled(
                    Rect::from_min_max(rule_rect.left_top(), pos2(split_x, rule_rect.bottom())),
                    CornerRadius::same(0),
                    theme::HINT,
                );
            }
            if done_w > 0.0 {
                ui.painter().rect_filled(
                    Rect::from_min_max(pos2(split_x, rule_rect.top()), rule_rect.right_bottom()),
                    CornerRadius::same(0),
                    theme::GREEN_SHADOW,
                );
            }
            ui.add_space(10.0);

            // Horizontal right-to-left mind-map of the sub-tasks.
            let chrome = if zen { 0.0 } else { 40.0 };
            let scroll_height = (ui.available_height() - chrome).max(40.0);

            let avail_w = ui.available_width();
            let base_metrics = mind_map_metrics(focus, avail_w);
            let editing_snap = editing_node_id.clone();

            // Automatic fit-to-pane zoom: shrink the whole tree (font, card
            // padding, column and row gaps) until it fits the pane, so no
            // scrolling is needed.
            let mut metrics = base_metrics;
            for _ in 0..3 {
                let req_h = (stack_height(ui, &pane.subtasks, &metrics, 0, editing_snap.as_deref())
                    + SHADOW_PAD)
                    .max(metrics.min_card_h + SHADOW_PAD);
                let req_w =
                    stack_width(ui, &pane.subtasks, &metrics, 0, editing_snap.as_deref())
                        + SHADOW_PAD;
                let scale = fit_scale(req_h, req_w, scroll_height, avail_w);
                if (scale - 1.0).abs() < 0.005 {
                    metrics = base_metrics;
                    break;
                }
                metrics = base_metrics.scaled(scale);
            }

            let content_w =
                stack_width(ui, &pane.subtasks, &metrics, 0, editing_snap.as_deref()) + SHADOW_PAD;
            let overflow = (content_w - avail_w).max(0.0);
            let seeded = scroll_seeded.contains(&pane.id);

            let mut area = ScrollArea::both()
                .id_salt(("pane_scroll", pane.id.as_str()))
                .auto_shrink([false, false])
                .max_height(scroll_height);
            if overflow > 0.5 && !seeded {
                // Anchor the first view on the root column (right edge).
                area = area.horizontal_scroll_offset(overflow);
            }
            area.show(ui, |ui| {
                let total_w = avail_w.max(content_w);
                ui.set_min_width(total_w);
                // Inside a ScrollArea the content origin moves with the scroll
                // offset, so cards must be placed relative to it.
                let origin = ui.max_rect().min;
                let origin_x = origin.x;
                let origin_y = origin.y;
                let content_h = (stack_height(
                    ui,
                    &pane.subtasks,
                    &metrics,
                    0,
                    editing_snap.as_deref(),
                ) + SHADOW_PAD)
                    .max(metrics.min_card_h + SHADOW_PAD);
                ui.set_min_height(content_h);

                let root_right = total_w - 2.0;
                let mut hover_controls = Vec::new();
                render_mind_map_nodes(
                    ui,
                    &mut pane.subtasks,
                    0,
                    (content_h - 3.0) * 0.5,
                    root_right,
                    origin_x,
                    origin_y,
                    &metrics,
                    zen,
                    focused_node_id,
                    editing_node_id,
                    request_focus_id,
                    selected_node_ids,
                    &mut hover_controls,
                    actions,
                );
                // Micro-controls are painted last so the ink threads never
                // run across them.
                paint_hover_controls(ui.painter(), &hover_controls);
            });
            if overflow > 0.5 && !seeded {
                scroll_seeded.insert(pane.id.clone());
            }
            ui.add_space(4.0);

            if !zen {
                render_pane_branch_input(ui, &pane.id, pane_input, &mut pane.subtasks, actions);
            }
        });
    });
}

/// Layouter that shapes Arabic but keeps the logical string untouched, so
/// `TextEdit` cursor indices remain character-aligned with the original text.
fn rtl_layouter(
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
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    }
}

/// Multiline RTL layouter for the architectural notes modal: shapes Arabic
/// per line and right-aligns wrapped lines while keeping logical char counts.
fn rtl_multiline_layouter(
    font_size: f32,
) -> impl FnMut(&Ui, &dyn egui::TextBuffer, f32) -> Arc<Galley> {
    move |ui, text, wrap_width| {
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = wrap_width;
        job.halign = Align::RIGHT;
        job.append(
            &shape_arabic(text.as_str()),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(font_size),
                color: theme::INK,
                ..Default::default()
            },
        );
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    }
}

/// A bounded-height right-to-left row.
///
/// Using `ui.with_layout(Layout::right_to_left(...))` directly inside a
/// vertical layout makes the row's cross-axis span the full available height
/// (the elements get vertically centered), which can swallow the whole canvas.
/// Giving the child UI an explicit height keeps rows content-sized.
fn rtl_row<R>(
    ui: &mut Ui,
    height: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let size = Vec2::new(ui.available_width(), height);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), add_contents)
}

/// Pointer position helper used by the lasso selection.
fn ctx_pointer_pos(ui: &Ui) -> Option<Pos2> {
    ui.input(|i| i.pointer.hover_pos())
}

/// A minimal text window control (minimize / maximize / close).
fn window_control(ui: &mut Ui, glyph: &str, tip: &str) -> egui::Response {
    let response = ui
        .add(
            Label::new(mono(glyph, 14.0).strong().color(theme::INK_SOFT))
                .selectable(false)
                .sense(Sense::click()),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            FontId::monospace(14.0),
            theme::INK,
        );
    }
    response.on_hover_text(tip)
}

/// A text-only link used by the ultra-minimalist top bar (`14.0px` SemiBold).
fn text_link(ui: &mut Ui, text: &str, active: bool) -> egui::Response {
    let color = if active { theme::INK } else { theme::SLATE };
    let response = ui
        .add(
            Label::new(mono(text, 14.0).strong().color(color))
                .selectable(false)
                .sense(Sense::click()),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    if !active && response.hovered() {
        ui.painter().text(
            response.rect.left_center(),
            egui::Align2::LEFT_CENTER,
            text,
            FontId::monospace(14.0),
            theme::INK,
        );
    }
    response
}

fn pill_button(ui: &mut Ui, text: &str, color: Color32, filled: bool) -> egui::Response {
    let (fill, fg) = if filled {
        (color, theme::PANE)
    } else {
        (Color32::TRANSPARENT, color)
    };
    let stroke = if filled {
        Stroke::NONE
    } else {
        Stroke::new(theme::HAIRLINE, color)
    };
    Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
        .inner_margin(Margin::symmetric(14, 7))
        .show(ui, |ui| {
            ui.add(
                Label::new(mono(text, 13.0).strong().color(fg))
                    .selectable(false)
                    .sense(Sense::click()),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
        })
        .inner
}

/// A borderless metric: `2 DONE` in `12.5px` monospace, the number in status ink.
fn metric(ui: &mut Ui, value: usize, color: Color32, label: &str) {
    ui.horizontal(|ui| {
        ui.label(mono(value.to_string(), 12.5).strong().color(color));
        ui.label(mono(label, 12.5).color(theme::INK_SOFT));
    });
}

/// Maps a logical character index inside an RTL Arabic string to its visual
/// character index in the shaped (visually reversed) galley.
///
/// For Arabic text of length `len`, typing at the end (`logical_idx == len`)
/// maps to `visual_idx == 0` — the **left** (leading) edge of the text.
fn rtl_visual_cursor_index(text: &str, logical_idx: usize) -> usize {
    let char_count = text.chars().count();
    if crate::rtl_text::contains_arabic(text) {
        char_count.saturating_sub(logical_idx)
    } else {
        logical_idx.min(char_count)
    }
}

/// Single-line frameless `TextEdit` with right-to-left Arabic alignment,
/// left-leading RTL caret placement, and non-overlapping shaped placeholder.
#[allow(clippy::too_many_arguments)]
fn rtl_text_edit(
    ui: &mut Ui,
    id: Id,
    text: &mut String,
    hint: &str,
    font_size: f32,
    desired_width: f32,
    margin: Margin,
    text_color: Color32,
) -> egui::text_edit::TextEditOutput {
    let is_arabic = !text.is_empty() && crate::rtl_text::contains_arabic(text);
    let prev_cursor = ui.visuals().text_cursor.clone();
    if is_arabic {
        // Suppress egui's default LTR caret so we can paint the RTL caret at
        // the true visual leading edge (left of Arabic text).
        ui.visuals_mut().text_cursor.stroke = Stroke::NONE;
        ui.visuals_mut().text_cursor.preview = false;
    }

    let font = FontId::proportional(font_size);
    let mut layouter = rtl_layouter(font_size, false);
    let output = TextEdit::singleline(text)
        .id(id)
        .font(font.clone())
        .desired_width(desired_width)
        .frame(false)
        .margin(margin)
        .background_color(Color32::TRANSPARENT)
        .text_color(text_color)
        .horizontal_align(Align::RIGHT)
        .layouter(&mut layouter)
        .show(ui);

    if is_arabic {
        ui.visuals_mut().text_cursor = prev_cursor.clone();
    }

    let has_focus = output.response.has_focus();
    let viewport_focused = ui.ctx().input(|i| i.focused);

    if text.is_empty() && !hint.is_empty() {
        // Paint the shaped RTL placeholder cleanly aligned to the right edge,
        // shifted left by 6px while focused so the right-edge caret never
        // overlaps the placeholder characters.
        let caret_clearance = if has_focus { 6.0 } else { 0.0 };
        let inner = output.response.rect - margin;
        let anchor = pos2(inner.right() - caret_clearance, inner.center().y);
        let painter = ui.painter_at(output.text_clip_rect.expand(1.0));
        painter.text(
            anchor,
            egui::Align2::RIGHT_CENTER,
            shape_arabic(hint),
            font,
            theme::MUTED,
        );
    } else if is_arabic && has_focus && viewport_focused {
        let logical_idx = output
            .cursor_range
            .map(|r| r.primary.index)
            .unwrap_or_else(|| text.chars().count());
        let visual_idx = rtl_visual_cursor_index(text, logical_idx);
        let local = output
            .galley
            .pos_from_cursor(egui::text::CCursor::new(visual_idx));
        let caret_x = output.galley_pos.x + local.min.x;
        let caret_top = output.galley_pos.y + local.min.y;
        let caret_bottom = output.galley_pos.y + local.max.y;

        let now = ui.ctx().input(|i| i.time);
        let reset_blink = output.response.changed() || output.response.gained_focus();
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
/// branch, clears the field, and retains keyboard focus for rapid entry.
fn render_pane_branch_input(
    ui: &mut Ui,
    pane_id: &str,
    input: &mut String,
    subtasks: &mut Vec<TaskNode>,
    actions: &mut PaneActions,
) {
    let response = rtl_text_edit(
        ui,
        Id::new(("pane_branch_input", pane_id)),
        input,
        "+ اكتب فرعاً جديداً...",
        14.5,
        f32::INFINITY,
        Margin::symmetric(6, 5),
        theme::INK,
    )
    .response;

    let submitted = (response.has_focus() || response.lost_focus())
        && ui.input(|i| i.key_pressed(Key::Enter));
    if submitted {
        let trimmed = input.trim().to_string();
        if !trimmed.is_empty() {
            subtasks.push(TaskNode::new(&trimmed));
            input.clear();
            actions.dirty = true;
            response.request_focus();
        }
    }
}

/// Live digital clock strings: `HH:MM:SS AM/PM` plus an English date such as
/// `SUN, NOV 15, 2026`.
fn clock_strings() -> (String, String) {
    let now = chrono::Local::now();
    let time = now.format("%I:%M:%S %p").to_string();
    let date = now.format("%a, %b %d, %Y").to_string().to_uppercase();
    (time, date)
}

/// Compact English date for the top bar clock, e.g. `SUN, NOV 15`.
fn short_date_string() -> String {
    chrono::Local::now()
        .format("%a, %b %d")
        .to_string()
        .to_uppercase()
}

/// Monospace technical text used for all UI chrome.
fn mono(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::monospace(size))
}

/// Hard architectural offset shadow displacement (`dx: -3.0, dy: 3.0`).
const SHADOW_OFFSET: Vec2 = Vec2::new(-3.0, 3.0);
/// Padding reserved around the mind-map canvas so hard shadows never clip.
const SHADOW_PAD: f32 = 6.0;

/// Metrics driving the horizontal right-to-left mind-map layout.
#[derive(Clone, Copy)]
struct MindMapMetrics {
    /// Maximum width of a task card (42% of the pane width).
    max_card_w: f32,
    /// Horizontal gap between parent and child cards (Bezier curves span this).
    gap_x: f32,
    /// Minimum height of one task card.
    min_card_h: f32,
    /// Vertical gap between two sibling branches (8.0px base).
    gap_y: f32,
    /// Font size of a level-0 task (15.5px).
    font_size: f32,
    /// Font size of a nested branch (15.5px).
    font_size_deep: f32,
    /// Horizontal padding inside a card (14.0px base).
    pad_x: f32,
    /// Vertical padding inside a card (8.0px base).
    pad_y: f32,
}

impl MindMapMetrics {
    /// Scales every metric proportionally (used by the fit-to-pane zoom).
    fn scaled(self, scale: f32) -> Self {
        Self {
            max_card_w: (self.max_card_w * scale).max(64.0),
            gap_x: (self.gap_x * scale).max(24.0),
            min_card_h: self.min_card_h * scale,
            gap_y: (self.gap_y * scale).max(5.0),
            font_size: (self.font_size * scale).clamp(10.0, 16.0),
            font_size_deep: (self.font_size_deep * scale).clamp(10.0, 16.0),
            pad_x: (self.pad_x * scale).max(7.0),
            pad_y: (self.pad_y * scale).max(4.0),
        }
    }
}

/// Zoom factor that makes a tree of the given size fit into the pane.
/// Never scales below 55% and never scales up.
fn fit_scale(req_h: f32, req_w: f32, avail_h: f32, avail_w: f32) -> f32 {
    let mut scale = 1.0_f32;
    if req_h > avail_h && req_h > 0.0 {
        scale = scale.min(avail_h / req_h);
    }
    if req_w > avail_w && req_w > 0.0 {
        scale = scale.min(avail_w / req_w);
    }
    scale.clamp(0.55, 1.0)
}

/// Computes the base mind-map metrics for a pane of the given width.
fn mind_map_metrics(focus: bool, pane_w: f32) -> MindMapMetrics {
    let font_size = 15.5;
    let font_size_deep = 15.5;
    let pad_x = 14.0;
    let pad_y = 8.0;

    MindMapMetrics {
        max_card_w: (pane_w * 0.42).max(80.0),
        gap_x: (pane_w * if focus { 0.08 } else { 0.10 }).clamp(36.0, 68.0),
        min_card_h: font_size * 1.35 + pad_y * 2.0,
        gap_y: 8.0,
        font_size,
        font_size_deep,
        pad_x,
        pad_y,
    }
}

/// The font size a task uses at the given tree depth.
fn font_for_depth(m: &MindMapMetrics, depth: usize) -> f32 {
    if depth == 0 {
        m.font_size
    } else {
        m.font_size_deep
    }
}

/// The ink color an active task/branch card uses (`#0F172A` high contrast).
fn ink_for_depth(_depth: usize) -> Color32 {
    theme::INK
}

/// Measures the dynamic `(width, height)` of a task/branch card to fit its
/// actual Arabic/English text content (`12px` horizontal padding, `6px`
/// vertical padding, wrapping cleanly if wider than `42%` of the pane).
fn measure_card_size(
    ui: &Ui,
    node: &TaskNode,
    m: &MindMapMetrics,
    depth: usize,
    is_editing: bool,
) -> Vec2 {
    let font_size = font_for_depth(m, depth);
    let max_text_w = (m.max_card_w - m.pad_x * 2.0).max(40.0);
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

    let mut job = egui::text::LayoutJob::default();
    if !is_editing {
        job.wrap.max_width = max_text_w;
    }
    job.append(
        &shape_arabic(sample),
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(font_size),
            ..Default::default()
        },
    );
    let text_size = ui.fonts_mut(|fonts| fonts.layout_job(job).size());

    // Small buffer (+4px, or +14px while editing for the caret) prevents
    // sub-pixel rounding from triggering an unintended extra line wrap.
    let extra_w = if is_editing { 14.0 } else { 4.0 };
    let inner_w = (text_size.x.ceil() + extra_w).clamp(32.0, max_text_w);
    let inner_h = text_size.y.max(line_h);

    vec2(
        (inner_w + m.pad_x * 2.0).min(m.max_card_w),
        (inner_h + m.pad_y * 2.0).max(m.min_card_h),
    )
}

/// Computes the crisp 0.0px `[+N]` folded-branch badge size (`13.0px` monospace).
fn collapsed_badge_size(ui: &Ui, hidden_count: usize) -> Vec2 {
    let text = format!("[+{}]", hidden_count);
    let galley = ui
        .painter()
        .layout_no_wrap(text, FontId::monospace(13.0), theme::PANE);
    galley.size() + vec2(8.0, 4.0)
}

/// Horizontal width a single node (with its whole branch) occupies.
fn subtree_width(
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
        let badge_w = collapsed_badge_size(ui, node.descendant_count()).x;
        return own_w + badge_w + 4.0;
    }
    own_w + m.gap_x + stack_width(ui, &node.children, m, depth + 1, editing_id)
}

/// Horizontal width a stack of siblings (with their branches) occupies.
fn stack_width(
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
fn subtree_height(
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
fn stack_height(
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
fn stack_centers(
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
struct NodeAnchor {
    /// Exact screen-space bounding rect of the card.
    rect: Rect,
    /// True while the pointer hovers this card or one of its descendants.
    hot: bool,
}

/// Hover micro-controls that are painted after the ink threads.
#[derive(Clone, Copy)]
struct HoverControl {
    /// The card the controls belong to.
    card_rect: Rect,
    /// `+` button (adds a nested branch).
    plus_rect: Rect,
    /// `×` button (deletes the card).
    del_rect: Rect,
    /// Whether to draw the left-edge anchor dot (suppressed when folded).
    show_anchor_dot: bool,
}

/// Renders one column of the horizontal mind-map and returns the anchors of
/// the cards it produced, so the caller can ink curves into them.
#[allow(clippy::too_many_arguments)]
fn render_mind_map_nodes(
    ui: &mut Ui,
    nodes: &mut [TaskNode],
    depth: usize,
    block_center_y: f32,
    col_right: f32,
    origin_x: f32,
    origin_y: f32,
    m: &MindMapMetrics,
    zen: bool,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    hover_controls: &mut Vec<HoverControl>,
    actions: &mut PaneActions,
) -> Vec<NodeAnchor> {
    let mut cards = Vec::new();
    if nodes.is_empty() {
        return cards;
    }

    let editing_snap = editing_node_id.clone();
    let centers = stack_centers(
        ui,
        nodes,
        m,
        depth,
        block_center_y,
        editing_snap.as_deref(),
    );

    for (node, center) in nodes.iter_mut().zip(centers) {
        // Cards grow to the LEFT from `col_right`, dynamically sized to their text.
        let is_editing = editing_node_id.as_deref() == Some(node.id.as_str());
        let card_size = measure_card_size(ui, node, m, depth, is_editing);
        let rect = Rect::from_min_max(
            pos2(
                origin_x + col_right - card_size.x,
                origin_y + center - card_size.y * 0.5,
            ),
            pos2(origin_x + col_right, origin_y + center + card_size.y * 0.5),
        );

        let mut anchor = render_node_card(
            ui,
            node,
            rect,
            depth,
            m,
            zen,
            focused_node_id,
            editing_node_id,
            request_focus_id,
            selected_node_ids,
            hover_controls,
            actions,
        );

        if !node.is_folded() && !node.children.is_empty() {
            let child_col_right = col_right - card_size.x - m.gap_x;
            let child_cards = render_mind_map_nodes(
                ui,
                &mut node.children,
                depth + 1,
                center,
                child_col_right,
                origin_x,
                origin_y,
                m,
                zen,
                focused_node_id,
                editing_node_id,
                request_focus_id,
                selected_node_ids,
                hover_controls,
                actions,
            );
            // Hovering anywhere in the branch lights up the whole branch.
            if child_cards.iter().any(|child| child.hot) {
                anchor.hot = true;
            }
            draw_mind_map_threads(ui, anchor, &child_cards);
        }

        cards.push(anchor);
    }

    cards
}

/// Draws a Swiss minimalist task card: a solid 3.0px hard offset shadow
/// (`dx: -3.0, dy: 3.0`) encoding tri-state status behind a 0.0px-rounded pure
/// white card with a crisp 1px `#1E293B` border and dynamically wrapped text.
#[allow(clippy::too_many_arguments)]
fn render_node_card(
    ui: &mut Ui,
    node: &mut TaskNode,
    rect: Rect,
    depth: usize,
    m: &MindMapMetrics,
    zen: bool,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    hover_controls: &mut Vec<HoverControl>,
    actions: &mut PaneActions,
) -> NodeAnchor {
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

    let badge_rect = if is_folded {
        let size = collapsed_badge_size(ui, hidden_count);
        Some(Rect::from_min_max(
            pos2(rect.left() - size.x, rect.center().y - size.y * 0.5),
            pos2(rect.left(), rect.center().y + size.y * 0.5),
        ))
    } else {
        None
    };

    // Floating hover controls positioned just outside the card's left edge
    // (or left of the `[+N]` folded badge) so hovering never reflows text.
    let controls_right = badge_rect.map_or(rect.left(), |b| b.left());
    let plus_rect = Rect::from_min_size(
        pos2(controls_right - 19.0, rect.center().y - 8.0),
        vec2(16.0, 16.0),
    );
    let del_rect = Rect::from_min_size(
        pos2(controls_right - 37.0, rect.center().y - 8.0),
        vec2(16.0, 16.0),
    );
    let mut hover_zone = if !zen && !is_editing {
        rect.union(plus_rect).union(del_rect).expand(2.0)
    } else {
        rect
    };
    if let Some(b_rect) = badge_rect {
        hover_zone = hover_zone.union(b_rect);
    }
    let hovering = ui.input(|i| {
        i.pointer
            .hover_pos()
            .is_some_and(|p| hover_zone.contains(p))
    });

    // 1. Solid 3.0px hard offset shadow behind the card (Active / Completed / Cancelled).
    let painter = ui.painter();
    painter.rect_filled(
        rect.translate(SHADOW_OFFSET),
        CornerRadius::same(theme::CARD_RADIUS),
        theme::status_shadow_color(node.status),
    );

    // 2. 0.0px-rounded white card with 1px #1E293B border (heavier when selected).
    painter.rect_filled(rect, CornerRadius::same(theme::CARD_RADIUS), theme::PANE);
    let (border_width, border_color) = if is_selected {
        (2.0, theme::INK)
    } else {
        (theme::HAIRLINE, theme::GRID)
    };
    painter.rect_stroke(
        rect,
        CornerRadius::same(theme::CARD_RADIUS),
        Stroke::new(border_width, border_color),
        StrokeKind::Inside,
    );

    // 3. Corner ink dog-ear indicator (8.0px × 8.0px solid #0F172A right-triangle)
    // flush inside the top-left corner whenever the node has non-empty notes.
    if has_notes {
        let tl = rect.left_top();
        painter.add(Shape::convex_polygon(
            vec![tl, pos2(tl.x + 8.0, tl.y), pos2(tl.x, tl.y + 8.0)],
            theme::INK,
            Stroke::NONE,
        ));
    }

    // 4. Folded branch badge `[+N]` flush on the left edge of the parent card.
    if let Some(b_rect) = badge_rect {
        let badge_text = format!("[+{}]", hidden_count);
        let font = FontId::monospace(13.0);
        painter.rect_filled(b_rect, CornerRadius::same(0), theme::INK);
        painter.text(
            b_rect.center(),
            egui::Align2::CENTER_CENTER,
            &badge_text,
            font.clone(),
            theme::PANE,
        );
        painter.text(
            b_rect.center() + vec2(0.5, 0.0),
            egui::Align2::CENTER_CENTER,
            &badge_text,
            font,
            theme::PANE,
        );
    }

    let content_rect = Rect::from_min_max(
        pos2(rect.left() + m.pad_x, rect.top() + m.pad_y),
        pos2(rect.right() - m.pad_x, rect.bottom() - m.pad_y),
    );

    if is_editing {
        let mut card_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Max)),
        );
        card_ui.set_clip_rect(content_rect);

        let response = rtl_text_edit(
            &mut card_ui,
            Id::new(&node_id),
            &mut node.title,
            "عنوان المهمة...",
            font_size,
            content_rect.width().max(32.0),
            Margin::same(0),
            theme::INK,
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
        // NOTE: raw key state instead of `response.has_focus()`: egui's Tab
        // focus navigation can steal focus before this branch runs.
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
                // An empty new branch is discarded instead of kept.
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

        // Top-left 16×16 dog-ear hit-zone opens the architectural notes modal.
        let dog_ear_rect = Rect::from_min_size(rect.left_top(), vec2(16.0, 16.0));
        let dog_ear_response = ui
            .interact(
                dog_ear_rect,
                ui.id().with(("dog_ear", &node_id)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);

        if let Some(b_rect) = badge_rect {
            let badge_response = ui
                .interact(b_rect, ui.id().with(("fold_badge", &node_id)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            if badge_response.clicked() {
                node.collapsed = false;
                node.is_expanded = true;
                actions.dirty = true;
            }
            badge_response.on_hover_text("CLICK: EXPAND BRANCH");
        }

        let text_color = match node.status {
            TaskStatus::Active => ink_for_depth(depth),
            TaskStatus::Completed | TaskStatus::Cancelled => theme::SLATE,
        };
        let mut label = RichText::new(shape_arabic(&node.title))
            .size(font_size)
            .color(text_color);
        if node.status == TaskStatus::Cancelled {
            label = label.strikethrough();
        }

        let mut card_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Max)),
        );
        card_ui.set_clip_rect(content_rect);
        card_ui.add(
            Label::new(label)
                .selectable(false)
                .halign(Align::Max)
                .wrap(),
        );

        if dog_ear_response.clicked() {
            selected_node_ids.clear();
            selected_node_ids.insert(node_id.clone());
            *focused_node_id = Some(node_id.clone());
            actions.focus_open_drawer = Some(node_id.clone());
        } else if card_response.double_clicked() {
            selected_node_ids.clear();
            selected_node_ids.insert(node_id.clone());
            *focused_node_id = Some(node_id.clone());
            if has_children {
                node.collapsed = !node.collapsed;
                node.is_expanded = !node.collapsed;
                actions.dirty = true;
            } else {
                *editing_node_id = Some(node_id.clone());
                *request_focus_id = Some(node_id.clone());
                actions.focus_node = Some(node_id.clone());
            }
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

        // Right-click context menu on sub-tasks (also reachable from the dog-ear zone).
        card_response.context_menu(|ui| {
            render_card_context_menu(
                ui,
                node,
                &node_id,
                focused_node_id,
                editing_node_id,
                request_focus_id,
                actions,
            );
        });
        dog_ear_response.context_menu(|ui| {
            render_card_context_menu(
                ui,
                node,
                &node_id,
                focused_node_id,
                editing_node_id,
                request_focus_id,
                actions,
            );
        });

        if has_notes {
            dog_ear_response.on_hover_text(shape_arabic("الملاحظات / Notes"));
        }

        let dbl_hint = if has_children {
            "DOUBLE-CLICK: FOLD/UNFOLD"
        } else {
            "DOUBLE-CLICK: EDIT"
        };
        card_response.on_hover_text(format!(
            "{} · CLICK: SELECT · {} · N: NOTES · 1/2/3 OR SPACE: STATUS",
            node.status.label_en().to_uppercase(),
            dbl_hint,
        ));
    }

    if !zen && !is_editing && hovering {
        let plus_response = ui
            .interact(plus_rect, ui.id().with(("plus", &node_id)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let del_response = ui
            .interact(del_rect, ui.id().with(("del", &node_id)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if plus_response.clicked() {
            actions.add_child = Some(node_id.clone());
        }
        if del_response.clicked() {
            actions.delete_node = Some(node_id.clone());
        }
        hover_controls.push(HoverControl {
            card_rect: rect,
            plus_rect,
            del_rect,
            show_anchor_dot: !is_folded,
        });
    }

    actions.node_rects.push((node_id, rect));

    NodeAnchor {
        rect,
        hot: hovering,
    }
}

/// Shared right-click context menu for a subtask card.
fn render_card_context_menu(
    ui: &mut Ui,
    node: &mut TaskNode,
    node_id: &str,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    actions: &mut PaneActions,
) {
    if ui
        .button("ACTIVE [1]")
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        node.status = TaskStatus::Active;
        actions.dirty = true;
    }
    if ui
        .button("COMPLETED [2]")
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        node.status = TaskStatus::Completed;
        actions.dirty = true;
    }
    if ui
        .button("CANCELLED [3]")
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        node.status = TaskStatus::Cancelled;
        actions.dirty = true;
    }
    ui.separator();
    if ui
        .button(shape_arabic("الملاحظات / Notes"))
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.focus_open_drawer = Some(node_id.to_string());
    }
    if ui
        .button("EDIT")
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        *editing_node_id = Some(node_id.to_string());
        *focused_node_id = Some(node_id.to_string());
        *request_focus_id = Some(node_id.to_string());
        actions.focus_node = Some(node_id.to_string());
    }
    if ui
        .button("ADD BRANCH")
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.add_child = Some(node_id.to_string());
    }
    if ui
        .button("DELETE TASK")
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.delete_node = Some(node_id.to_string());
    }
}

/// Draws smooth cubic Bezier ink curves (`1.4px` `#334155`) from
/// `parent_rect.left_center()` to `child_rect.right_center()` with a crisp
/// left-pointing arrowhead.
fn draw_mind_map_threads(ui: &Ui, parent: NodeAnchor, children: &[NodeAnchor]) {
    let painter = ui.painter();

    for child in children {
        let hot = parent.hot || child.hot;
        let color = if hot { theme::INK } else { theme::INK_SOFT };
        let stroke = Stroke::new(if hot { 1.75 } else { 1.4 }, color);

        let p0 = parent.rect.left_center();
        let p3 = child.rect.right_center();
        let dx = (p0.x - p3.x).abs().max(1.0);

        let curve = CubicBezierShape {
            points: [
                p0,
                pos2(p0.x - dx * 0.45, p0.y),
                pos2(p3.x + dx * 0.45, p3.y),
                p3,
            ],
            closed: false,
            fill: Color32::TRANSPARENT,
            stroke: stroke.into(),
        };
        painter.add(curve);

        // Crisp solid left-pointing arrowhead meeting child_rect.right_center().
        let tip = p3;
        let base = p3.x + 6.5;
        painter.add(Shape::convex_polygon(
            vec![tip, pos2(base, p3.y - 3.2), pos2(base, p3.y + 3.2)],
            color,
            Stroke::NONE,
        ));
    }
}

/// Paints the hover micro-controls: an ink anchor dot on the card's left edge
/// plus the `+` / `×` buttons.
fn paint_hover_controls(painter: &Painter, controls: &[HoverControl]) {
    for control in controls {
        // White knockout so the ink threads never cross the glyphs.
        let buttons = control.plus_rect.union(control.del_rect).expand(2.0);
        painter.rect_filled(buttons, CornerRadius::same(0), theme::PANE);

        // Ink anchor dot marking the branch origin (omitted when folded behind [+N]).
        if control.show_anchor_dot {
            painter.circle_filled(control.card_rect.left_center(), 2.5, theme::GRID);
        }

        painter.text(
            control.plus_rect.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            FontId::monospace(13.0),
            theme::INK,
        );
        painter.text(
            control.del_rect.center(),
            egui::Align2::CENTER_CENTER,
            "\u{d7}",
            FontId::monospace(13.0),
            theme::INK,
        );
    }
}

/// Counts `(completed, total)` task nodes in a pane, including nested branches.
fn count_pane_progress(nodes: &[TaskNode]) -> (usize, usize) {
    let mut done = 0;
    let mut total = 0;
    for node in nodes {
        total += 1;
        if node.status == TaskStatus::Completed {
            done += 1;
        }
        let (child_done, child_total) = count_pane_progress(&node.children);
        done += child_done;
        total += child_total;
    }
    (done, total)
}

fn set_status_in_nodes(nodes: &mut [TaskNode], target_ids: &HashSet<String>, status: TaskStatus) {
    for node in nodes {
        if target_ids.contains(&node.id) {
            node.status = status;
        }
        set_status_in_nodes(&mut node.children, target_ids, status);
    }
}

fn cycle_status_in_nodes(nodes: &mut [TaskNode], target_ids: &HashSet<String>) {
    for node in nodes {
        if target_ids.contains(&node.id) {
            node.status = node.status.next();
        }
        cycle_status_in_nodes(&mut node.children, target_ids);
    }
}

fn insert_child_recursive(nodes: &mut [TaskNode], parent_id: &str, new_node: TaskNode) -> bool {
    for node in nodes {
        if node.id == parent_id {
            node.children.push(new_node);
            node.is_expanded = true;
            node.collapsed = false;
            return true;
        }
        if insert_child_recursive(&mut node.children, parent_id, new_node.clone()) {
            return true;
        }
    }
    false
}

fn insert_sibling_in_nodes(nodes: &mut Vec<TaskNode>, target_id: &str, new_node: TaskNode) -> bool {
    if let Some(pos) = nodes.iter().position(|n| n.id == target_id) {
        nodes.insert(pos + 1, new_node);
        return true;
    }
    for node in nodes.iter_mut() {
        if insert_sibling_in_nodes(&mut node.children, target_id, new_node.clone()) {
            return true;
        }
    }
    false
}

fn delete_node_in_nodes(nodes: &mut Vec<TaskNode>, target_id: &str) -> bool {
    if let Some(pos) = nodes.iter().position(|n| n.id == target_id) {
        nodes.remove(pos);
        return true;
    }
    for node in nodes.iter_mut() {
        if delete_node_in_nodes(&mut node.children, target_id) {
            return true;
        }
    }
    false
}

fn outdent_recursive(nodes: &mut Vec<TaskNode>, target_id: &str) -> bool {
    let mut found = None;
    for (i, node) in nodes.iter().enumerate() {
        if let Some(pos) = node.children.iter().position(|n| n.id == target_id) {
            found = Some((i, pos));
            break;
        }
    }
    if let Some((parent_idx, child_idx)) = found {
        let child = nodes[parent_idx].children.remove(child_idx);
        nodes.insert(parent_idx + 1, child);
        return true;
    }
    for node in nodes.iter_mut() {
        if outdent_recursive(&mut node.children, target_id) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> AppData {
        let mut data = AppData::default();
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
        assert!(subtasks.is_expanded);
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
        assert!(text.contains("ZEN"), "zen header missing, got {text:?}");
        assert!(
            text.contains(&crate::rtl_text::shape_arabic("علاج بابا")),
            "zen must still render today's tasks"
        );
        // Zen hides all navigation chrome.
        assert!(!text.contains("HARBOR"), "nav tabs leaked into zen");
        assert!(!text.contains("AUTO-SAVED"), "footer leaked into zen");
    }

    /// The horizontal RTL mind-map must place child cards to the LEFT of their
    /// parent card, vertically centred on the parent, and size cards dynamically
    /// to fit their text up to 42% of the pane width.
    #[test]
    fn mind_map_columns_grow_to_the_left() {
        let m = mind_map_metrics(false, 1000.0);
        assert_eq!(m.font_size, 15.5);
        assert_eq!(m.pad_x, 14.0);
        assert_eq!(m.pad_y, 8.0);
        assert_eq!(m.gap_y, 8.0);
        assert!((m.max_card_w - 420.0).abs() < 0.001);

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
    fn mind_map_metrics_respect_42_percent_and_zoom_bounds() {
        let m = mind_map_metrics(false, 500.0);
        assert!((m.max_card_w - 210.0).abs() < 0.01);
        assert_eq!(m.font_size, 15.5);
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
            text.contains(&shape_arabic("ملاحظات: كتابة اللائحة للمرافعة")),
            "drawer header missing branch title: {text:?}"
        );
        assert!(
            output.shapes.iter().any(|c| matches!(
                &c.shape,
                egui::Shape::Rect(r) if r.fill == theme::DRAWER_BG
            )),
            "expected #F8FAFC drawer background fill"
        );
        assert!(
            output.shapes.iter().any(|c| matches!(
                &c.shape,
                egui::Shape::Rect(r)
                    if r.fill == theme::INK && (r.rect.height() - 1.5).abs() < 0.05
            )),
            "expected 1.5px #0F172A top border on embedded drawer"
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
        assert!(text_p1.contains("BOARD"));
        assert!(text_p1.contains("HARBOR"));
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

        // Pane 0 in sample_data has 3 subtasks: 1 completed, 1 active, 1 cancelled -> ratio 1/3.
        let mut app = WhiteboardApp::from_data(sample_data());
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
            text.contains("[+2]"),
            "expected [+2] badge for folded parent; got {text:?}"
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
}
