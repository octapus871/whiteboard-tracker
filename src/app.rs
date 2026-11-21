use crate::models::{AppData, CardStamp, DayProfile, MainTaskPane, TaskNode, TaskStatus};
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
use std::time::{Duration, Instant};

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
    /// When true, the full-canvas Master Index view (`INDEX` tab) is shown.
    pub show_master_index: bool,
    /// Draft title input at the top of the Master Index view.
    pub index_new_pane_input: String,
    /// Pane row currently selected in the Master Index (for Up/Down keyboard reordering & F2 rename).
    pub index_selected_pane_id: Option<String>,
    /// Pane awaiting a second click ("تأكيد؟") to confirm deletion in Master Index.
    pub index_confirm_delete_pane_id: Option<String>,
    /// Pane whose main title is being inline-edited in the Master Index.
    pub index_editing_pane_id: Option<String>,
    /// Original title backed up when entering inline rename in Master Index so `Esc` cancels cleanly.
    pub index_edit_backup_title: Option<String>,
    /// Requests keyboard focus on a Master Index title editor next frame.
    pub index_focus_edit_id: Option<String>,

    /// Per-pane embedded inline note drawer: maps `pane_id -> open_node_id`.
    pub pane_note_drawers: HashMap<String, String>,
    /// Node ID whose embedded drawer `TextEdit` should grab focus next frame.
    pub focus_notes_editor: Option<String>,

    /// Bounded undo stack (max 30 snapshots) for instant `Ctrl+Z` / `Cmd+Z`.
    pub undo_stack: Vec<AppSnapshot>,
    /// Subtle 4-second inline status bar notice: `(message, expires_at_time)`.
    pub undo_toast: Option<(String, f64)>,
    /// Latest egui frame timestamp for deterministic toast expiration.
    pub last_frame_time: f64,
    /// Card currently displaying its sticky contextual hover strip + last active timestamp
    /// (supports a 180ms grace timer so moving toward `✓ + ≡` never dismisses prematurely).
    pub last_hovered_card: Option<(String, Instant)>,
    /// Parent nodes explicitly expanded by the user (immune to auto-collapse).
    pub manually_expanded_nodes: HashSet<String>,

    /// When true, the chronological Daily Profiles manager modal is open.
    pub show_profiles_modal: bool,
    /// Profile awaiting a 2nd click ("تأكيد؟") before deletion.
    pub profile_confirm_delete_id: Option<String>,
    /// Newly created profile ID whose inline title `TextEdit` should grab focus next frame.
    pub profile_focus_title_id: Option<String>,
    /// When true, the calm Swiss First-Run Quick Guide modal is displayed.
    pub show_onboarding_modal: bool,

    pub active_mode: AppMode,
    pub dirty: bool,
}

/// Snapshot of board state captured prior to destructive or state-changing actions.
#[derive(Debug, Clone)]
pub struct AppSnapshot {
    pub panes: Vec<MainTaskPane>,
    pub archived_panes: Vec<MainTaskPane>,
    pub current_page: usize,
}

const MAX_UNDO_STACK: usize = 30;
const UNDO_TOAST_DURATION_SECS: f64 = 4.0;
const MIN_HITBOX_SIZE: f32 = 28.0;
/// Grace delay before dismissing the contextual hover strip after the pointer leaves
/// the unified `card_rect.union(strip_rect).expand(4.0)` bridge zone.
const HOVER_GRACE_DURATION: Duration = Duration::from_millis(180);

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

    pub fn from_data(mut data: AppData) -> Self {
        data.ensure_profiles_initialized();
        let show_onboarding_modal = !data.has_seen_onboarding;
        let show_quick_entry_modal = data.current_day.panes.is_empty() && !show_onboarding_modal;

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
            index_selected_pane_id: None,
            index_confirm_delete_pane_id: None,
            index_editing_pane_id: None,
            index_edit_backup_title: None,
            index_focus_edit_id: None,
            pane_note_drawers: HashMap::new(),
            focus_notes_editor: None,
            undo_stack: Vec::new(),
            undo_toast: None,
            last_frame_time: 0.0,
            last_hovered_card: None,
            manually_expanded_nodes: HashSet::new(),
            show_profiles_modal: false,
            profile_confirm_delete_id: None,
            profile_focus_title_id: None,
            show_onboarding_modal,
            active_mode: AppMode::LiveBoard,
            dirty: false,
        }
    }

    pub fn auto_save(&mut self) {
        if self.dirty {
            self.data.sync_active_profile_from_current_day();
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

    /// Captures a bounded snapshot of all panes/tasks before a state-changing action.
    pub fn push_undo_snapshot(&mut self) {
        if self.undo_stack.len() >= MAX_UNDO_STACK {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(AppSnapshot {
            panes: self.data.current_day.panes.clone(),
            archived_panes: self.data.archived_panes.clone(),
            current_page: self.current_page,
        });
    }

    /// Restores the latest undo snapshot and displays a 4-second inline toast
    /// in the bottom status bar.
    pub fn undo(&mut self, ctx: Option<&egui::Context>) -> bool {
        let Some(snapshot) = self.undo_stack.pop() else {
            return false;
        };
        self.data.current_day.panes = snapshot.panes;
        self.data.archived_panes = snapshot.archived_panes;
        self.current_page = snapshot.current_page;
        self.clamp_current_page();
        self.editing_node_id = None;
        self.index_confirm_delete_pane_id = None;
        self.index_editing_pane_id = None;
        self.index_edit_backup_title = None;
        if let Some(focused_id) = &self.focused_pane_id {
            if !self
                .data
                .current_day
                .panes
                .iter()
                .any(|p| &p.id == focused_id)
            {
                self.focused_pane_id = None;
            }
        }
        let valid_panes: HashSet<String> = self
            .data
            .current_day
            .panes
            .iter()
            .map(|p| p.id.clone())
            .collect();
        self.pane_note_drawers
            .retain(|pane_id, _| valid_panes.contains(pane_id));

        let now = ctx
            .map(|c| c.input(|i| i.time))
            .unwrap_or(self.last_frame_time);
        self.undo_toast = Some((
            "تم التراجع (Ctrl+Z)".to_string(),
            now + UNDO_TOAST_DURATION_SECS,
        ));
        self.mark_dirty();
        self.auto_save();
        true
    }

    /// Sets the status of a single task node with undo history tracking.
    pub fn set_single_node_status(&mut self, node_id: &str, status: TaskStatus) {
        self.push_undo_snapshot();
        let mut target = HashSet::with_capacity(1);
        target.insert(node_id.to_string());
        for pane in &mut self.data.current_day.panes {
            set_status_in_nodes(&mut pane.subtasks, &target, status);
        }
        self.mark_dirty();
    }

    /// Cycles the subtle geometric watermark stamp on a single task node
    /// (`None -> Dog -> Cat -> Duck -> Bird -> Star -> None`) with undo tracking.
    pub fn cycle_node_stamp(&mut self, node_id: &str) {
        self.push_undo_snapshot();
        for pane in &mut self.data.current_day.panes {
            if let Some(node) = find_node_in_slice_mut(&mut pane.subtasks, node_id) {
                node.stamp = CardStamp::next_option(node.stamp);
                self.mark_dirty();
                return;
            }
        }
    }

    /// Returns the active profile's ISO date string (`YYYY-MM-DD`).
    pub fn active_profile_date(&self) -> &str {
        self.data
            .profiles
            .get(self.data.active_profile_idx)
            .map(|p| p.date_str.as_str())
            .unwrap_or(self.data.current_day.date_iso.as_str())
    }

    /// Loads `profiles[idx]` into `current_day` without overwriting `profiles[idx]`
    /// with the previously active profile's panes.
    fn load_profile_into_canvas(&mut self, idx: usize, ctx: Option<&egui::Context>) {
        if idx >= self.data.profiles.len() {
            return;
        }
        self.close_all_note_drawers(ctx);
        self.data.active_profile_idx = idx;
        self.data.current_day.panes = self.data.profiles[idx].panes.clone();
        self.data.current_day.date_iso = self.data.profiles[idx].date_str.clone();
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
        self.auto_save();
    }

    /// Switches the active board to `profiles[idx]`, persisting current panes first.
    pub fn switch_to_profile(&mut self, idx: usize, ctx: Option<&egui::Context>) {
        if idx >= self.data.profiles.len() {
            return;
        }
        self.data.sync_active_profile_from_current_day();
        self.load_profile_into_canvas(idx, ctx);
    }

    /// Creates a brand-new blank chronological daily profile for today (placed at
    /// index 0, newest first, with empty `panes: Vec::new()` — never cloning the
    /// active profile's tasks), activates it, and queues focus on its inline title editor.
    pub fn create_new_profile(&mut self, ctx: Option<&egui::Context>) -> String {
        self.data.sync_active_profile_from_current_day();
        let mut new_profile = DayProfile::new_for_today(Vec::new());
        let max_ts = self
            .data
            .profiles
            .iter()
            .map(|p| p.created_ts)
            .max()
            .unwrap_or(new_profile.created_ts);
        if new_profile.created_ts <= max_ts {
            new_profile.created_ts = max_ts + 1;
        }
        let new_id = new_profile.id.clone();
        self.data.profiles.insert(0, new_profile);
        self.load_profile_into_canvas(0, ctx);
        self.profile_focus_title_id = Some(new_id.clone());
        new_id
    }

    /// Deletes the profile at `idx` (refuses to delete if only 1 profile remains).
    pub fn delete_profile_at(&mut self, idx: usize, ctx: Option<&egui::Context>) {
        if self.data.profiles.len() <= 1 || idx >= self.data.profiles.len() {
            return;
        }
        self.data.sync_active_profile_from_current_day();
        let was_active = idx == self.data.active_profile_idx;
        let removed = self.data.profiles.remove(idx);
        if self.profile_confirm_delete_id.as_deref() == Some(removed.id.as_str()) {
            self.profile_confirm_delete_id = None;
        }
        if self.profile_focus_title_id.as_deref() == Some(removed.id.as_str()) {
            self.profile_focus_title_id = None;
        }
        if was_active {
            let next_idx = idx.min(self.data.profiles.len() - 1);
            self.load_profile_into_canvas(next_idx, ctx);
        } else {
            if idx < self.data.active_profile_idx {
                self.data.active_profile_idx -= 1;
            }
            self.mark_dirty();
            self.auto_save();
        }
    }

    /// Copies only non-completed, non-cancelled active items from the previous
    /// chronological profile (`active_profile_idx + 1`, or nearest other profile)
    /// into the active profile, pushing an Undo snapshot first.
    pub fn carry_over_active_from_previous(&mut self, ctx: Option<&egui::Context>) -> usize {
        self.data.sync_active_profile_from_current_day();
        let prev_idx = if self.data.active_profile_idx + 1 < self.data.profiles.len() {
            Some(self.data.active_profile_idx + 1)
        } else if self.data.active_profile_idx > 0 {
            Some(self.data.active_profile_idx - 1)
        } else {
            None
        };
        let Some(prev_idx) = prev_idx else {
            return 0;
        };
        let carried: Vec<MainTaskPane> = self.data.profiles[prev_idx]
            .panes
            .iter()
            .filter_map(MainTaskPane::clone_active_only)
            .collect();
        let count = carried.len();
        if count == 0 {
            return 0;
        }
        self.push_undo_snapshot();
        self.data.current_day.panes.extend(carried);
        self.data.sync_active_profile_from_current_day();
        let now = ctx
            .map(|c| c.input(|i| i.time))
            .unwrap_or(self.last_frame_time);
        self.undo_toast = Some((
            format!("تم ترحيل {} لوحات نشطة", count),
            now + UNDO_TOAST_DURATION_SECS,
        ));
        self.mark_dirty();
        self.auto_save();
        count
    }

    /// Appends a new top-level branch to the specified pane with undo tracking.
    pub fn append_branch_to_pane(&mut self, pane_id: &str, title: &str) {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return;
        }
        self.push_undo_snapshot();
        if let Some(pane) = self
            .data
            .current_day
            .panes
            .iter_mut()
            .find(|p| p.id == pane_id)
        {
            pane.subtasks.push(TaskNode::new(trimmed));
            self.mark_dirty();
        }
    }

    pub fn set_selected_nodes_status(&mut self, status: TaskStatus) {
        if self.selected_node_ids.is_empty() && self.selected_pane_ids.is_empty() {
            return;
        }

        self.push_undo_snapshot();
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

        self.push_undo_snapshot();
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
        self.push_undo_snapshot();
        self.data.current_day.panes.push(MainTaskPane::new(trimmed));
        self.current_page = (self.data.current_day.panes.len() - 1) / PANES_PER_PAGE;
        self.mark_dirty();
        self.auto_save();
    }

    /// Deletes the main pane at `idx`, cleaning up per-pane state and clamping `current_page`.
    pub fn delete_pane_at(&mut self, idx: usize, ctx: Option<&egui::Context>) {
        if idx < self.data.current_day.panes.len() {
            self.push_undo_snapshot();
            let removed = self.data.current_day.panes.remove(idx);
            self.selected_pane_ids.remove(&removed.id);
            self.pane_inputs.remove(&removed.id);
            self.close_note_drawer_for_pane(&removed.id, ctx);
            if self.focused_pane_id.as_deref() == Some(removed.id.as_str()) {
                self.focused_pane_id = None;
            }
            if self.index_selected_pane_id.as_deref() == Some(removed.id.as_str()) {
                self.index_selected_pane_id = None;
            }
            if self.index_confirm_delete_pane_id.as_deref() == Some(removed.id.as_str()) {
                self.index_confirm_delete_pane_id = None;
            }
            if self.index_editing_pane_id.as_deref() == Some(removed.id.as_str()) {
                self.index_editing_pane_id = None;
                self.index_edit_backup_title = None;
            }
            self.clamp_current_page();
            self.mark_dirty();
        }
    }

    /// Archives the main pane at `idx` into Harbor (`archived_panes`) with undo tracking.
    pub fn archive_pane_at(&mut self, idx: usize, ctx: Option<&egui::Context>) {
        if idx < self.data.current_day.panes.len() {
            self.push_undo_snapshot();
            let archived = self.data.current_day.panes.remove(idx);
            self.selected_pane_ids.remove(&archived.id);
            self.pane_inputs.remove(&archived.id);
            self.close_note_drawer_for_pane(&archived.id, ctx);
            if self.focused_pane_id.as_deref() == Some(archived.id.as_str()) {
                self.focused_pane_id = None;
            }
            if self.index_selected_pane_id.as_deref() == Some(archived.id.as_str()) {
                self.index_selected_pane_id = None;
            }
            if self.index_confirm_delete_pane_id.as_deref() == Some(archived.id.as_str()) {
                self.index_confirm_delete_pane_id = None;
            }
            if self.index_editing_pane_id.as_deref() == Some(archived.id.as_str()) {
                self.index_editing_pane_id = None;
                self.index_edit_backup_title = None;
            }
            self.data.archived_panes.push(archived);
            self.clamp_current_page();
            self.mark_dirty();
        }
    }

    /// Restores an archived pane from Harbor back to the live board.
    pub fn restore_archived_pane(&mut self, idx: usize) {
        if idx < self.data.archived_panes.len() {
            self.push_undo_snapshot();
            let pane = self.data.archived_panes.remove(idx);
            self.data.current_day.panes.push(pane);
            self.current_page = (self.data.current_day.panes.len() - 1) / PANES_PER_PAGE;
            self.mark_dirty();
        }
    }

    /// Moves a pane up one slot (crossing page boundaries automatically).
    pub fn move_pane_up(&mut self, idx: usize) {
        if idx > 0 && idx < self.data.current_day.panes.len() {
            self.push_undo_snapshot();
            self.data.current_day.panes.swap(idx - 1, idx);
            self.mark_dirty();
        }
    }

    /// Moves a pane down one slot (crossing page boundaries automatically).
    pub fn move_pane_down(&mut self, idx: usize) {
        if idx + 1 < self.data.current_day.panes.len() {
            self.push_undo_snapshot();
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
        self.push_undo_snapshot();
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

        let raw_tasks: Vec<String> = input
            .split(&['/', '\n'][..])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        if !raw_tasks.is_empty() {
            self.push_undo_snapshot();
        }
        for title in raw_tasks {
            self.data.current_day.panes.push(MainTaskPane::new(&title));
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
        if self.show_quick_entry_modal || self.show_profiles_modal || self.show_onboarding_modal {
            return;
        }

        // On HOME, PageUp / PageDown, Alt+Left/Right, and Ctrl+Left/Right flip pages
        // immediately from anywhere (even while the live bottom branch input retains
        // focus), as long as a card title isn't actively being renamed inline.
        let on_home = self.active_mode == AppMode::LiveBoard && !self.show_master_index;
        let (page_up, page_down, nav_left, nav_right) = ctx.input(|i| {
            let nav_mod = i.modifiers.alt || i.modifiers.ctrl || i.modifiers.command;
            (
                i.key_pressed(Key::PageUp),
                i.key_pressed(Key::PageDown),
                nav_mod && i.key_pressed(Key::ArrowLeft),
                nav_mod && i.key_pressed(Key::ArrowRight),
            )
        });
        if on_home && self.editing_node_id.is_none() {
            if page_up || nav_left {
                self.prev_page();
                return;
            }
            if page_down || nav_right {
                self.next_page();
                return;
            }
        }

        if ctx.wants_keyboard_input() {
            return;
        }

        if page_up || nav_left {
            self.prev_page();
            return;
        }
        if page_down || nav_right {
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
        let snapshot = AppSnapshot {
            panes: self.data.current_day.panes.clone(),
            archived_panes: self.data.archived_panes.clone(),
            current_page: self.current_page,
        };
        for pane in &mut self.data.current_day.panes {
            if insert_sibling_in_nodes(&mut pane.subtasks, target_id, new_node.clone()) {
                if self.undo_stack.len() >= MAX_UNDO_STACK {
                    self.undo_stack.remove(0);
                }
                self.undo_stack.push(snapshot);
                return self.begin_editing(new_id);
            }
        }
        None
    }

    fn add_child_under(&mut self, parent_id: &str) -> Option<String> {
        let new_node = TaskNode::new("");
        let new_id = new_node.id.clone();
        let snapshot = AppSnapshot {
            panes: self.data.current_day.panes.clone(),
            archived_panes: self.data.archived_panes.clone(),
            current_page: self.current_page,
        };
        for pane in &mut self.data.current_day.panes {
            if insert_child_recursive(&mut pane.subtasks, parent_id, new_node.clone()) {
                if self.undo_stack.len() >= MAX_UNDO_STACK {
                    self.undo_stack.remove(0);
                }
                self.undo_stack.push(snapshot);
                return self.begin_editing(new_id);
            }
        }
        None
    }

    fn outdent_node(&mut self, target_id: &str) -> Option<String> {
        let snapshot = AppSnapshot {
            panes: self.data.current_day.panes.clone(),
            archived_panes: self.data.archived_panes.clone(),
            current_page: self.current_page,
        };
        for pane in &mut self.data.current_day.panes {
            if outdent_recursive(&mut pane.subtasks, target_id) {
                if self.undo_stack.len() >= MAX_UNDO_STACK {
                    self.undo_stack.remove(0);
                }
                self.undo_stack.push(snapshot);
                self.mark_dirty();
                self.focused_node_id = Some(target_id.to_string());
                return Some(target_id.to_string());
            }
        }
        None
    }

    fn delete_node(&mut self, target_id: &str) -> Option<String> {
        self.delete_node_internal(target_id, true)
    }

    fn delete_node_internal(&mut self, target_id: &str, record_undo: bool) -> Option<String> {
        let snapshot = if record_undo {
            Some(AppSnapshot {
                panes: self.data.current_day.panes.clone(),
                archived_panes: self.data.archived_panes.clone(),
                current_page: self.current_page,
            })
        } else {
            None
        };
        for pane in &mut self.data.current_day.panes {
            if delete_node_in_nodes(&mut pane.subtasks, target_id) {
                if let Some(snap) = snapshot {
                    if self.undo_stack.len() >= MAX_UNDO_STACK {
                        self.undo_stack.remove(0);
                    }
                    self.undo_stack.push(snap);
                }
                self.selected_node_ids.remove(target_id);
                if self.focused_node_id.as_deref() == Some(target_id) {
                    self.focused_node_id = None;
                }
                if self.editing_node_id.as_deref() == Some(target_id) {
                    self.editing_node_id = None;
                }
                self.pane_note_drawers.retain(|_, v| v != target_id);
                self.manually_expanded_nodes.remove(target_id);
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
            self.delete_node_internal(&node_id, false);
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

        let now = ctx.input(|i| i.time);
        self.last_frame_time = now;
        if let Some((_, expires_at)) = &self.undo_toast {
            if now >= *expires_at {
                self.undo_toast = None;
            }
        }

        // Escape always works - even while a text field has focus - so an
        // edit can be cancelled (and an empty new node removed again).
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.show_profiles_modal {
                self.show_profiles_modal = false;
                self.profile_confirm_delete_id = None;
            } else if self.show_onboarding_modal {
                self.show_onboarding_modal = false;
                self.data.has_seen_onboarding = true;
                self.mark_dirty();
            } else if self.show_quick_entry_modal {
                self.show_quick_entry_modal = false;
                self.focus_quick_entry = false;
            } else if let Some(edit_id) = self.index_editing_pane_id.take() {
                if let Some(backup) = self.index_edit_backup_title.take() {
                    if let Some(pane) = self
                        .data
                        .current_day
                        .panes
                        .iter_mut()
                        .find(|p| p.id == edit_id)
                    {
                        pane.title = backup;
                    }
                }
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
                let (close_shortcut, zen_toggle, undo_shortcut) = ctx.input(|i| {
                    let cmd = i.modifiers.ctrl || i.modifiers.command;
                    (
                        cmd && i.key_pressed(Key::Q),
                        cmd && i.key_pressed(Key::H),
                        cmd && !i.modifiers.shift
                            && !i.modifiers.alt
                            && i.key_pressed(Key::Z),
                    )
                });
                if close_shortcut {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else if zen_toggle {
                    self.is_zen = !self.is_zen;
                } else if undo_shortcut
                    && !self.show_quick_entry_modal
                    && !self.show_profiles_modal
                    && !self.show_onboarding_modal
                {
                    self.undo(Some(ctx));
                }
            }

            if !self.show_master_index && !self.show_profiles_modal && !self.show_onboarding_modal {
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

        if self.show_quick_entry_modal && !self.show_onboarding_modal {
            self.render_quick_entry_modal(ctx);
        }
        if self.show_profiles_modal {
            self.render_profiles_modal(ctx);
        }
        if self.show_onboarding_modal {
            self.render_onboarding_modal(ctx);
        }

        if self.dirty {
            self.auto_save();
        }
    }
}

impl WhiteboardApp {
    /// Ultra-minimalist single-line top bar: explicit `HOME | INDEX | HARBOR` tabs
    /// + `[ YYYY-MM-DD ▾ ]` profile pill on the left (crisp `2.0px` `#0F172A` active
    /// underline, instant first-click response), centered `‹ PAGE 1 / N ›` page
    /// navigator, and clock + window controls on the right, over a 1px black hairline.
    /// Window dragging (`ViewportCommand::StartDrag`) is strictly limited to empty
    /// whitespace gaps between clusters so it never intercepts button clicks.
    fn render_top_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let bar_rect = ui.max_rect();
        let total_pages = self.total_pages();
        let primary_released = ctx.input(|i| i.pointer.primary_released());
        let activated = |r: &egui::Response| r.clicked() || (r.hovered() && primary_released);

        let mut any_control_hovered = false;
        let mut surrender_focus = false;

        // Left navigation: `HOME | INDEX | HARBOR` + `[ YYYY-MM-DD ▾ ]` profile pill
        // (or `← BACK [Esc]` when maximized).
        let left_end = (bar_rect.center().x - 102.0).max(bar_rect.left() + 365.0);
        let left_rect = Rect::from_min_max(bar_rect.left_top(), pos2(left_end, bar_rect.bottom()));
        let mut left_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(left_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        left_ui.spacing_mut().item_spacing.x = 6.0;
        let left_used_right = if self.focused_pane_id.is_some() {
            let back_resp = top_bar_tab(&mut left_ui, "← BACK [Esc]", true);
            any_control_hovered |= back_resp.hovered();
            if activated(&back_resp) {
                self.focused_pane_id = None;
                surrender_focus = true;
            }
            back_resp.rect.right()
        } else {
            let home_active = self.active_mode == AppMode::LiveBoard && !self.show_master_index;
            let index_active = self.active_mode == AppMode::LiveBoard && self.show_master_index;
            let harbor_active = self.active_mode == AppMode::Harbor;

            let home_resp = top_bar_tab(&mut left_ui, "HOME", home_active)
                .on_hover_text("ACTIVE 4-PANE BOARD");
            any_control_hovered |= home_resp.hovered();
            if activated(&home_resp) {
                self.active_mode = AppMode::LiveBoard;
                self.show_master_index = false;
                self.focused_pane_id = None;
                self.index_confirm_delete_pane_id = None;
                self.index_editing_pane_id = None;
                self.index_edit_backup_title = None;
                surrender_focus = true;
            }

            left_ui.label(mono("|", 13.0).color(theme::DIVIDER));

            let index_resp = top_bar_tab(&mut left_ui, "INDEX", index_active)
                .on_hover_text("MASTER INDEX");
            any_control_hovered |= index_resp.hovered();
            if activated(&index_resp) {
                self.active_mode = AppMode::LiveBoard;
                self.show_master_index = true;
                self.focused_pane_id = None;
                self.index_confirm_delete_pane_id = None;
                self.index_editing_pane_id = None;
                self.index_edit_backup_title = None;
                surrender_focus = true;
            }

            left_ui.label(mono("|", 13.0).color(theme::DIVIDER));

            let harbor_resp = top_bar_tab(&mut left_ui, "HARBOR", harbor_active)
                .on_hover_text("ARCHIVE & REPORTS");
            any_control_hovered |= harbor_resp.hovered();
            if activated(&harbor_resp) {
                self.active_mode = AppMode::Harbor;
                self.show_master_index = false;
                self.focused_pane_id = None;
                self.index_confirm_delete_pane_id = None;
                self.index_editing_pane_id = None;
                self.index_edit_backup_title = None;
                surrender_focus = true;
            }

            left_ui.add_space(6.0);
            let date_str = self.active_profile_date().to_string();
            let pill_resp = profile_selector_pill(&mut left_ui, &date_str, self.show_profiles_modal)
                .on_hover_text("DAILY PROFILES [PROFILES]");
            any_control_hovered |= pill_resp.hovered();
            if activated(&pill_resp) {
                self.data.sync_active_profile_from_current_day();
                self.show_profiles_modal = !self.show_profiles_modal;
                self.profile_confirm_delete_id = None;
                surrender_focus = true;
            }
            pill_resp.rect.right()
        };

        // Center page navigator: `‹ PAGE 1 / N ›` with generous 28x28 hitboxes and
        // clickable `PAGE 1 / N` label to advance pages cleanly.
        let center_w = 196.0;
        let center_x = bar_rect
            .center()
            .x
            .max(left_used_right + center_w * 0.5 + 8.0);
        let center_rect = Rect::from_center_size(
            pos2(center_x, bar_rect.center().y),
            vec2(center_w, bar_rect.height()),
        );
        let mut center_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(center_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let mut center_used_rect = center_rect;
        center_ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                let row = ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let can_prev = self.current_page > 0;
                    let can_next = self.current_page + 1 < total_pages;

                    let (prev_rect, prev_resp) = ui.allocate_exact_size(
                        vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                        Sense::click(),
                    );
                    let prev_resp = prev_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("PREVIOUS PAGE [PageUp / Ctrl+Left / Alt+Left]");
                    any_control_hovered |= prev_resp.hovered();
                    let prev_color = if can_prev { theme::INK } else { theme::MUTED };
                    if can_prev && prev_resp.hovered() {
                        ui.painter().rect_filled(
                            Rect::from_center_size(prev_rect.center(), vec2(22.0, 22.0)),
                            CornerRadius::same(0),
                            theme::SEL_FILL,
                        );
                    }
                    ui.painter().text(
                        prev_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "‹",
                        FontId::monospace(16.0),
                        prev_color,
                    );
                    if activated(&prev_resp) && can_prev {
                        self.prev_page();
                        surrender_focus = true;
                    }

                    let page_text = format!("PAGE {} / {}", self.current_page + 1, total_pages);
                    let page_font = FontId::monospace(13.5);
                    let page_text_w = ui
                        .fonts_mut(|f| {
                            f.layout_no_wrap(page_text.clone(), page_font.clone(), Color32::TRANSPARENT)
                        })
                        .size()
                        .x;
                    let (page_rect, page_resp) = ui.allocate_exact_size(
                        vec2((page_text_w + 12.0).max(76.0), MIN_HITBOX_SIZE),
                        Sense::click(),
                    );
                    let page_resp = page_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("CLICK: NEXT PAGE");
                    any_control_hovered |= page_resp.hovered();
                    if total_pages > 1 && page_resp.hovered() {
                        ui.painter().rect_filled(
                            Rect::from_center_size(page_rect.center(), vec2(page_rect.width(), 22.0)),
                            CornerRadius::same(0),
                            theme::SEL_FILL,
                        );
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
                    if activated(&page_resp) && total_pages > 1 {
                        self.current_page = (self.current_page + 1) % total_pages;
                        self.focused_pane_id = None;
                        surrender_focus = true;
                    }

                    let (next_rect, next_resp) = ui.allocate_exact_size(
                        vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                        Sense::click(),
                    );
                    let next_resp = next_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("NEXT PAGE [PageDown / Ctrl+Right / Alt+Right]");
                    any_control_hovered |= next_resp.hovered();
                    let next_color = if can_next { theme::INK } else { theme::MUTED };
                    if can_next && next_resp.hovered() {
                        ui.painter().rect_filled(
                            Rect::from_center_size(next_rect.center(), vec2(22.0, 22.0)),
                            CornerRadius::same(0),
                            theme::SEL_FILL,
                        );
                    }
                    ui.painter().text(
                        next_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "›",
                        FontId::monospace(16.0),
                        next_color,
                    );
                    if activated(&next_resp) && can_next {
                        self.next_page();
                        surrender_focus = true;
                    }
                });
                center_used_rect = row.response.rect;
            },
        );

        // Right cluster: window controls + digital clock.
        let right_rect = Rect::from_min_max(
            pos2(
                (center_rect.right() + 8.0).min(bar_rect.right() - 180.0),
                bar_rect.top(),
            ),
            bar_rect.right_bottom(),
        );
        let mut right_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(right_rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        right_ui.spacing_mut().item_spacing.x = 12.0;
        let close_btn = window_control(&mut right_ui, "\u{d7}", "CLOSE");
        any_control_hovered |= close_btn.hovered();
        if activated(&close_btn) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let max_btn = window_control(&mut right_ui, "\u{25a1}", "MAXIMIZE");
        any_control_hovered |= max_btn.hovered();
        if activated(&max_btn) {
            let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        let min_btn = window_control(&mut right_ui, "\u{2014}", "MINIMIZE");
        any_control_hovered |= min_btn.hovered();
        if activated(&min_btn) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        right_ui.add_space(10.0);
        let (time, _date) = clock_strings();
        let short_date = short_date_string();
        let time_resp = right_ui.label(mono(&time, 14.0).strong().color(theme::INK));
        let date_resp = right_ui.label(mono(short_date, 12.5).color(theme::SLATE));
        let right_used_left = date_resp.rect.left().min(time_resp.rect.left());

        // Window dragging strictly on EMPTY top-bar whitespace gaps only — never
        // overlapping any tab, profile pill, page arrow, or window button.
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
                ui.add_space(16.0);
                // Subtle `?` help button (28px hitbox) to reopen Quick Guide anytime.
                let (help_rect, help_resp) =
                    ui.allocate_exact_size(vec2(MIN_HITBOX_SIZE, 26.0), Sense::click());
                let help_resp = help_resp
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text(shape_arabic("دليل الاستخدام السريع"));
                let help_color = if help_resp.hovered() {
                    theme::INK
                } else {
                    theme::SLATE
                };
                if help_resp.hovered() {
                    let pill = Rect::from_center_size(help_rect.center(), vec2(20.0, 20.0));
                    ui.painter()
                        .rect_filled(pill, CornerRadius::same(0), theme::SEL_FILL);
                }
                ui.painter().text(
                    help_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "?",
                    FontId::monospace(13.0),
                    help_color,
                );
                if help_resp.clicked() {
                    self.show_onboarding_modal = true;
                }

                ui.add_space(6.0);
                if self.dirty {
                    ui.label(mono("SAVING...", 12.5).strong().color(theme::AMBER_INK));
                } else {
                    ui.label(mono("AUTO-SAVED", 12.5).color(theme::SLATE));
                }
                if let Some((toast_msg, _)) = &self.undo_toast {
                    ui.label(mono("\u{2022}", 12.5).color(theme::DIVIDER));
                    ui.label(
                        RichText::new(shape_arabic(toast_msg))
                            .size(12.5)
                            .strong()
                            .color(theme::INK),
                    );
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

    /// Clean, centered `0.0px` Swiss architectural modal (`max_width: 640px`,
    /// `1.5px` `#0F172A` border, `4.0px` shadow, `#FFFFFF` fill) listing
    /// chronological daily profiles newest-first with inline title editing,
    /// completion badges, `+ بروفايل جديد`, `ترحيل النشط من الأمس`, `تفعيل`,
    /// and 2-click `حذف` (disabled when only 1 profile remains).
    fn render_profiles_modal(&mut self, ctx: &egui::Context) {
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

        // Dimmed backdrop: clicking outside `modal_rect` closes cleanly.
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
                // Absorb clicks inside `modal_rect` so clicking whitespace or dynamic rows
                // inside the modal NEVER bubbles through to outside-click dismissal.
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
                let mut m_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(Align::Max)),
                );
                m_ui.set_clip_rect(inner);

                // Top header: Arabic title on the right, `PROFILES` tag + crisp X close on the left.
                rtl_row(&mut m_ui, 32.0, |ui| {
                    ui.label(
                        RichText::new(shape_arabic("البروفايلات اليومية"))
                            .size(18.0)
                            .strong()
                            .color(theme::INK),
                    );
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        let (x_rect, x_resp) = ui.allocate_exact_size(
                            vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
                            Sense::click(),
                        );
                        let x_resp = x_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text("CLOSE [Esc]");
                        let x_col = if x_resp.hovered() {
                            theme::INK
                        } else {
                            theme::INK_SOFT
                        };
                        let c = x_rect.center();
                        let s = Stroke::new(1.6, x_col);
                        ui.painter()
                            .line_segment([pos2(c.x - 5.0, c.y - 5.0), pos2(c.x + 5.0, c.y + 5.0)], s);
                        ui.painter()
                            .line_segment([pos2(c.x - 5.0, c.y + 5.0), pos2(c.x + 5.0, c.y - 5.0)], s);
                        if x_resp.clicked() {
                            close_modal = true;
                        }
                        ui.label(mono("PROFILES", 12.0).color(theme::SLATE));
                    });
                });

                m_ui.add_space(10.0);

                // Action bar: `+ بروفايل جديد` and `ترحيل النشط من الأمس`.
                let has_previous = self.data.profiles.len() > 1;
                rtl_row(&mut m_ui, 34.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;

                    let (new_rect, new_resp) =
                        ui.allocate_exact_size(vec2(138.0, 32.0), Sense::click());
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
                        shape_arabic("+ بروفايل جديد"),
                        FontId::proportional(13.5),
                        theme::PANE,
                    );
                    if new_resp.clicked() {
                        create_new = true;
                    }

                    let (carry_rect, carry_resp) =
                        ui.allocate_exact_size(vec2(174.0, 32.0), Sense::click());
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
                        shape_arabic("ترحيل النشط من الأمس"),
                        FontId::proportional(13.5),
                        carry_fg,
                    );
                    if carry_resp.clicked() && has_previous {
                        carry_over = true;
                    }
                });

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
                            let mut row_ui = ui.new_child(
                                UiBuilder::new()
                                    .max_rect(row_inner)
                                    .layout(Layout::right_to_left(Align::Center)),
                            );
                            row_ui.set_clip_rect(row_inner);
                            row_ui.spacing_mut().item_spacing.x = 8.0;

                            // Rightmost: Monospace Bold date string (`YYYY-MM-DD`).
                            row_ui.label(mono(&date_str, 13.0).strong().color(theme::INK));

                            // Inline-editable RTL profile title (`16.0px`).
                            let controls_w = 195.0;
                            let title_w = (row_ui.available_width() - controls_w).max(90.0);
                            let edit_resp = rtl_text_edit(
                                &mut row_ui,
                                Id::new(("profile_title_edit", profile_id.as_str())),
                                &mut profile.title,
                                "عنوان اليوم...",
                                16.0,
                                title_w,
                                Margin::symmetric(4, 2),
                                theme::INK,
                            )
                            .response;
                            if self.profile_focus_title_id.as_deref() == Some(profile_id.as_str()) {
                                edit_resp.request_focus();
                                self.profile_focus_title_id = None;
                            }
                            if edit_resp.changed() {
                                dirty = true;
                            }

                            // Left controls cluster: Delete (`حذف` / `تأكيد؟`), Open (`تفعيل` / `نشط`), completion badge.
                            row_ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                // 1. 2-click Delete (disabled if only 1 profile remains).
                                if !can_delete {
                                    let (del_rect, _) = ui.allocate_exact_size(
                                        vec2(42.0, MIN_HITBOX_SIZE),
                                        Sense::hover(),
                                    );
                                    ui.painter().text(
                                        del_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        shape_arabic("حذف"),
                                        FontId::proportional(12.5),
                                        theme::HINT,
                                    );
                                } else if is_confirming_del {
                                    let (c_rect, c_resp) = ui.allocate_exact_size(
                                        vec2(52.0, MIN_HITBOX_SIZE),
                                        Sense::click(),
                                    );
                                    let c_resp = c_resp.on_hover_cursor(CursorIcon::PointingHand);
                                    let pill =
                                        Rect::from_center_size(c_rect.center(), vec2(50.0, 22.0));
                                    ui.painter().rect_filled(
                                        pill,
                                        CornerRadius::same(0),
                                        theme::RED_BAR,
                                    );
                                    ui.painter().text(
                                        pill.center(),
                                        egui::Align2::CENTER_CENTER,
                                        shape_arabic("تأكيد؟"),
                                        FontId::proportional(12.5),
                                        theme::PANE,
                                    );
                                    if c_resp.clicked() {
                                        delete_idx = Some(idx);
                                    }
                                } else {
                                    let del_resp = index_text_action(
                                        ui,
                                        "حذف",
                                        42.0,
                                        theme::SLATE,
                                        theme::RED_BAR,
                                    )
                                    .on_hover_text("DELETE PROFILE");
                                    if del_resp.clicked() {
                                        self.profile_confirm_delete_id = Some(profile_id.clone());
                                    }
                                }

                                // 2. Activate (`تفعيل` / `OPEN`) or active badge (`نشط`).
                                if is_active {
                                    let (act_rect, _) = ui.allocate_exact_size(
                                        vec2(54.0, MIN_HITBOX_SIZE),
                                        Sense::hover(),
                                    );
                                    let pill =
                                        Rect::from_center_size(act_rect.center(), vec2(50.0, 22.0));
                                    ui.painter().rect_filled(
                                        pill,
                                        CornerRadius::same(0),
                                        theme::GREEN_BAR,
                                    );
                                    ui.painter().text(
                                        pill.center(),
                                        egui::Align2::CENTER_CENTER,
                                        shape_arabic("نشط"),
                                        FontId::proportional(12.5),
                                        theme::PANE,
                                    );
                                } else {
                                    let (open_rect, open_resp) = ui.allocate_exact_size(
                                        vec2(54.0, MIN_HITBOX_SIZE),
                                        Sense::click(),
                                    );
                                    let open_resp =
                                        open_resp.on_hover_cursor(CursorIcon::PointingHand);
                                    let pill = Rect::from_center_size(
                                        open_rect.center(),
                                        vec2(52.0, 22.0),
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
                                        shape_arabic("تفعيل"),
                                        FontId::proportional(12.5),
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
            // Keep Profiles modal open (`show_profiles_modal = true`) and auto-focus
            // the newly created blank profile's inline title editor immediately.
            let new_id = self.create_new_profile(Some(ctx));
            let edit_id = Id::new(("profile_title_edit", new_id.as_str()));
            ctx.memory_mut(|m| m.request_focus(edit_id));
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
        }
    }

    /// Minimal First-Run Quick Guide (`520px` wide, `0.0px` Swiss welcome card,
    /// `#FFFFFF` fill, `1.5px` `#0F172A` border, `4.0px` solid `#0F172A` shadow).
    fn render_onboarding_modal(&mut self, ctx: &egui::Context) {
        let screen = ctx.viewport_rect();
        let card_w = (screen.width() - 40.0).clamp(320.0, 520.0);
        let card_h = 336.0_f32.min(screen.height() - 48.0).max(260.0);
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
                let mut m_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::top_down(Align::Max)),
                );
                m_ui.set_clip_rect(inner);

                // Title: `دليل الاستخدام السريع` (`20.0px` Bold `#0F172A`).
                m_ui.label(
                    RichText::new(shape_arabic("دليل الاستخدام السريع"))
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

                let bullets = [
                    "1. إضافة الفروع: اكتب في الشريط السفلي لأي لوحة واضغط Enter.",
                    "2. طيّ الفروع: اضغط المربع الصغير [−] أو [+N] يسار البطاقة لإخفاء أو إظهار تفرعاتها.",
                    "3. الملاحظات: نقرة واحدة على أي بطاقة تفتح دفتر الملاحظات الورقي الخاص بها.",
                    "4. تنظيم الصفحات: افتح تبويب INDEX وحدد أي مهمة لتحريكها بين الصفحات بالأسهم ↑ ↓.",
                ];
                let bullet_font = FontId::proportional(15.5);
                let bullet_color = Color32::from_rgb(0x1E, 0x29, 0x3B);
                let wrap_w = m_ui.available_width().max(120.0);
                for line in bullets {
                    let (wrapped, size) = wrap_and_shape_rtl(&m_ui, line, bullet_font.clone(), wrap_w);
                    let (line_rect, _) =
                        m_ui.allocate_exact_size(vec2(wrap_w, size.y + 4.0), Sense::hover());
                    let mut job = egui::text::LayoutJob::default();
                    job.wrap.max_width = f32::INFINITY;
                    job.halign = Align::RIGHT;
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
                    m_ui.painter().galley(
                        pos2(line_rect.right(), line_rect.top()),
                        galley,
                        bullet_color,
                    );
                    m_ui.add_space(8.0);
                }

                m_ui.add_space(10.0);
                // High-contrast CTA button `ابدأ العمل` (`#0F172A` fill, `#FFFFFF` 15.5px Bold).
                let (btn_rect, btn_resp) =
                    m_ui.allocate_exact_size(vec2(150.0, 38.0), Sense::click());
                let btn_resp = btn_resp.on_hover_cursor(CursorIcon::PointingHand);
                let btn_bg = if btn_resp.hovered() {
                    theme::GRID
                } else {
                    theme::INK
                };
                m_ui.painter()
                    .rect_filled(btn_rect, CornerRadius::same(0), btn_bg);
                let cta_text = shape_arabic("ابدأ العمل");
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
            self.mark_dirty();
            self.auto_save();
        }
    }

    /// Full-canvas Swiss architectural Master Index view (`#FFFFFF`, `0.0px`):
    /// lists only main pane titles grouped into pages of 4 separated by `2.0px`
    /// solid `#0F172A` dividers (`PAGE 01`, `PAGE 02`, ...), with inline add,
    /// row selection + Up/Down keyboard reordering across pages, F2 / double-click /
    /// `تعديل` inline RTL rename (`Enter` saves, `Esc` cancels), `أرشفة`, and
    /// 2-click `حذف` confirmation.
    fn render_master_index(&mut self, ui: &mut Ui) {
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
                                    let mut row_ui = ui.new_child(
                                        UiBuilder::new()
                                            .max_rect(row_inner)
                                            .layout(Layout::right_to_left(Align::Center)),
                                    );
                                    row_ui.set_clip_rect(row_inner);
                                    row_ui.spacing_mut().item_spacing.x = 10.0;

                                    // Rightmost: monospace index `[0N]`.
                                    row_ui.label(
                                        mono(format!("[{:02}]", idx + 1), 14.0)
                                            .strong()
                                            .color(theme::INK_SOFT),
                                    );

                                    // Title region: single-click selects row, double-click edits inline.
                                    let controls_w = if is_confirming_del { 356.0 } else { 340.0 };
                                    let title_w = (row_ui.available_width() - controls_w).max(100.0);

                                    if is_editing {
                                        let edit_resp = rtl_text_edit(
                                            &mut row_ui,
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
                                        let enter_pressed = (edit_resp.has_focus()
                                            || edit_resp.lost_focus())
                                            && row_ui.input(|i| i.key_pressed(Key::Enter));
                                        if enter_pressed || edit_resp.lost_focus() {
                                            commit_edit = true;
                                        }
                                    } else {
                                        let title_display = if pane.title.trim().is_empty() {
                                            shape_arabic("بدون عنوان")
                                        } else {
                                            shape_arabic(&pane.title)
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
                                                .halign(Align::Max)
                                                .truncate(),
                                            )
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(
                                                "CLICK: SELECT · DOUBLE-CLICK / F2: RENAME · UP/DOWN: REORDER · ENTER: OPEN PAGE",
                                            );
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

                                    // Left-aligned controls cluster (100% valid text/vector icons, >=28px hitboxes).
                                    row_ui.with_layout(
                                        Layout::left_to_right(Align::Center),
                                        |ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;

                                            // 1. Delete (`حذف` -> 2-click confirm `"تأكيد؟"`).
                                            if is_confirming_del {
                                                let (confirm_rect, confirm_resp) =
                                                    ui.allocate_exact_size(
                                                        vec2(54.0, MIN_HITBOX_SIZE),
                                                        Sense::click(),
                                                    );
                                                let confirm_resp = confirm_resp
                                                    .on_hover_cursor(CursorIcon::PointingHand);
                                                let pill = Rect::from_center_size(
                                                    confirm_rect.center(),
                                                    vec2(52.0, 22.0),
                                                );
                                                ui.painter().rect_filled(
                                                    pill,
                                                    CornerRadius::same(0),
                                                    theme::RED_BAR,
                                                );
                                                ui.painter().text(
                                                    pill.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    shape_arabic("تأكيد؟"),
                                                    FontId::proportional(12.5),
                                                    theme::PANE,
                                                );
                                                if confirm_resp.clicked() {
                                                    delete_idx = Some(idx);
                                                }
                                            } else {
                                                let del_resp = index_text_action(
                                                    ui,
                                                    "حذف",
                                                    38.0,
                                                    theme::SLATE,
                                                    theme::RED_BAR,
                                                )
                                                .on_hover_text("DELETE PANE");
                                                if del_resp.clicked() {
                                                    self.index_selected_pane_id =
                                                        Some(pane_id.clone());
                                                    self.index_confirm_delete_pane_id =
                                                        Some(pane_id.clone());
                                                }
                                            }

                                            // 2. Archive (`أرشفة` -> moves pane to Harbor archive).
                                            let arch_resp = index_text_action(
                                                ui,
                                                "أرشفة",
                                                46.0,
                                                theme::SLATE,
                                                theme::INK,
                                            )
                                            .on_hover_text("ARCHIVE TO HARBOR");
                                            if arch_resp.clicked() {
                                                self.index_confirm_delete_pane_id = None;
                                                archive_idx = Some(idx);
                                            }

                                            // 3. Inline rename (`تعديل`, >=28px tall hitbox).
                                            let edit_btn = index_text_action(
                                                ui,
                                                "تعديل",
                                                44.0,
                                                if is_editing { theme::INK } else { theme::SLATE },
                                                theme::INK,
                                            )
                                            .on_hover_text("RENAME [F2]");
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

                                            // 4. Reorder up (`↑`) and down (`↓`, painter-drawn vectors, 28x28 hitboxes).
                                            let can_up = idx > 0;
                                            let can_down = idx + 1 < pane_count;

                                            let up_resp = index_arrow_button(ui, true, can_up)
                                                .on_hover_text("MOVE UP [Up]");
                                            if up_resp.clicked() && can_up {
                                                self.index_selected_pane_id = Some(pane_id.clone());
                                                move_up = Some(idx);
                                                self.index_confirm_delete_pane_id = None;
                                            }

                                            let down_resp = index_arrow_button(ui, false, can_down)
                                                .on_hover_text("MOVE DOWN [Down]");
                                            if down_resp.clicked() && can_down {
                                                self.index_selected_pane_id = Some(pane_id.clone());
                                                move_down = Some(idx);
                                                self.index_confirm_delete_pane_id = None;
                                            }

                                            // 5. Page assignment badge `[P1]` / `[P2]`
                                            // (compact visual box inside a generous >=28px hitbox).
                                            let p_badge = format!("[P{}]", page_idx + 1);
                                            let (hit_rect, badge_resp) = ui.allocate_exact_size(
                                                vec2(40.0, MIN_HITBOX_SIZE),
                                                Sense::click(),
                                            );
                                            let badge_resp = badge_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text("MOVE TO NEXT PAGE");
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
                                        },
                                    );

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

    fn render_harbor_placeholder(&mut self, ui: &mut Ui) {
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
                        ui.label(mono("HARBOR", 20.0).strong());
                        ui.add_space(6.0);
                        ui.label(
                            mono("NO ARCHIVED PANES YET.", 12.0).color(theme::TEXT_MUTED),
                        );
                    });
            });
            return;
        }

        let mut restore_idx: Option<usize> = None;
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
                        Layout::top_down(Align::Max),
                        |ui| {
                            ui.set_min_width(content_w);
                            ui.label(mono("HARBOR", 18.0).strong().color(theme::INK));
                            ui.add_space(8.0);
                            let (rule, _) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 2.0),
                                Sense::hover(),
                            );
                            ui.painter()
                                .rect_filled(rule, CornerRadius::same(0), theme::INK);
                            ui.add_space(8.0);

                            for (idx, pane) in self.data.archived_panes.iter().enumerate() {
                                let (done, total) = count_pane_progress(&pane.subtasks);
                                rtl_row(ui, 42.0, |ui| {
                                    ui.label(
                                        RichText::new(shape_arabic(&pane.title))
                                            .size(16.0)
                                            .strong()
                                            .color(theme::INK),
                                    );
                                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                        let r = index_text_action(
                                            ui,
                                            "استعادة",
                                            56.0,
                                            theme::INK_SOFT,
                                            theme::INK,
                                        )
                                        .on_hover_text("RESTORE TO BOARD");
                                        if r.clicked() {
                                            restore_idx = Some(idx);
                                        }
                                        ui.label(
                                            mono(format!("{}/{}", done, total), 13.0)
                                                .color(theme::SLATE),
                                        );
                                    });
                                });
                            }
                        },
                    );
                });
            });

        if let Some(idx) = restore_idx {
            self.restore_archived_pane(idx);
        }
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
                let now = Instant::now();
                let prev_hovered_card: Option<String> = if self.is_zen || self.editing_node_id.is_some()
                {
                    None
                } else {
                    self.last_hovered_card.as_ref().and_then(|(id, since)| {
                        if now.saturating_duration_since(*since) < HOVER_GRACE_DURATION {
                            Some(id.clone())
                        } else {
                            None
                        }
                    })
                };
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
                        prev_hovered_card.as_deref(),
                        &mut self.manually_expanded_nodes,
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

        let hover_now = Instant::now();
        if self.is_zen || self.editing_node_id.is_some() {
            self.last_hovered_card = None;
        } else if let Some(active_id) = actions.next_hovered_card {
            self.last_hovered_card = Some((active_id, hover_now));
        } else if let Some((_, since)) = &self.last_hovered_card {
            let elapsed = hover_now.saturating_duration_since(*since);
            if elapsed >= HOVER_GRACE_DURATION {
                self.last_hovered_card = None;
            } else {
                ui.ctx()
                    .request_repaint_after(HOVER_GRACE_DURATION - elapsed);
            }
        }

        if actions.dirty {
            self.dirty = true;
        }

        if let Some(idx) = actions.delete_pane {
            self.delete_pane_at(idx, Some(ui.ctx()));
        }

        if let Some((node_id, status)) = actions.set_node_status {
            self.set_single_node_status(&node_id, status);
        }
        if let Some(node_id) = actions.cycle_node_stamp {
            self.cycle_node_stamp(&node_id);
        }
        if let Some((pane_id, title)) = actions.append_branch {
            self.append_branch_to_pane(&pane_id, &title);
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
    /// Update a single node's status with undo history tracking.
    set_node_status: Option<(String, TaskStatus)>,
    /// Cycle a single node's geometric watermark stamp (`None -> Dog -> ... -> None`).
    cycle_node_stamp: Option<String>,
    /// Append a top-level branch to a pane with undo history tracking.
    append_branch: Option<(String, String)>,
    /// Open the embedded note drawer on card click without stealing card focus.
    select_open_drawer: Option<String>,
    /// Open the embedded note drawer and immediately focus the notes editor.
    focus_open_drawer: Option<String>,
    /// Close the embedded note drawer for the specified pane ID.
    close_drawer_pane: Option<String>,
    /// Card currently displaying the contextual top-left hover strip.
    next_hovered_card: Option<String>,
    node_rects: Vec<(String, Rect)>,
    pane_header_rects: Vec<(String, Rect)>,
    dirty: bool,
    /// Node that must grab keyboard focus on the next frame (carried over).
    focus_node: Option<String>,
}

/// Expands `visual_rect` symmetrically so its invisible hit target is at least
/// `28.0 × 28.0` while preserving compact painted geometry.
#[allow(dead_code)]
fn min_hitbox(visual_rect: Rect) -> Rect {
    let w = visual_rect.width().max(MIN_HITBOX_SIZE);
    let h = visual_rect.height().max(MIN_HITBOX_SIZE);
    Rect::from_center_size(visual_rect.center(), vec2(w, h))
}

/// Crisp `0.0px` painter-drawn vector arrow button (`up = true` for `↑`, `false`
/// for `↓`) used for row reordering in `INDEX`. Never relies on Unicode arrow
/// font glyphs and allocates a generous `28.0 × 28.0` hitbox.
fn index_arrow_button(ui: &mut Ui, up: bool, enabled: bool) -> egui::Response {
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
fn index_text_action(
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
        shape_arabic(label),
        FontId::proportional(13.0),
        color,
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

/// Embedded light-mode Swiss architectural note card (`0.0px` corners, `#F8FAFC`
/// paper fill, `1.5px` `#0F172A` border, `3.0px` solid `#0F172A` offset shadow,
/// minimalist code/text-block header with `NOTE` tag, `COPY`, and vector `X`
/// close button, plus a frameless `16.0px`-padded RTL writing canvas).
fn render_pane_note_drawer(
    ui: &mut Ui,
    rect: Rect,
    pane_id: &str,
    node: &mut TaskNode,
    focus_notes_editor: &mut Option<String>,
    actions: &mut PaneActions,
) {
    let painter = ui.painter();
    // Clean white pane backdrop behind the architectural note card so its 3px shadow pops.
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
    let mut card_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(inner_rect)
            .layout(Layout::top_down(Align::Max)),
    );
    card_ui.set_clip_rect(inner_rect);

    let node_id = node.id.clone();
    let now = ui.ctx().input(|i| i.time);
    let copy_time_id = Id::new(("note_copy_feedback", node_id.as_str()));
    let copied_until: f64 = ui
        .ctx()
        .data(|d| d.get_temp::<f64>(copy_time_id).unwrap_or(0.0));
    let show_copied = now < copied_until;

    // Minimalist top action header (`36.0px`):
    // Right: branch title (`15.5px` Bold `#0F172A`).
    // Left: `X` close button, `COPY` button, subtle `NOTE` tag (`12.0px` `#64748B`).
    let header_rect = Rect::from_min_size(inner_rect.left_top(), vec2(inner_rect.width(), 36.0));
    let header_inner = Rect::from_min_max(
        pos2(header_rect.left() + 14.0, header_rect.top()),
        pos2(header_rect.right() - 16.0, header_rect.bottom()),
    );
    let mut header_ui = card_ui.new_child(
        UiBuilder::new()
            .max_rect(header_inner)
            .layout(Layout::right_to_left(Align::Center)),
    );
    header_ui.set_clip_rect(header_inner);

    let controls_reserve = 160.0;
    let title_width = (header_inner.width() - controls_reserve).max(40.0);
    let branch_title = if node.title.trim().is_empty() {
        "بدون عنوان"
    } else {
        node.title.trim()
    };
    let title_font = FontId::proportional(15.5);
    let title_shaped = shape_arabic(branch_title);
    let (title_rect, _) =
        header_ui.allocate_exact_size(vec2(title_width, 32.0), Sense::hover());
    let title_galley = header_ui.fonts_mut(|f| {
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = title_width;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.halign = Align::RIGHT;
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
        title_rect.right(),
        title_rect.center().y - title_galley.size().y * 0.5,
    );
    let title_painter = header_ui.painter_at(title_rect);
    title_painter.galley(title_pos, title_galley.clone(), theme::INK);
    title_painter.galley(title_pos + vec2(0.45, 0.0), title_galley, theme::INK);

    // Left-aligned action cluster (`X` close, `COPY`, `NOTE`).
    header_ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
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
        close_response.on_hover_text("CLOSE NOTES [Esc]");

        // 2. Crisp Copy button (clean text `COPY` / `COPIED`, zero missing-glyph icon, >=28px hitbox).
        let copy_label = if show_copied { "COPIED" } else { "COPY" };
        let copy_w = if show_copied { 52.0 } else { 42.0 };
        let (copy_rect, copy_response) =
            ui.allocate_exact_size(vec2(copy_w, MIN_HITBOX_SIZE), Sense::click());
        let copy_response = copy_response
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("COPY NOTES TO CLIPBOARD");
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
            copy_label,
            FontId::monospace(11.5),
            copy_color,
        );
        if copy_response.clicked() {
            ui.ctx().copy_text(node.notes.clone());
            ui.ctx()
                .data_mut(|d| d.insert_temp(copy_time_id, now + 1.5));
        }

        // 3. Subtle monospace `NOTE` tag (`12.0px` `#64748B`).
        ui.label(mono("NOTE", 12.0).color(theme::SLATE));
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

    // Frameless distraction-free RTL writing canvas (`16.0px` inner padding, `16.0px` font):
    // Arabic starts flush against the right `16.0px` padding, flows naturally right-to-left,
    // and places the caret accurately without mutating the logical `node.notes` buffer.
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
                "اكتب ملاحظاتك هنا...",
                16.0,
                Margin::same(16),
                theme::DRAWER_BG,
                theme::INK,
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

/// Renders a single task pane: header + RTL progress hairline, horizontal-first
/// mind-map (auto-splitting into 2 balanced half-width columns when dense), and
/// a strict `38.0px` bottom input strip.
#[allow(clippy::too_many_arguments)]
fn render_pane(
    ui: &mut Ui,
    pane: &mut MainTaskPane,
    pane_idx: usize,
    is_selected: bool,
    zen: bool,
    _focus: bool,
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

    // Reserve a dedicated 38.0px bottom strip (#FFFFFF fill, 1.0px #E2E8F0 top border)
    // for `+ اكتب فرعاً جديداً...` so tree nodes never collide with it.
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

    let mut body_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(body_rect)
            .layout(Layout::top_down(Align::Max)),
    );
    body_ui.set_clip_rect(upper_rect);

    let mut focus_toggle: Option<bool> = None;
    let (done, total) = count_pane_progress(&pane.subtasks);

    // Swiss architectural header: bold Arabic title flush right, a
    // monospace index + progress badge on the left.
    rtl_row(&mut body_ui, 36.0, |ui| {
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
            // Hover-only delete affordance (28x28 hitbox, no permanent clutter).
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

    body_ui.add_space(10.0);
    // Header-embedded 2.0px progress hairline: right-to-left Emerald
    // Green (#10B981) proportional to completion ratio, with the
    // remaining left segment in #CBD5E1.
    let (rule_rect, _) =
        body_ui.allocate_exact_size(Vec2::new(body_ui.available_width(), 2.0), Sense::hover());
    let ratio = if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    };
    let done_w = rule_rect.width() * ratio;
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
    body_ui.add_space(4.0);

    // Mind-map canvas strictly above the bottom 38.0px input strip.
    let tree_rect = body_ui.available_rect_before_wrap();
    let avail_w = tree_rect.width().max(60.0);
    let avail_h = tree_rect.height().max(40.0);
    let editing_snap = editing_node_id.clone();

    let single_base = mind_map_metrics_for_density(avail_w, total);

    // Predictable, non-jumping auto-collapse policy:
    // Never collapse active/in-progress trees or while the user is editing a
    // node in this pane. Auto-collapse ONLY parent groups whose descendants
    // are 100% Completed when the pane exceeds its available vertical height
    // (unless the user manually expanded that group).
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
        // Automatic height-balanced 2-column split (Right Half 50% | Left Half 50%)
        // with a crisp 1.0px #E2E8F0 center vertical divider and an 8.0px inner margin
        // on both sides (16.0px total gutter). Each column is strictly clipped to its
        // own half-pane rect and owns an independent vertical ScrollArea with full
        // bottom shadow (3.0px) + 12.0px bottom padding clearance.
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
        // Compute each root group's actual half-column subtree pixel height for packing.
        let root_heights: Vec<f32> = pane
            .subtasks
            .iter()
            .map(|n| subtree_height(&body_ui, n, &half_base, 0, editing_snap.as_deref()))
            .collect();
        let split_idx = find_balanced_split(&root_heights, half_base.gap_y);

        // Clean 1.0px #E2E8F0 vertical column divider spanning header hairline -> bottom input strip.
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
            let (right_slice, left_slice) = pane.subtasks.split_at(split_idx);
            let rh = stack_height(&body_ui, right_slice, &metrics, 0, editing_snap.as_deref());
            let lh = stack_height(&body_ui, left_slice, &metrics, 0, editing_snap.as_deref());
            let rw = stack_width(&body_ui, right_slice, &metrics, 0, editing_snap.as_deref());
            let lw = stack_width(&body_ui, left_slice, &metrics, 0, editing_snap.as_deref());
            let req_h = rh.max(lh) + DENSE_TOP_MARGIN + bottom_clearance;
            let req_w = rw.max(lw) + SHADOW_PAD;
            let scale = fit_scale(req_h, req_w, avail_h, half_w);
            if (scale - 1.0).abs() < 0.005 {
                metrics = half_base;
                break;
            }
            metrics = half_base.scaled(scale);
        }

        let (right_nodes, left_nodes) = pane.subtasks.split_at_mut(split_idx);
        let right_h = stack_height(&body_ui, right_nodes, &metrics, 0, editing_snap.as_deref());
        let left_h = stack_height(&body_ui, left_nodes, &metrics, 0, editing_snap.as_deref());
        let right_content_h = (right_h + DENSE_TOP_MARGIN + bottom_clearance).max(avail_h);
        let left_content_h = (left_h + DENSE_TOP_MARGIN + bottom_clearance).max(avail_h);

        // Right Column: strictly clipped to `right_col_rect` with its own vertical ScrollArea.
        let mut right_col_ui = body_ui.new_child(UiBuilder::new().max_rect(right_col_rect));
        right_col_ui.set_clip_rect(right_col_rect);
        ScrollArea::vertical()
            .id_salt(("pane_scroll_2col_r", pane.id.as_str()))
            .auto_shrink([false, false])
            .max_height(avail_h)
            .show(&mut right_col_ui, |ui| {
                ui.set_clip_rect(right_col_rect);
                let col_w = right_col_rect.width();
                let (content_rect, _) =
                    ui.allocate_exact_size(vec2(col_w, right_content_h), Sense::hover());
                render_mind_map_nodes(
                    ui,
                    right_nodes,
                    0,
                    0,
                    DENSE_TOP_MARGIN + right_h * 0.5,
                    col_w - 2.0,
                    content_rect.min.x,
                    content_rect.min.y,
                    &metrics,
                    zen,
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

        // Left Column: strictly clipped to `left_col_rect` with its own vertical ScrollArea.
        let mut left_col_ui = body_ui.new_child(UiBuilder::new().max_rect(left_col_rect));
        left_col_ui.set_clip_rect(left_col_rect);
        ScrollArea::vertical()
            .id_salt(("pane_scroll_2col_l", pane.id.as_str()))
            .auto_shrink([false, false])
            .max_height(avail_h)
            .show(&mut left_col_ui, |ui| {
                ui.set_clip_rect(left_col_rect);
                let col_w = left_col_rect.width();
                let (content_rect, _) =
                    ui.allocate_exact_size(vec2(col_w, left_content_h), Sense::hover());
                render_mind_map_nodes(
                    ui,
                    left_nodes,
                    0,
                    split_idx,
                    DENSE_TOP_MARGIN + left_h * 0.5,
                    col_w - 2.0,
                    content_rect.min.x,
                    content_rect.min.y,
                    &metrics,
                    zen,
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
        // Single wide horizontal right-to-left tree across the full pane width.
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
            // Optical upper-middle center computed against the unshrunk pane height so
            // opening the bottom note card on Click 1 never jumps cards out from under
            // the cursor before Click 2 of a double-click rename.
            let preferred = (unshrunk_avail_h * 0.34).max(min_center);
            let max_center = (avail_h - bottom_clearance - tree_h * 0.5).max(min_center);
            preferred.min(max_center)
        } else {
            min_center
        };

        // Clipped ScrollArea safety net above the 38.0px bottom input strip.
        let seeded = scroll_seeded.contains(&pane.id);
        let mut area = ScrollArea::both()
            .id_salt(("pane_scroll", pane.id.as_str()))
            .auto_shrink([false, false])
            .max_height(avail_h);
        if overflow_x > 1.0 && !seeded {
            area = area.horizontal_scroll_offset(overflow_x);
        }
        area.show(&mut body_ui, |ui| {
            let total_w = avail_w.max(content_w);
            let (content_rect, _) =
                ui.allocate_exact_size(vec2(total_w, content_h), Sense::hover());
            let origin = content_rect.min;
            render_mind_map_nodes(
                ui,
                &mut pane.subtasks,
                0,
                0,
                block_center_y,
                total_w - 2.0,
                origin.x,
                origin.y,
                &metrics,
                zen,
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

    // Paint fold handles cleanly over Bezier origins strictly inside each handle's
    // own column clip rect, then paint the sticky contextual hover strip on top.
    paint_fold_handles(ui, &fold_handles);

    let has_direct_hover = hover_controls.iter().any(|c| c.is_direct_hover);
    let visible_controls: Vec<HoverControl> = if has_direct_hover {
        hover_controls
            .into_iter()
            .filter(|c| c.is_direct_hover)
            .collect()
    } else {
        hover_controls
    };
    let strip_painter = ui.painter_at(pane_outer_rect);
    paint_hover_controls(&strip_painter, &visible_controls);

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
        render_pane_branch_input(&mut input_ui, &pane.id, pane_input, actions);
    }
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
fn shape_multiline_for_editor(
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
/// index in `visual_text` (produced by [`shape_multiline_for_editor`]).
fn rtl_multiline_visual_cursor_index(
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
            let slice_has_arabic = crate::rtl_text::contains_arabic(&slice_str);
            let visual_col = if slice_has_arabic {
                line_len.saturating_sub(col)
            } else {
                col
            };
            return line_start + visual_col;
        }
        line_start = line_end + 1;
    }
    clamped
}

/// Frameless multiline RTL `TextEdit` for the architectural note card:
/// * Keeps the mutable `text` buffer purely logical UTF-8 (never mutates with bidi).
/// * Aligns every line flush against the right padding (`16.0px`).
/// * Paints an accurate blinking RTL caret at the true visual leading edge on Arabic lines.
#[allow(clippy::too_many_arguments)]
fn rtl_multiline_text_edit(
    ui: &mut Ui,
    id: Id,
    text: &mut String,
    hint: &str,
    font_size: f32,
    margin: Margin,
    bg_color: Color32,
    text_color: Color32,
) -> egui::text_edit::TextEditOutput {
    let has_arabic = crate::rtl_text::contains_arabic(text);
    let prev_cursor = ui.visuals().text_cursor.clone();
    if has_arabic {
        // Suppress egui's default LTR caret when editing Arabic so we can paint
        // the true RTL leading-edge caret cleanly.
        ui.visuals_mut().text_cursor.stroke = Stroke::NONE;
        ui.visuals_mut().text_cursor.preview = false;
    }

    let font = FontId::proportional(font_size);
    let mut shaped_snapshot = String::new();
    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
        let shaped = shape_multiline_for_editor(ui, buf.as_str(), &font, wrap_width);
        shaped_snapshot = shaped.clone();
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = wrap_width;
        job.halign = Align::RIGHT;
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
    };

    let output = TextEdit::multiline(text)
        .id(id)
        .font(font.clone())
        .desired_width(f32::INFINITY)
        .desired_rows(4)
        .frame(false)
        .margin(margin)
        .background_color(bg_color)
        .text_color(text_color)
        .horizontal_align(Align::RIGHT)
        .layouter(&mut layouter)
        .show(ui);

    if has_arabic {
        ui.visuals_mut().text_cursor = prev_cursor.clone();
    }

    let has_focus = output.response.has_focus();
    let viewport_focused = ui.ctx().input(|i| i.focused);

    if text.is_empty() && !has_focus && !hint.is_empty() {
        let inner = output.response.rect - margin;
        let anchor = pos2(inner.right(), inner.top());
        let painter = ui.painter_at(output.text_clip_rect.expand(1.0));
        painter.text(
            anchor,
            egui::Align2::RIGHT_TOP,
            shape_arabic(hint),
            font,
            theme::MUTED,
        );
    } else if has_arabic && has_focus && viewport_focused {
        let logical_idx = output
            .cursor_range
            .map(|r| r.primary.index)
            .unwrap_or_else(|| text.chars().count());
        let visual_idx = rtl_multiline_visual_cursor_index(text, &shaped_snapshot, logical_idx);
        let local = output
            .galley
            .pos_from_cursor(egui::text::CCursor::new(visual_idx));
        let caret_x = output.galley_pos.x + local.min.x;
        let caret_top = output.galley_pos.y + local.min.y;
        let caret_bottom = output.galley_pos.y + local.max.y;

        let now = ui.ctx().input(|i| i.time);
        let reset_blink = output.response.changed() || output.response.gained_focus();
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

/// A minimal text window control (minimize / maximize / close) with a generous `28x28` hitbox.
fn window_control(ui: &mut Ui, glyph: &str, tip: &str) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let color = if response.hovered() {
        theme::INK
    } else {
        theme::INK_SOFT
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::monospace(14.0),
        color,
    );
    response.on_hover_text(tip)
}

/// Single-click architectural top-bar navigation tab (`HOME`, `INDEX`, `HARBOR`)
/// with full-bar height hit target, `PointingHand` cursor, and crisp `2.0px`
/// solid `#0F172A` active bottom underline.
fn top_bar_tab(ui: &mut Ui, text: &str, active: bool) -> egui::Response {
    let font = FontId::monospace(14.0);
    let text_w = ui
        .fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::TRANSPARENT))
        .size()
        .x;
    let tab_w = (text_w + 14.0).max(48.0);
    let tab_h = ui.available_height().max(32.0);
    let (rect, response) = ui.allocate_exact_size(vec2(tab_w, tab_h), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let color = if active || response.hovered() {
        theme::INK
    } else {
        theme::SLATE
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font.clone(),
        color,
    );
    if active {
        // Bold impression + crisp 2.0px #0F172A bottom active underline.
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

/// Compact single-click chronological profile selector pill `[ YYYY-MM-DD ▾ ]`
/// (`14.0px` SemiBold, `0.0px` sharp border, painter-drawn downward triangle).
fn profile_selector_pill(ui: &mut Ui, date_str: &str, open: bool) -> egui::Response {
    let font = FontId::monospace(13.5);
    let label = format!("[ {}   ]", date_str);
    let text_w = ui
        .fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), Color32::TRANSPARENT))
        .size()
        .x;
    let pill_w = (text_w + 14.0).max(122.0);
    let (hit_rect, response) =
        ui.allocate_exact_size(vec2(pill_w, ui.available_height().max(28.0)), Sense::click());
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

    if text.is_empty() && !has_focus && !hint.is_empty() {
        // Hide placeholder immediately on focus or non-empty text so the typing
        // caret never overlaps the `+` glyph.
        let inner = output.response.rect - margin;
        let anchor = pos2(inner.right(), inner.center().y);
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
/// branch (recorded in the undo stack), clears the field, and retains focus.
fn render_pane_branch_input(
    ui: &mut Ui,
    pane_id: &str,
    input: &mut String,
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
            actions.append_branch = Some((pane_id.to_string(), trimmed));
            input.clear();
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
/// Height of the zero-overlap contextual hover strip (`0.0px` edge pill).
const HOVER_STRIP_H: f32 = 22.0;
/// Width of each action slot inside the contextual hover strip (`✓`, `+`, `≡`).
const HOVER_STRIP_SLOT_W: f32 = 24.0;
/// Width of the compact `STAMP` cycle slot in the contextual hover strip.
const HOVER_STRIP_STAMP_W: f32 = 42.0;
/// Crisp single-color slate stroke (`#94A3B8`) for geometric card watermark stamps.
fn stamp_watermark_stroke_color() -> Color32 {
    Color32::from_rgb(0x94, 0xA3, 0xB8)
}

/// Ultra-subtle cool slate interior tint (`#CBD5E1` @ ~22% alpha) backing geometric line-art stamps.
fn stamp_watermark_color() -> Color32 {
    Color32::from_rgba_unmultiplied(0xCB, 0xD5, 0xE1, 56)
}
/// Top margin applied when top-aligning dense task trees inside a pane.
const DENSE_TOP_MARGIN: f32 = 14.0;
/// Extra bottom padding reserved beneath the bottom card's 3.0px shadow inside scroll areas.
const BOTTOM_SCROLL_PAD: f32 = 12.0;
/// Dedicated bottom input strip height (`#FFFFFF` fill, `1.0px` `#E2E8F0` top border).
const BOTTOM_INPUT_HEIGHT: f32 = 38.0;
/// Generous horizontal breathing room between parent left edge and child right edge (single column).
const BRANCH_GAP_X: f32 = 56.0;
/// Compact horizontal Bezier branch span inside 2-column split panes so child cards never cross columns.
const SPLIT_BRANCH_GAP_X: f32 = 30.0;
/// Inner horizontal margin on each side of the center vertical divider (`16.0px` total gutter).
const SPLIT_COL_GUTTER: f32 = 8.0;

/// Metrics driving the horizontal right-to-left mind-map layout.
#[derive(Clone, Copy)]
struct MindMapMetrics {
    /// Maximum width of a root task card.
    max_card_w: f32,
    /// Maximum width of a child sub-branch card (capped tighter inside split columns).
    child_max_card_w: f32,
    /// Horizontal gap between parent and child cards (Bezier curves span this).
    gap_x: f32,
    /// Minimum height of one task card.
    min_card_h: f32,
    /// Vertical gap between two sibling branches (8.0px base).
    gap_y: f32,
    /// Font size of a level-0 task (15.5px).
    font_size: f32,
    /// Font size of a nested branch (14.5px).
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

/// Width of the crisp `0.0px` root-only index badge (`01`, `02`, `03`...) flush
/// inside the right edge of top-level root cards (`depth == 0`).
const ROOT_BADGE_W: f32 = 24.0;

/// Computes adaptive mind-map metrics for a pane column of width `pane_w`:
/// * `<= 7` total nodes -> spacious (`14.0px` H, `8.0px` V, `8.0px` sibling gap).
/// * `> 7` total nodes -> compact (`12.0px` H, `5.0px` V, `5.0px` sibling gap).
/// * Root cards use `15.5px` Bold; child sub-branches use `14.5px` Regular.
/// * Generous `56.0px` horizontal Bezier gap and wider card cap so Arabic phrases
///   sit comfortably on 1–2 lines without cramped wrapping.
fn mind_map_metrics_for_density(pane_w: f32, total_nodes: usize) -> MindMapMetrics {
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
fn mind_map_metrics_for_split_col(col_w: f32, total_nodes: usize) -> MindMapMetrics {
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
fn find_balanced_split(heights: &[f32], gap_y: f32) -> usize {
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
fn mind_map_metrics(_focus: bool, pane_w: f32) -> MindMapMetrics {
    mind_map_metrics_for_density(pane_w, 0)
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

/// Wraps logical Arabic/RTL text into lines fitting `max_width` *before*
/// applying RTL visual reversal on each line individually, so multi-line
/// Arabic paragraphs always read naturally top-to-bottom (never reversing line
/// order). Returns `(shaped_multiline_text, measured_size)`.
fn wrap_and_shape_rtl(
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
fn measure_card_size(
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
const FOLD_HANDLE_SIZE: f32 = 14.0;
/// Generous invisible click hitbox (`24.0px × 24.0px`) for the fold handle.
const FOLD_HANDLE_HITBOX: f32 = 24.0;

/// Auto-collapses ONLY parent groups whose descendants are 100% Completed
/// (`TaskStatus::Completed`) when the pane exceeds `avail_h`, skipping any
/// group the user manually expanded. Never touches active/in-progress trees.
fn auto_collapse_completed_groups(
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
            node.is_expanded = false;
            changed = true;
        }
    }
    changed
}

/// Computes the crisp `0.0px` left-border-straddling fold handle dimensions
/// (`14.0px × 14.0px` base, widening slightly only for multi-digit `+N`).
fn fold_handle_visual_size(ui: &Ui, is_folded: bool, hidden_count: usize) -> Vec2 {
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
        let half_handle = fold_handle_visual_size(ui, true, node.descendant_count()).x * 0.5;
        return own_w + half_handle + 2.0;
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

/// Zero-overlap contextual hover strip painted flush along the top-left outer
/// edge of a hovered card (`y - 22.0`, `22.0px` height, `#0F172A` fill, `#FFFFFF`
/// `13.5px` Bold icons: `✓`, `+`, `≡`).
#[derive(Clone, Copy)]
struct HoverControl {
    strip_rect: Rect,
    check_rect: Rect,
    plus_rect: Rect,
    notes_rect: Rect,
    stamp_rect: Rect,
    check_hovered: bool,
    plus_hovered: bool,
    notes_hovered: bool,
    stamp_hovered: bool,
    is_completed: bool,
    has_stamp: bool,
    is_direct_hover: bool,
}

/// Deferred visual for the `14.0px × 14.0px` left-border-straddling fold handle
/// (`[−]` when expanded, `[+N]` when collapsed), painted cleanly over Bezier origins
/// while staying strictly clipped to its column rect.
#[derive(Clone, Copy)]
struct FoldHandleVisual {
    rect: Rect,
    clip_rect: Rect,
    is_folded: bool,
    hidden_count: usize,
    emphasized: bool,
}

/// Renders one column of the horizontal mind-map and returns the anchors of
/// the cards it produced, so the caller can ink curves into them.
#[allow(clippy::too_many_arguments)]
fn render_mind_map_nodes(
    ui: &mut Ui,
    nodes: &mut [TaskNode],
    depth: usize,
    root_index_offset: usize,
    block_center_y: f32,
    col_right: f32,
    origin_x: f32,
    origin_y: f32,
    m: &MindMapMetrics,
    zen: bool,
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
        // Root branches only (`depth == 0`) get a sequential 1-based index (`01`, `02`, ...).
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
            let child_col_right = col_right - card_size.x - m.gap_x;
            let child_cards = render_mind_map_nodes(
                ui,
                &mut node.children,
                depth + 1,
                0,
                center,
                child_col_right,
                origin_x,
                origin_y,
                m,
                zen,
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

/// Draws a Swiss minimalist task card with clear root vs. child visual hierarchy:
/// * Root (`depth == 0`): `1.5px` `#0F172A` border, `#FFFFFF` fill, `15.5px` Bold
///   text, and flush right-edge `0.0px` monospace number badge (`01`, `02`, ...).
/// * Child (`depth > 0`): `1.0px` `#64748B` border, `#F8FAFC` paper fill, `14.5px`
///   Regular text, and no number badge.
#[allow(clippy::too_many_arguments)]
fn render_node_card(
    ui: &mut Ui,
    node: &mut TaskNode,
    rect: Rect,
    depth: usize,
    root_number: Option<usize>,
    m: &MindMapMetrics,
    zen: bool,
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

    // Zero-width-penalty 14×14 architectural fold handle straddling `rect.left_center()`.
    // Its 24×24 invisible click hitbox is nudged 3px left so it covers the entire
    // handle generously without colliding with the top-left dog-ear zone.
    let fold_handle_rects = if has_children {
        let vis_size = fold_handle_visual_size(ui, is_folded, hidden_count);
        let vis_rect = Rect::from_center_size(rect.left_center(), vis_size);
        let hit_rect = Rect::from_center_size(
            pos2(rect.left() - 3.0, rect.center().y),
            vec2(vis_size.x.max(FOLD_HANDLE_HITBOX), FOLD_HANDLE_HITBOX),
        );
        Some((vis_rect, hit_rect))
    } else {
        None
    };

    // Sticky zero-gap contextual hover strip flush against the card's top edge
    // (`strip_rect.bottom() == rect.top()`, zero gap), clamped horizontally inside
    // `pane_rect.shrink(4.0)` so short cards near the right edge never clip the strip.
    let strip_w = HOVER_STRIP_SLOT_W * 3.0 + HOVER_STRIP_STAMP_W;
    let clamped_x = rect.left().clamp(
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
    let stamp_rect = Rect::from_min_size(
        pos2(
            strip_rect.left() + HOVER_STRIP_SLOT_W * 3.0,
            strip_rect.top(),
        ),
        vec2(HOVER_STRIP_STAMP_W, HOVER_STRIP_H),
    );

    let mut card_hit_area = rect;
    if let Some((vis_rect, _)) = fold_handle_rects {
        card_hit_area = card_hit_area.union(vis_rect);
    }
    let col_clip = ui.clip_rect();
    let card_visible = rect.intersects(col_clip) && rect.top() >= col_clip.top() - 2.0;
    let is_sticky_card = prev_hovered_card == Some(node_id.as_str());
    let bridge_zone = rect.union(strip_rect).expand(4.0);
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let pointer_on_card =
        card_visible && hover_pos.is_some_and(|p| col_clip.contains(p) && card_hit_area.contains(p));
    let pointer_in_bridge = card_visible
        && !zen
        && !is_editing
        && is_sticky_card
        && hover_pos.is_some_and(|p| bridge_zone.contains(p));
    let direct_hover = pointer_on_card || pointer_in_bridge;
    let hovering = direct_hover || (card_visible && !zen && !is_editing && is_sticky_card);
    if direct_hover && !zen && !is_editing && actions.next_hovered_card.is_none() {
        actions.next_hovered_card = Some(node_id.clone());
    }

    // 1. Solid 3.0px hard offset shadow behind the card (Active / Completed / Cancelled).
    let painter = ui.painter();
    painter.rect_filled(
        rect.translate(SHADOW_OFFSET),
        CornerRadius::same(theme::CARD_RADIUS),
        theme::status_shadow_color(node.status),
    );

    // 2. Architectural visual hierarchy:
    //    Root (`depth == 0`): `#FFFFFF` fill, `1.5px` `#0F172A` border (2.0px when selected).
    //    Child (`depth > 0`): `#F8FAFC` paper fill, `1.0px` `#64748B` border.
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

    // 2b. Root-only monospace number badge (`01`, `02`, `03`...) flush on the right edge.
    if let Some(num) = root_number {
        let num_rect = Rect::from_min_max(
            pos2(rect.right() - ROOT_BADGE_W, rect.top()),
            rect.right_bottom(),
        );
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

    // 2c. Visible single-color geometric line-art stamp (`24.0px × 24.0px` bounds,
    // `4.0px` inset in the bottom-left interior corner) painted BEFORE card text so it
    // sits cleanly behind text without interfering with readability.
    if let Some(stamp) = node.stamp {
        let stamp_bounds = Rect::from_min_size(
            pos2(rect.left() + 4.0, (rect.bottom() - 4.0 - 24.0).max(rect.top() + 2.0)),
            vec2(24.0, 24.0),
        );
        let stamp_painter = ui.painter_at(rect.shrink(1.5).intersect(col_clip));
        paint_card_stamp(&stamp_painter, stamp_bounds, stamp);
    }

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

    let badge_reserve = if root_number.is_some() {
        ROOT_BADGE_W
    } else {
        0.0
    };
    let content_rect = Rect::from_min_max(
        pos2(rect.left() + m.pad_x, rect.top() + m.pad_y),
        pos2(
            (rect.right() - m.pad_x - badge_reserve).max(rect.left() + m.pad_x + 12.0),
            rect.bottom() - m.pad_y,
        ),
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

        // Generous 28×28 invisible top-left dog-ear hit-zone (painted triangle stays 8×8).
        let dog_ear_rect = Rect::from_min_size(
            rect.left_top(),
            vec2(MIN_HITBOX_SIZE, MIN_HITBOX_SIZE),
        );
        let dog_ear_response = ui
            .interact(
                dog_ear_rect,
                ui.id().with(("dog_ear", &node_id)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);

        // Zero-width-penalty left-border-straddling fold handle (`[−]` / `[+N]`).
        // Registered after card & dog-ear so clicking its 24×24 hitbox ONLY
        // toggles collapse/expand without opening the note card or renaming.
        let mut fold_handle_clicked = false;
        if let Some((vis_rect, hit_rect)) = fold_handle_rects {
            let handle_response = ui
                .interact(
                    hit_rect,
                    ui.id().with(("fold_handle", &node_id)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand);
            if handle_response.clicked() {
                fold_handle_clicked = true;
                node.collapsed = !node.collapsed;
                node.is_expanded = !node.collapsed;
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
                Rect::from_center_size(rect.left_center(), sz)
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
            handle_response.on_hover_text(if node.is_folded() {
                "CLICK: EXPAND BRANCH"
            } else {
                "CLICK: COLLAPSE BRANCH"
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
        job.halign = Align::RIGHT;
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
            content_rect.right(),
            content_rect.center().y - galley.size().y * 0.5,
        );
        let text_painter = ui.painter_at(content_rect.expand(1.0).intersect(col_clip));
        if is_root {
            // Bold visual weight (`15.5px` Bold) for root cards.
            text_painter.galley(text_pos, galley.clone(), text_color);
            text_painter.galley(text_pos + vec2(0.45, 0.0), galley, text_color);
        } else {
            // Regular visual weight (`14.5px` Regular) for child sub-branches.
            text_painter.galley(text_pos, galley, text_color);
        }

        // Strict click separation:
        // - Fold handle (`[−]` / `[+N]`): toggles collapse/expand only (handled above).
        // - Double-click card body: inline rename card text.
        // - Single-click card body: select card & open its light paper note card.
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

        card_response.on_hover_text(format!(
            "{} · CLICK: SELECT & NOTES · DOUBLE-CLICK: RENAME · 1/2/3 OR SPACE: STATUS",
            node.status.label_en().to_uppercase(),
        ));
    }

    let show_strip = card_visible
        && !zen
        && !is_editing
        && (direct_hover || (is_sticky_card && actions.next_hovered_card.is_none()));
    if show_strip {
        // Expand each strip action upward by 6px for effortless 28px-tall click targets
        // without extending downward into the card's text or dog-ear region.
        let hit_up = |r: Rect| Rect::from_min_max(pos2(r.left(), r.top() - 6.0), r.right_bottom());

        let check_response = ui
            .interact(
                hit_up(check_rect),
                ui.id().with(("hover_check", &node_id)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let plus_response = ui
            .interact(
                hit_up(plus_rect),
                ui.id().with(("hover_plus", &node_id)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let notes_response = ui
            .interact(
                hit_up(notes_rect),
                ui.id().with(("hover_notes", &node_id)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let stamp_response = ui
            .interact(
                hit_up(stamp_rect),
                ui.id().with(("hover_stamp", &node_id)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);

        if check_response.clicked() {
            let next_status = if node.status == TaskStatus::Completed {
                TaskStatus::Active
            } else {
                TaskStatus::Completed
            };
            actions.set_node_status = Some((node_id.clone(), next_status));
        }
        if plus_response.clicked() {
            actions.add_child = Some(node_id.clone());
        }
        if notes_response.clicked() {
            selected_node_ids.clear();
            selected_node_ids.insert(node_id.clone());
            *focused_node_id = Some(node_id.clone());
            actions.focus_open_drawer = Some(node_id.clone());
        }
        if stamp_response.clicked() {
            actions.cycle_node_stamp = Some(node_id.clone());
        }

        hover_controls.push(HoverControl {
            strip_rect,
            check_rect,
            plus_rect,
            notes_rect,
            stamp_rect,
            check_hovered: check_response.hovered(),
            plus_hovered: plus_response.hovered(),
            notes_hovered: notes_response.hovered(),
            stamp_hovered: stamp_response.hovered(),
            is_completed: node.status == TaskStatus::Completed,
            has_stamp: node.stamp.is_some(),
            is_direct_hover: direct_hover,
        });

        check_response.on_hover_text("TOGGLE ACTIVE / COMPLETED");
        plus_response.on_hover_text("ADD CHILD BRANCH");
        notes_response.on_hover_text("OPEN NOTES DRAWER");
        stamp_response.on_hover_text(match node.stamp {
            Some(s) => format!("STAMP: {} (CLICK TO CYCLE)", s.label_en()),
            None => "CYCLE WATERMARK STAMP".to_string(),
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
        actions.set_node_status = Some((node_id.to_string(), TaskStatus::Active));
    }
    if ui
        .button("COMPLETED [2]")
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.set_node_status = Some((node_id.to_string(), TaskStatus::Completed));
    }
    if ui
        .button("CANCELLED [3]")
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.set_node_status = Some((node_id.to_string(), TaskStatus::Cancelled));
    }
    ui.separator();
    let stamp_label = match node.stamp {
        Some(s) => format!("STAMP: {}", s.label_en()),
        None => "STAMP: NONE".to_string(),
    };
    if ui
        .button(stamp_label)
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
    {
        actions.cycle_node_stamp = Some(node_id.to_string());
    }
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

/// Paints the zero-width-penalty `14.0px × 14.0px` fold handles (`[−]` / `[+N]`)
/// straddling each parent card's left border center cleanly over Bezier origins,
/// strictly clipped to each handle's column rect.
fn paint_fold_handles(ui: &Ui, handles: &[FoldHandleVisual]) {
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

/// Paints a distinct, recognizable monochrome geometric line-art silhouette
/// (`24.0px × 24.0px` bounds, `1.5px` `#94A3B8` stroke with subtle `#CBD5E1`
/// interior wash) inside a card's bottom-left interior corner behind card text.
fn paint_card_stamp(painter: &Painter, b: Rect, stamp: CardStamp) {
    let stroke_col = stamp_watermark_stroke_color();
    let fill_col = stamp_watermark_color();
    let stroke = Stroke::new(1.5, stroke_col);
    let thin = Stroke::new(1.2, stroke_col);
    let p = |x: f32, y: f32| pos2(b.left() + x * b.width(), b.top() + y * b.height());

    match stamp {
        CardStamp::Dog => {
            // Geometric profile dog head with floppy ear & prominent snout.
            let head_pts = vec![
                p(0.24, 0.20),
                p(0.56, 0.20),
                p(0.66, 0.38),
                p(0.92, 0.42),
                p(0.88, 0.62),
                p(0.54, 0.66),
                p(0.46, 0.88),
                p(0.18, 0.88),
                p(0.18, 0.44),
            ];
            painter.add(Shape::convex_polygon(
                vec![p(0.24, 0.20), p(0.56, 0.20), p(0.66, 0.38), p(0.54, 0.66), p(0.18, 0.44)],
                fill_col,
                Stroke::NONE,
            ));
            painter.add(Shape::closed_line(head_pts, stroke));
            // Floppy ear hanging along left side of head.
            let ear_pts = vec![p(0.22, 0.24), p(0.42, 0.28), p(0.38, 0.64), p(0.18, 0.58)];
            painter.add(Shape::convex_polygon(ear_pts.clone(), fill_col, Stroke::NONE));
            painter.add(Shape::closed_line(ear_pts, stroke));
            // Eye + nose tip accent.
            painter.circle_filled(p(0.54, 0.36), 1.4, stroke_col);
            painter.circle_filled(p(0.90, 0.44), 1.6, stroke_col);
        }
        CardStamp::Cat => {
            // Geometric cat face with two sharp pointed triangular ears & whiskers.
            let face_pts = vec![
                p(0.20, 0.12),
                p(0.38, 0.32),
                p(0.62, 0.32),
                p(0.80, 0.12),
                p(0.82, 0.62),
                p(0.50, 0.86),
                p(0.18, 0.62),
            ];
            painter.add(Shape::convex_polygon(
                vec![p(0.20, 0.32), p(0.80, 0.32), p(0.82, 0.62), p(0.50, 0.86), p(0.18, 0.62)],
                fill_col,
                Stroke::NONE,
            ));
            painter.add(Shape::closed_line(face_pts, stroke));
            // Eyes & triangular nose.
            painter.circle_filled(p(0.37, 0.50), 1.4, stroke_col);
            painter.circle_filled(p(0.63, 0.50), 1.4, stroke_col);
            painter.add(Shape::closed_line(
                vec![p(0.45, 0.60), p(0.55, 0.60), p(0.50, 0.67)],
                thin,
            ));
            // Four crisp geometric whiskers.
            painter.line_segment([p(0.04, 0.54), p(0.24, 0.58)], thin);
            painter.line_segment([p(0.04, 0.67), p(0.24, 0.65)], thin);
            painter.line_segment([p(0.76, 0.58), p(0.96, 0.54)], thin);
            painter.line_segment([p(0.76, 0.65), p(0.96, 0.67)], thin);
        }
        CardStamp::Duck => {
            // Geometric duck silhouette with flat horizontal bill & upturned tail.
            let body_pts = vec![
                p(0.28, 0.20),
                p(0.48, 0.20),
                p(0.52, 0.48),
                p(0.76, 0.48),
                p(0.94, 0.34),
                p(0.86, 0.74),
                p(0.72, 0.84),
                p(0.28, 0.84),
                p(0.22, 0.62),
                p(0.28, 0.46),
            ];
            painter.add(Shape::convex_polygon(
                vec![p(0.26, 0.48), p(0.86, 0.48), p(0.72, 0.84), p(0.28, 0.84)],
                fill_col,
                Stroke::NONE,
            ));
            painter.add(Shape::closed_line(body_pts, stroke));
            // Flat geometric bill extending left.
            let bill_pts = vec![p(0.06, 0.34), p(0.28, 0.30), p(0.28, 0.44), p(0.08, 0.44)];
            painter.add(Shape::convex_polygon(bill_pts.clone(), fill_col, Stroke::NONE));
            painter.add(Shape::closed_line(bill_pts, stroke));
            // Eye + wing fold line.
            painter.circle_filled(p(0.38, 0.30), 1.4, stroke_col);
            painter.line_segment([p(0.44, 0.60), p(0.72, 0.60)], thin);
            painter.line_segment([p(0.72, 0.60), p(0.62, 0.72)], thin);
        }
        CardStamp::Bird => {
            // Geometric origami bird with swept triangular wing, beak & forked tail.
            let beak = vec![p(0.06, 0.42), p(0.26, 0.34), p(0.26, 0.48)];
            painter.add(Shape::closed_line(beak, stroke));
            let body = vec![
                p(0.26, 0.34),
                p(0.56, 0.46),
                p(0.94, 0.38),
                p(0.76, 0.60),
                p(0.88, 0.74),
                p(0.52, 0.70),
                p(0.24, 0.54),
            ];
            painter.add(Shape::convex_polygon(
                vec![p(0.26, 0.34), p(0.56, 0.46), p(0.52, 0.70), p(0.24, 0.54)],
                fill_col,
                Stroke::NONE,
            ));
            painter.add(Shape::closed_line(body, stroke));
            // Swept origami wing + crisp crease line.
            let wing = vec![p(0.34, 0.40), p(0.68, 0.10), p(0.60, 0.48)];
            painter.add(Shape::convex_polygon(wing.clone(), fill_col, Stroke::NONE));
            painter.add(Shape::closed_line(wing, stroke));
            painter.line_segment([p(0.46, 0.44), p(0.68, 0.10)], thin);
            // Legs.
            painter.line_segment([p(0.42, 0.66), p(0.38, 0.88)], thin);
            painter.line_segment([p(0.52, 0.68), p(0.54, 0.88)], thin);
        }
        CardStamp::Star => {
            // Crisp 5-point geometric star outline (`1.5px` `#94A3B8`) + subtle `#CBD5E1` wash.
            let center = b.center();
            let r_outer = b.width() * 0.46;
            let r_inner = b.width() * 0.20;
            let mut star_pts = Vec::with_capacity(10);
            let mut outer = [Pos2::ZERO; 5];
            let mut inner = [Pos2::ZERO; 5];
            for i in 0..5 {
                let a_out = -std::f32::consts::FRAC_PI_2
                    + (i as f32) * std::f32::consts::TAU / 5.0;
                let a_in = a_out + std::f32::consts::PI / 5.0;
                outer[i] = pos2(
                    center.x + r_outer * a_out.cos(),
                    center.y + r_outer * a_out.sin(),
                );
                inner[i] = pos2(
                    center.x + r_inner * a_in.cos(),
                    center.y + r_inner * a_in.sin(),
                );
                star_pts.push(outer[i]);
                star_pts.push(inner[i]);
            }
            for i in 0..5 {
                let prev_in = inner[(i + 4) % 5];
                painter.add(Shape::convex_polygon(
                    vec![center, prev_in, outer[i], inner[i]],
                    fill_col,
                    Stroke::NONE,
                ));
            }
            painter.add(Shape::closed_line(star_pts, stroke));
        }
    }
}

/// Paints the zero-overlap `0.0px` contextual hover strip (`22.0px` height,
/// `#0F172A` background, `#FFFFFF` `13.5px` Bold icons: `✓`, `+`, `≡`, `ختم`).
fn paint_hover_controls(painter: &Painter, controls: &[HoverControl]) {
    let font = FontId::monospace(13.5);
    let stamp_font = FontId::proportional(12.5);
    let stamp_label = shape_arabic("ختم");
    for control in controls {
        painter.rect_filled(control.strip_rect, CornerRadius::same(0), theme::INK);

        // Subtle hover tint per segment so the active icon feels crisp and tactile.
        for (seg_rect, hovered) in [
            (control.check_rect, control.check_hovered),
            (control.plus_rect, control.plus_hovered),
            (control.notes_rect, control.notes_hovered),
            (control.stamp_rect, control.stamp_hovered),
        ] {
            if hovered {
                painter.rect_filled(seg_rect, CornerRadius::same(0), theme::GRID);
            }
        }

        // Crisp 1px vertical separators between the 4 actions.
        let sep_stroke = Stroke::new(1.0, theme::INK_SOFT);
        for sep_x in [
            control.plus_rect.left(),
            control.notes_rect.left(),
            control.stamp_rect.left(),
        ] {
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

        let stamp_color = if control.has_stamp {
            theme::GREEN_SHADOW
        } else {
            theme::PANE
        };
        painter.text(
            control.stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            &stamp_label,
            stamp_font.clone(),
            stamp_color,
        );
        painter.text(
            control.stamp_rect.center() + vec2(0.4, 0.0),
            egui::Align2::CENTER_CENTER,
            &stamp_label,
            stamp_font.clone(),
            stamp_color,
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
        assert!(text.contains("NOTE"), "note card header missing NOTE tag: {text:?}");
        assert!(text.contains("COPY"), "note card header missing COPY action: {text:?}");
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
    fn min_hitbox_expands_small_targets_and_hover_strip_is_zero_overlap() {
        let tiny = Rect::from_min_size(pos2(100.0, 100.0), vec2(8.0, 8.0));
        let expanded = min_hitbox(tiny);
        assert!((expanded.width() - 28.0).abs() < 0.01);
        assert!((expanded.height() - 28.0).abs() < 0.01);
        assert!((expanded.center().x - tiny.center().x).abs() < 0.01);

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
                    && (r.rect.left() - card_tl.x).abs() < 0.1
            }
            _ => false,
        });
        assert!(
            strip_above_card,
            "expected 22.0px #0F172A hover strip flush along top-left outer edge (y - 22.0)"
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

        // 3. Single-click card body -> selects card & opens its light paper note card.
        let card_body_pos = pos2(handle_rect.center().x + 48.0, handle_rect.center().y);
        let _ = click_at(card_body_pos, 3.0, &mut app);
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

        // 4. Double-click parent card body -> enters inline rename mode.
        let _ = click_at(card_body_pos, 4.0, &mut app);
        let _ = click_at(card_body_pos, 4.12, &mut app);
        assert_eq!(
            app.editing_node_id.as_deref(),
            Some(parent_id.as_str()),
            "double-clicking parent card body must enter inline rename mode"
        );
    }

    #[test]
    fn auto_collapse_only_folds_completed_groups_when_overflowing() {
        let ctx = egui::Context::default();
        crate::rtl_text::install_arabic_fonts(&ctx);
        crate::theme::install(&ctx);

        let mut data = AppData::default();
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
    fn sticky_hover_strip_bridge_and_180ms_grace_delay() {
        let ctx = egui::Context::default();
        crate::rtl_text::install_arabic_fonts(&ctx);
        crate::theme::install(&ctx);

        let mut app = WhiteboardApp::from_data(sample_data());
        app.show_quick_entry_modal = false;
        app.data.current_day.panes.truncate(1);
        app.data.current_day.panes[0].subtasks[0].notes = "ملاحظة".to_string();
        let node_id = app.data.current_day.panes[0].subtasks[0].id.clone();

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
            .expect("dog-ear triangle must locate card top-left");

        // 1. Hover card body -> strip renders flush against card top edge (`strip.bottom == card_tl.y`).
        let mut hover_card = egui::RawInput::default();
        hover_card
            .events
            .push(egui::Event::PointerMoved(card_tl + vec2(30.0, 16.0)));
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
            app.last_hovered_card.as_ref().map(|(id, _)| id.as_str()),
            Some(node_id.as_str())
        );

        // 2. Move pointer into `card_rect.union(strip_rect).expand(4.0)` bridge (above card, slightly right of strip).
        let mut hover_bridge = egui::RawInput::default();
        hover_bridge
            .events
            .push(egui::Event::PointerMoved(pos2(strip_rect.right() + 2.0, card_tl.y - 6.0)));
        let out_bridge = ctx.run(hover_bridge, |ctx| app.draw(ctx));
        assert!(
            rendered_text(&out_bridge).contains("≡"),
            "strip must stay visible inside card.union(strip).expand(4.0) bridge zone"
        );

        // 3. Move pointer completely outside -> strip remains visible during the 180ms grace timer.
        let mut hover_far = egui::RawInput::default();
        hover_far
            .events
            .push(egui::Event::PointerMoved(pos2(5.0, 5.0)));
        let out_grace = ctx.run(hover_far.clone(), |ctx| app.draw(ctx));
        assert!(
            rendered_text(&out_grace).contains("≡"),
            "strip must stay visible during the 180ms grace window after leaving"
        );
        assert!(
            app.last_hovered_card.is_some(),
            "last_hovered_card must stay active during 180ms grace window"
        );

        // 4. After 180ms elapses, strip dismisses cleanly.
        app.last_hovered_card = Some((node_id, Instant::now() - Duration::from_millis(220)));
        let out_expired = ctx.run(hover_far, |ctx| app.draw(ctx));
        assert!(
            !rendered_text(&out_expired).contains("≡"),
            "strip must dismiss cleanly once the 180ms grace timer expires"
        );
        assert!(app.last_hovered_card.is_none());
    }

    #[test]
    fn top_bar_tabs_single_click_immediately_and_page_keys_work_while_input_focused() {
        let ctx = egui::Context::default();
        crate::rtl_text::install_arabic_fonts(&ctx);
        crate::theme::install(&ctx);

        let mut data = AppData::default();
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
        assert!(idx_text.contains(&shape_arabic("أرشفة")), "expected أرشفة action in INDEX");
        assert!(idx_text.contains(&shape_arabic("حذف")), "expected حذف action in INDEX");
        assert!(idx_text.contains(&shape_arabic("تعديل")), "expected تعديل action in INDEX");

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
        assert!(rendered_text(&out).contains("COPY"));
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

        // 2. Startup on the next day ("2026-11-22") prepends a fresh empty today profile at index 0
        //    without duplicating yesterday's tasks automatically.
        loaded.ensure_profiles_for_date("2026-11-22", 1_795_286_400, "الأحد — مهام اليوم".into());
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.profiles[0].date_str, "2026-11-22");
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

        // Fresh state has `has_seen_onboarding == false` -> Quick Guide opens automatically.
        let mut fresh = AppData::default();
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
    fn monochrome_geometric_watermark_stamps_cycle_and_render_in_bottom_left_corner() {
        // Verify full stamp cycle None -> Dog -> Cat -> Duck -> Bird -> Star -> None.
        let mut s = None;
        for expected in [
            Some(CardStamp::Dog),
            Some(CardStamp::Cat),
            Some(CardStamp::Duck),
            Some(CardStamp::Bird),
            Some(CardStamp::Star),
            None,
        ] {
            s = CardStamp::next_option(s);
            assert_eq!(s, expected);
        }

        let ctx = egui::Context::default();
        crate::rtl_text::install_arabic_fonts(&ctx);
        crate::theme::install(&ctx);

        let mut app = WhiteboardApp::from_data(sample_data());
        app.show_quick_entry_modal = false;
        app.data.current_day.panes.truncate(1);
        let node_id = app.data.current_day.panes[0].subtasks[0].id.clone();

        // Cycle stamp onto root card (None -> Dog) and verify monochrome slate watermark renders.
        app.cycle_node_stamp(&node_id);
        assert_eq!(
            app.data.current_day.panes[0].subtasks[0].stamp,
            Some(CardStamp::Dog)
        );

        let out = ctx.run(Default::default(), |ctx| app.draw(ctx));
        let expected_stroke = egui::epaint::ColorMode::Solid(stamp_watermark_stroke_color());
        let has_watermark_line = out.shapes.iter().any(|c| match &c.shape {
            egui::Shape::Path(p) => {
                p.stroke.color == expected_stroke && (p.stroke.width - 1.5).abs() < 0.05
            }
            _ => false,
        });
        assert!(
            has_watermark_line,
            "expected crisp 1.5px #94A3B8 geometric line-art watermark silhouette for stamped card"
        );

        // Verify all 5 geometric stamps render cleanly without panicking.
        for stamp in [
            CardStamp::Dog,
            CardStamp::Cat,
            CardStamp::Duck,
            CardStamp::Bird,
            CardStamp::Star,
        ] {
            app.data.current_day.panes[0].subtasks[0].stamp = Some(stamp);
            let out_s = ctx.run(Default::default(), |ctx| app.draw(ctx));
            assert!(
                out_s.shapes.iter().any(|c| match &c.shape {
                    egui::Shape::Path(p) => p.stroke.color == expected_stroke,
                    _ => false,
                }),
                "stamp {stamp:?} must paint visible #94A3B8 geometric line-art"
            );
        }

        // Also test Profiles modal opens cleanly from top-bar date pill.
        let date_label = format!("[ {}   ]", app.active_profile_date());
        assert!(
            rendered_text(&out).contains(&date_label),
            "expected top-bar profile date selector pill {date_label:?}"
        );
        app.show_profiles_modal = true;
        let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
        let out_profiles = ctx.run(Default::default(), |ctx| app.draw(ctx));
        let prof_text = rendered_text(&out_profiles);
        assert!(prof_text.contains("PROFILES"));
        assert!(prof_text.contains(&shape_arabic("+ بروفايل جديد")));
        assert!(prof_text.contains(&shape_arabic("ترحيل النشط من الأمس")));
    }

    #[test]
    fn top_bar_page_label_click_and_ctrl_arrows_flip_pages_with_zero_drag_conflict() {
        let ctx = egui::Context::default();
        crate::rtl_text::install_arabic_fonts(&ctx);
        crate::theme::install(&ctx);

        let mut data = AppData::default();
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
}
