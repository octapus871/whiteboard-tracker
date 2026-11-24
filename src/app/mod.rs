mod bottom_bar;
mod harbor;
mod index;
mod mind_map;
mod modals;
mod note_drawer;
mod rtl_edit;
mod top_bar;
#[cfg(test)]
mod tests;

use self::index::*;
use self::mind_map::*;
use self::note_drawer::*;
use self::rtl_edit::*;

use crate::i18n::{disp, format_carry_over_toast, tr};
use crate::models::{AppData, AppLang, DayProfile, MainTaskPane, TaskNode, TaskStatus};
use crate::rtl_text::shape_arabic;
use crate::storage::{
    backup_current_app_data, export_app_data_to_path, export_markdown_to_path,
    format_day_as_markdown, import_app_data_from_path, pick_open_file, pick_save_file,
    save_app_data, LoadError,
};
use crate::theme;
use egui::epaint::CubicBezierShape;
use egui::{
    pos2, vec2, Align, Button, Color32, CornerRadius, CursorIcon, FontId, Frame, Galley, Id, Key,
    Label, Layout, Margin, Painter, Pos2, Rect, RichText, ScrollArea, Sense, Shape, Stroke,
    StrokeKind, TextEdit, Ui, UiBuilder, Vec2,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

pub struct WhiteboardApp {
    pub data: AppData,
    pub show_quick_entry_modal: bool,
    pub quick_entry_input: String,
    /// Tasks staged via `Enter` inside the Quick Task Entry modal before `CREATE BOARD`.
    pub quick_entry_list: Vec<String>,
    pub focus_quick_entry: bool,
    /// Inline validation message (e.g. duplicate title rejected) inside Quick Task Entry.
    pub quick_entry_error: Option<String>,
    /// Index of the staged row currently being inline-edited in Quick Task Entry.
    pub quick_entry_editing_idx: Option<usize>,
    /// Backup of the staged row's title when entering inline edit so `Esc` cancels cleanly.
    pub quick_entry_edit_backup: Option<String>,
    /// Requests keyboard focus on a staged row's inline `TextEdit` next frame.
    pub quick_entry_focus_edit_idx: Option<usize>,

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
    /// Pending single-click on a card body awaiting the double-click window before
    /// opening its Note Drawer: `(node_id, open_at_time)`.
    pub pending_card_drawer_click: Option<(String, f64)>,

    /// Bounded undo stack (max 30 snapshots) for instant `Ctrl+Z` / `Cmd+Z`.
    pub undo_stack: Vec<AppSnapshot>,
    /// Subtle 4-second inline status bar notice: `(message, expires_at_time)`.
    pub undo_toast: Option<(String, f64)>,
    /// Latest egui frame timestamp for deterministic toast expiration.
    pub last_frame_time: f64,
    /// Card currently displaying its sticky contextual hover strip + last active timestamp
    /// (supports a 300ms grace timer so moving toward `✓ + ≡` never dismisses prematurely).
    pub active_hover_card: Option<(String, Instant)>,
    /// Parent nodes explicitly expanded by the user (immune to auto-collapse).
    pub manually_expanded_nodes: HashSet<String>,

    /// When true, the chronological Daily Profiles manager modal is open.
    pub show_profiles_modal: bool,
    /// Profile awaiting a 2nd click ("تأكيد؟") before deletion.
    pub profile_confirm_delete_id: Option<String>,
    /// Newly created profile ID whose inline title `TextEdit` should grab focus next frame.
    pub profile_focus_title_id: Option<String>,
    /// Date input (`YYYY-MM-DD`) in the Profiles modal for creating a profile on any date.
    pub profile_new_date_input: String,
    /// Inline date validation error in the Profiles modal.
    pub profile_date_error: Option<String>,
    /// Live search filter in the Harbor archive view.
    pub harbor_search_query: String,
    /// Archived pane awaiting a 2nd click ("CONFIRM?" / "تأكيد؟") before permanent deletion.
    pub harbor_confirm_delete_pane_id: Option<String>,
    /// When true, the calm Swiss First-Run Quick Guide modal is displayed.
    pub show_onboarding_modal: bool,
    /// When true, the Export / Import data modal is displayed.
    pub show_data_io_modal: bool,
    /// Validated JSON import awaiting explicit user confirmation before replacing current data.
    pub pending_import: Option<PendingImport>,
    /// Inline error message inside the Export / Import modal.
    pub data_io_error: Option<String>,
    /// Background native file-dialog result receiver (keeps UI responsive while picker is open).
    data_dialog_rx: Option<mpsc::Receiver<DataDialogResult>>,

    pub active_mode: AppMode,
    pub dirty: bool,
    /// Timestamp of the most recent mutation (`mark_dirty`) used for 400ms auto-save debouncing.
    pub last_dirty_at: Option<Instant>,
    /// Last persistence error; displayed in the bottom bar and cleared on the next successful save.
    pub save_error: Option<String>,
    /// Timestamp of the last failed save attempt (rate-limits retries to at most once every 5s).
    pub last_save_attempt: Option<Instant>,
    /// Blocking startup load error when `data.json` fails to read or parse.
    pub load_error: Option<LoadError>,
    /// Tracks whether the user copied the backup path from the load error modal.
    pub load_error_copied_path: bool,
}

/// Validated JSON backup ready to replace current data upon explicit confirmation.
#[derive(Debug, Clone)]
pub struct PendingImport {
    pub source_path: PathBuf,
    pub data: AppData,
}

enum DataDialogResult {
    ExportJson(Result<Option<PathBuf>, String>),
    ExportMarkdown(Result<Option<PathBuf>, String>),
    ImportJson(Result<Option<PathBuf>, String>),
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
const CARD_SINGLE_CLICK_DELAY_SECS: f64 = 0.30;
const AUTO_SAVE_DEBOUNCE: Duration = Duration::from_millis(400);
const SAVE_RETRY_INTERVAL: Duration = Duration::from_secs(5);
/// Grace delay before dismissing the contextual hover strip after the pointer leaves
/// the unified `card_rect.union(strip_rect).expand(8.0)` bridge zone.
const HOVER_GRACE_DURATION: Duration = Duration::from_millis(300);
/// Repaint cadence while the sticky contextual hover strip is active.
const HOVER_REPAINT_INTERVAL: Duration = Duration::from_millis(50);
/// Padded bridge expansion (`8.0px`) around `card_rect.union(strip_rect)`.
const HOVER_BRIDGE_PAD: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    LiveBoard,
    Harbor,
}

/// Strict maximum number of panes rendered on a single board page (2x2 grid).
const PANES_PER_PAGE: usize = 4;

impl WhiteboardApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        data: AppData,
        load_error: Option<LoadError>,
    ) -> Self {
        crate::rtl_text::install_arabic_fonts(&cc.egui_ctx);
        theme::install(&cc.egui_ctx);
        Self::from_data_with_load_error(data, load_error)
    }

    #[cfg(test)]
    pub fn from_data(data: AppData) -> Self {
        Self::from_data_with_load_error(data, None)
    }

    pub fn from_data_with_load_error(mut data: AppData, load_error: Option<LoadError>) -> Self {
        data.ensure_profiles_initialized();
        let show_onboarding_modal = !data.has_seen_onboarding;
        let show_quick_entry_modal = data.current_day.panes.is_empty() && !show_onboarding_modal;

        Self {
            data,
            show_quick_entry_modal,
            quick_entry_input: String::new(),
            quick_entry_list: Vec::new(),
            focus_quick_entry: show_quick_entry_modal,
            quick_entry_error: None,
            quick_entry_editing_idx: None,
            quick_entry_edit_backup: None,
            quick_entry_focus_edit_idx: None,
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
            pending_card_drawer_click: None,
            undo_stack: Vec::new(),
            undo_toast: None,
            last_frame_time: 0.0,
            active_hover_card: None,
            manually_expanded_nodes: HashSet::new(),
            show_profiles_modal: false,
            profile_confirm_delete_id: None,
            profile_focus_title_id: None,
            profile_new_date_input: chrono::Local::now().format("%Y-%m-%d").to_string(),
            profile_date_error: None,
            harbor_search_query: String::new(),
            harbor_confirm_delete_pane_id: None,
            show_onboarding_modal,
            show_data_io_modal: false,
            pending_import: None,
            data_io_error: None,
            data_dialog_rx: None,
            active_mode: AppMode::LiveBoard,
            dirty: false,
            last_dirty_at: None,
            save_error: None,
            last_save_attempt: None,
            load_error,
            load_error_copied_path: false,
        }
    }

    fn try_save_with<F>(&mut self, now: Instant, save_fn: F) -> bool
    where
        F: FnOnce(&AppData) -> Result<(), String>,
    {
        if !self.dirty || self.load_error.is_some() {
            return false;
        }
        self.data.sync_active_profile_from_current_day();
        match save_fn(&self.data) {
            Ok(()) => {
                self.dirty = false;
                self.last_dirty_at = None;
                self.save_error = None;
                self.last_save_attempt = None;
                true
            }
            Err(e) => {
                eprintln!("Error saving data: {}", e);
                self.save_error = Some(e);
                self.last_save_attempt = Some(now);
                false
            }
        }
    }

    fn auto_save_if_due_with<F>(
        &mut self,
        now: Instant,
        ctx: Option<&egui::Context>,
        save_fn: F,
    ) -> bool
    where
        F: FnOnce(&AppData) -> Result<(), String>,
    {
        if !self.dirty || self.load_error.is_some() {
            return false;
        }
        if self.save_error.is_some() {
            if let Some(last_attempt) = self.last_save_attempt {
                let elapsed = now.saturating_duration_since(last_attempt);
                if elapsed < SAVE_RETRY_INTERVAL {
                    if let Some(ctx) = ctx {
                        ctx.request_repaint_after(SAVE_RETRY_INTERVAL - elapsed);
                    }
                    return false;
                }
            }
        }
        if let Some(last_dirty) = self.last_dirty_at {
            let elapsed = now.saturating_duration_since(last_dirty);
            if elapsed < AUTO_SAVE_DEBOUNCE {
                if let Some(ctx) = ctx {
                    ctx.request_repaint_after(AUTO_SAVE_DEBOUNCE - elapsed);
                }
                return false;
            }
        }
        self.try_save_with(now, save_fn)
    }

    pub fn auto_save(&mut self, ctx: Option<&egui::Context>) {
        self.auto_save_if_due_with(Instant::now(), ctx, save_app_data);
    }

    /// Forces an immediate save (bypassing debounce and retry cooldown), used on app close.
    /// Refuses to overwrite `data.json` while a blocking `load_error` is still unresolved.
    pub fn force_save(&mut self) {
        self.try_save_with(Instant::now(), save_app_data);
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_dirty_at = Some(Instant::now());
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
        self.harbor_confirm_delete_pane_id = None;
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
            tr(self.active_lang()).toast_undone.to_string(),
            now + UNDO_TOAST_DURATION_SECS,
        ));
        self.data.sync_active_profile_from_current_day();
        self.mark_dirty();
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

    /// Returns the active UI language (`AppLang::Ar` by default until selected).
    pub fn active_lang(&self) -> AppLang {
        self.data.language.unwrap_or(AppLang::Ar)
    }

    /// Switches the active UI language live, re-anchors horizontal scroll offsets
    /// for the new reading direction, and schedules persistence.
    pub fn set_language(&mut self, lang: AppLang) {
        self.data.language = Some(lang);
        self.scroll_seeded.clear();
        self.mark_dirty();
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
        self.undo_stack.clear();
        self.undo_toast = None;
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
    #[cfg(test)]
    pub fn create_new_profile(&mut self, ctx: Option<&egui::Context>) -> String {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.create_profile_for_date(&today, ctx)
            .expect("today's date is always valid")
    }

    /// Creates a brand-new blank chronological daily profile for an arbitrary `YYYY-MM-DD`
    /// date (empty defaults to today), inserts it in chronological order (newest first),
    /// activates it, and queues focus on its inline title editor.
    pub fn create_profile_for_date(
        &mut self,
        date_str: &str,
        ctx: Option<&egui::Context>,
    ) -> Result<String, String> {
        let trimmed = date_str.trim();
        let today = chrono::Local::now().date_naive();
        let date = if trimmed.is_empty() {
            today
        } else {
            chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
                .map_err(|_| tr(self.active_lang()).profiles_invalid_date.to_string())?
        };

        self.data.sync_active_profile_from_current_day();
        let mut new_profile = DayProfile::new_for_date_in_lang(date, Vec::new(), self.active_lang());
        let target_date_str = new_profile.date_str.clone();
        // Keep same-date profiles newest-first; for today with no newer future dates, stay newest overall.
        let max_same_or_older_ts = self
            .data
            .profiles
            .iter()
            .filter(|p| p.date_str <= target_date_str)
            .map(|p| p.created_ts)
            .max();
        if let Some(max_ts) = max_same_or_older_ts {
            if new_profile.created_ts <= max_ts {
                new_profile.created_ts = max_ts + 1;
            }
        }
        let new_id = new_profile.id.clone();
        self.data.profiles.push(new_profile);
        self.data
            .profiles
            .sort_by(|a, b| b.created_ts.cmp(&a.created_ts));
        let new_idx = self
            .data
            .profiles
            .iter()
            .position(|p| p.id == new_id)
            .unwrap_or(0);
        self.load_profile_into_canvas(new_idx, ctx);
        self.profile_focus_title_id = Some(new_id.clone());
        self.profile_date_error = None;
        Ok(new_id)
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
        }
    }

    /// Returns the index of the immediately preceding (older) chronological profile,
    /// or `None` if no chronologically older profile exists. Never returns a newer profile.
    fn previous_profile_index(&self) -> Option<usize> {
        let active = self.data.profiles.get(self.data.active_profile_idx)?;
        let candidate_idx = self.data.active_profile_idx + 1;
        if let Some(candidate) = self.data.profiles.get(candidate_idx) {
            if candidate.created_ts <= active.created_ts {
                return Some(candidate_idx);
            }
        }
        self.data
            .profiles
            .iter()
            .enumerate()
            .filter(|(i, p)| *i != self.data.active_profile_idx && p.created_ts < active.created_ts)
            .max_by_key(|(_, p)| p.created_ts)
            .map(|(i, _)| i)
    }

    /// Copies only non-completed, non-cancelled active items from the chronologically
    /// previous profile into the active profile (never from a newer profile), skipping
    /// panes whose title already exists in the active profile so repeated clicks do not
    /// duplicate panes. Pushes an Undo snapshot first.
    pub fn carry_over_active_from_previous(&mut self, ctx: Option<&egui::Context>) -> usize {
        self.data.sync_active_profile_from_current_day();
        let Some(prev_idx) = self.previous_profile_index() else {
            return 0;
        };
        let mut existing_titles: HashSet<String> = self
            .data
            .current_day
            .panes
            .iter()
            .map(|p| p.title.trim().to_string())
            .collect();
        let carried: Vec<MainTaskPane> = self.data.profiles[prev_idx]
            .panes
            .iter()
            .filter_map(|pane| {
                let title_key = pane.title.trim();
                if existing_titles.contains(title_key) {
                    return None;
                }
                let cloned = pane.clone_active_only()?;
                existing_titles.insert(title_key.to_string());
                Some(cloned)
            })
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
            format_carry_over_toast(self.active_lang(), count),
            now + UNDO_TOAST_DURATION_SECS,
        ));
        self.mark_dirty();
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
            if self.harbor_confirm_delete_pane_id.as_deref() == Some(pane.id.as_str()) {
                self.harbor_confirm_delete_pane_id = None;
            }
            self.data.current_day.panes.push(pane);
            self.current_page = (self.data.current_day.panes.len() - 1) / PANES_PER_PAGE;
            self.mark_dirty();
        }
    }

    /// Permanently deletes an archived pane from Harbor.
    pub fn delete_archived_pane(&mut self, idx: usize) {
        if idx < self.data.archived_panes.len() {
            self.push_undo_snapshot();
            let removed = self.data.archived_panes.remove(idx);
            if self.harbor_confirm_delete_pane_id.as_deref() == Some(removed.id.as_str()) {
                self.harbor_confirm_delete_pane_id = None;
            }
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

    pub fn open_quick_entry_modal(&mut self) {
        self.show_quick_entry_modal = true;
        self.focus_quick_entry = true;
        self.quick_entry_error = None;
        self.quick_entry_editing_idx = None;
        self.quick_entry_edit_backup = None;
        self.quick_entry_focus_edit_idx = None;
    }

    fn close_quick_entry_modal(&mut self) {
        self.show_quick_entry_modal = false;
        self.quick_entry_input.clear();
        self.quick_entry_list.clear();
        self.focus_quick_entry = false;
        self.quick_entry_error = None;
        self.quick_entry_editing_idx = None;
        self.quick_entry_edit_backup = None;
        self.quick_entry_focus_edit_idx = None;
    }

    fn quick_entry_title_exists(&self, title: &str, ignore_staged_idx: Option<usize>) -> bool {
        let key = title.trim().to_lowercase();
        if key.is_empty() {
            return false;
        }
        self.data
            .current_day
            .panes
            .iter()
            .any(|p| p.title.trim().to_lowercase() == key)
            || self
                .quick_entry_list
                .iter()
                .enumerate()
                .any(|(i, s)| Some(i) != ignore_staged_idx && s.trim().to_lowercase() == key)
    }

    fn format_quick_entry_duplicate_error(&self, duplicates: &[String]) -> String {
        let prefix = tr(self.active_lang()).quick_entry_duplicate;
        format!("{} {}", prefix, duplicates.join(", "))
    }

    /// Commits an active inline edit on a staged Quick Task Entry row.
    /// Returns `true` if the edit finished (saved or restored empty), `false` if rejected as duplicate.
    fn commit_quick_entry_row_edit(&mut self, revert_on_duplicate: bool) -> bool {
        let Some(edit_idx) = self.quick_entry_editing_idx else {
            return true;
        };
        let edited = self
            .quick_entry_list
            .get(edit_idx)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if edited.is_empty() {
            if let Some(backup) = self.quick_entry_edit_backup.take() {
                if let Some(slot) = self.quick_entry_list.get_mut(edit_idx) {
                    *slot = backup;
                }
            }
            self.quick_entry_editing_idx = None;
            self.quick_entry_focus_edit_idx = None;
            self.quick_entry_error = None;
            return true;
        }
        if self.quick_entry_title_exists(&edited, Some(edit_idx)) {
            self.quick_entry_error = Some(self.format_quick_entry_duplicate_error(&[edited]));
            if revert_on_duplicate {
                if let Some(backup) = self.quick_entry_edit_backup.take() {
                    if let Some(slot) = self.quick_entry_list.get_mut(edit_idx) {
                        *slot = backup;
                    }
                }
                self.quick_entry_editing_idx = None;
                self.quick_entry_focus_edit_idx = None;
            }
            return false;
        }
        if let Some(slot) = self.quick_entry_list.get_mut(edit_idx) {
            *slot = edited;
        }
        self.quick_entry_editing_idx = None;
        self.quick_entry_edit_backup = None;
        self.quick_entry_focus_edit_idx = None;
        self.quick_entry_error = None;
        true
    }

    fn cancel_quick_entry_row_edit(&mut self) {
        if let Some(edit_idx) = self.quick_entry_editing_idx.take() {
            if let Some(backup) = self.quick_entry_edit_backup.take() {
                if let Some(slot) = self.quick_entry_list.get_mut(edit_idx) {
                    *slot = backup;
                }
            }
        }
        self.quick_entry_focus_edit_idx = None;
        self.quick_entry_error = None;
    }

    /// Splits `quick_entry_input` by `/` (and newlines), trims non-empty entries,
    /// rejects titles that already exist in `quick_entry_list` or today's panes,
    /// appends unique entries into `quick_entry_list`, and clears the input buffer
    /// without creating panes or closing the modal.
    pub fn stage_quick_entry_input(&mut self) -> usize {
        let candidates: Vec<String> = self
            .quick_entry_input
            .split(&['/', '\n'][..])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        if candidates.is_empty() {
            self.quick_entry_input.clear();
            return 0;
        }

        let mut added = 0usize;
        let mut duplicates: Vec<String> = Vec::new();
        for title in candidates {
            if self.quick_entry_title_exists(&title, None) {
                duplicates.push(title);
            } else {
                self.quick_entry_list.push(title);
                added += 1;
            }
        }

        if duplicates.is_empty() {
            self.quick_entry_error = None;
            self.quick_entry_input.clear();
        } else {
            self.quick_entry_error = Some(self.format_quick_entry_duplicate_error(&duplicates));
            if added > 0 {
                self.quick_entry_input.clear();
            }
        }
        added
    }

    /// Creates one main task pane per unique staged item in `quick_entry_list` (plus any
    /// remaining unique `/`-separated items in `quick_entry_input`), rejects duplicates
    /// with an inline error, saves state, and closes the modal. No-ops cleanly if empty.
    pub fn process_quick_entry(&mut self) {
        if self.quick_entry_editing_idx.is_some() && !self.commit_quick_entry_row_edit(false) {
            return;
        }

        let trailing: Vec<String> = self
            .quick_entry_input
            .split(&['/', '\n'][..])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        let mut duplicates: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = self
            .data
            .current_day
            .panes
            .iter()
            .map(|p| p.title.trim().to_lowercase())
            .collect();

        let mut all_tasks: Vec<String> = Vec::new();
        for title in self.quick_entry_list.iter().cloned().chain(trailing) {
            let trimmed = title.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            let key = trimmed.to_lowercase();
            if seen.contains(&key) {
                duplicates.push(trimmed);
            } else {
                seen.insert(key);
                all_tasks.push(trimmed);
            }
        }

        if !duplicates.is_empty() {
            self.quick_entry_error = Some(self.format_quick_entry_duplicate_error(&duplicates));
            if all_tasks.is_empty() {
                return;
            }
        }

        if all_tasks.is_empty() {
            return;
        }

        self.push_undo_snapshot();
        for title in all_tasks {
            self.data.current_day.panes.push(MainTaskPane::new(&title));
        }

        if !self.data.current_day.panes.is_empty() {
            self.current_page = (self.data.current_day.panes.len() - 1) / PANES_PER_PAGE;
        }

        self.close_quick_entry_modal();
        self.mark_dirty();
    }

    // ---------------------------------------------------------------------
    // Keyboard outlining
    // ---------------------------------------------------------------------

    fn handle_keyboard_navigation(&mut self, ctx: &egui::Context) {
        if self.data.language.is_none()
            || self.show_quick_entry_modal
            || self.show_profiles_modal
            || self.show_onboarding_modal
            || self.show_data_io_modal
        {
            return;
        }

        // On HOME, PageUp / PageDown and Alt+Left/Right flip pages immediately even
        // while a live input has focus (unless inline-renaming a card title).
        // Ctrl+Left/Right only flips pages when no TextEdit has keyboard focus so
        // word-by-word caret navigation works normally inside text inputs.
        let on_home = self.active_mode == AppMode::LiveBoard && !self.show_master_index;
        let text_has_focus = ctx.wants_keyboard_input();
        let (page_up, page_down, alt_left, alt_right, ctrl_left, ctrl_right) = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command;
            let alt = i.modifiers.alt;
            (
                i.key_pressed(Key::PageUp),
                i.key_pressed(Key::PageDown),
                alt && i.key_pressed(Key::ArrowLeft),
                alt && i.key_pressed(Key::ArrowRight),
                ctrl && i.key_pressed(Key::ArrowLeft),
                ctrl && i.key_pressed(Key::ArrowRight),
            )
        });
        if on_home && self.editing_node_id.is_none() {
            if page_up || alt_left || (!text_has_focus && ctrl_left) {
                self.prev_page();
                return;
            }
            if page_down || alt_right || (!text_has_focus && ctrl_right) {
                self.next_page();
                return;
            }
        }

        if text_has_focus {
            return;
        }

        if page_up || alt_left || ctrl_left {
            self.prev_page();
            return;
        }
        if page_down || alt_right || ctrl_right {
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

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.force_save();
    }
}

impl WhiteboardApp {
    /// Draws the whole application. Separated from [`eframe::App::update`] so it
    /// can be exercised headlessly in tests.
    pub fn draw(&mut self, ctx: &egui::Context) {
        // Keep the live digital clock ticking at one frame per second.
        ctx.request_repaint_after(Duration::from_secs(1));

        self.poll_data_dialog(ctx);

        if ctx.input(|i| i.viewport().close_requested()) {
            self.force_save();
        }

        let now = ctx.input(|i| i.time);
        self.last_frame_time = now;
        if let Some((_, expires_at)) = &self.undo_toast {
            if now >= *expires_at {
                self.undo_toast = None;
            }
        }
        if let Some((pending_node_id, open_at)) = self.pending_card_drawer_click.clone() {
            let blocked = self.editing_node_id.is_some()
                || self.load_error.is_some()
                || self.data.language.is_none()
                || self.show_quick_entry_modal
                || self.show_profiles_modal
                || self.show_onboarding_modal
                || self.show_data_io_modal
                || self.show_master_index
                || self.active_mode != AppMode::LiveBoard;
            if blocked {
                self.pending_card_drawer_click = None;
            } else if now >= open_at {
                self.pending_card_drawer_click = None;
                self.open_note_drawer(pending_node_id, false);
            } else {
                ctx.request_repaint_after(Duration::from_secs_f64((open_at - now).max(0.01)));
            }
        }

        // Escape always works - even while a text field has focus - so an
        // edit can be cancelled (and an empty new node removed again).
        let has_load_error = self.load_error.is_some();
        let needs_language = self.data.language.is_none();
        if has_load_error || needs_language {
            // Block all shortcuts until a load error or first-launch language choice is resolved.
        } else if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.pending_card_drawer_click = None;
            if self.show_data_io_modal {
                if self.pending_import.is_some() {
                    self.pending_import = None;
                    self.data_io_error = None;
                } else if self.data_dialog_rx.is_none() {
                    self.show_data_io_modal = false;
                    self.data_io_error = None;
                }
            } else if self.show_profiles_modal {
                self.show_profiles_modal = false;
                self.profile_confirm_delete_id = None;
                self.profile_date_error = None;
            } else if self.show_onboarding_modal {
                self.show_onboarding_modal = false;
                self.data.has_seen_onboarding = true;
                if self.data.current_day.panes.is_empty() {
                    self.open_quick_entry_modal();
                }
                self.mark_dirty();
            } else if self.show_quick_entry_modal {
                if self.quick_entry_editing_idx.is_some() {
                    self.cancel_quick_entry_row_edit();
                } else {
                    self.close_quick_entry_modal();
                }
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
            } else if self.harbor_confirm_delete_pane_id.is_some() {
                self.harbor_confirm_delete_pane_id = None;
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
            let quick_entry_shortcut = ctx.input(|i| {
                let cmd = i.modifiers.ctrl || i.modifiers.command;
                cmd && !i.modifiers.shift && !i.modifiers.alt && i.key_pressed(Key::N)
            });
            if quick_entry_shortcut
                && !self.show_quick_entry_modal
                && !self.show_profiles_modal
                && !self.show_onboarding_modal
                && !self.show_data_io_modal
            {
                self.open_quick_entry_modal();
            } else if !ctx.wants_keyboard_input() {
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
                    self.force_save();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else if zen_toggle {
                    self.is_zen = !self.is_zen;
                } else if undo_shortcut
                    && !self.show_quick_entry_modal
                    && !self.show_profiles_modal
                    && !self.show_onboarding_modal
                    && !self.show_data_io_modal
                {
                    self.undo(Some(ctx));
                }
            }

            if !self.show_master_index
                && !self.show_profiles_modal
                && !self.show_onboarding_modal
                && !self.show_data_io_modal
            {
                self.handle_pane_hotkeys(ctx);
            }
        }

        self.clamp_current_page();
        if !has_load_error {
            self.handle_keyboard_navigation(ctx);
        }

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
                .frame(Frame::new().fill(theme::PANE).inner_margin(Margin::same(0)))
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

        if has_load_error {
            self.render_load_error_modal(ctx);
        } else if needs_language {
            self.render_language_selector_modal(ctx);
        } else {
            if self.show_quick_entry_modal && !self.show_onboarding_modal {
                self.render_quick_entry_modal(ctx);
            }
            if self.show_profiles_modal {
                self.render_profiles_modal(ctx);
            }
            if self.show_onboarding_modal {
                self.render_onboarding_modal(ctx);
            }
            if self.show_data_io_modal {
                self.render_data_io_modal(ctx);
            }
        }

        if self.dirty && self.load_error.is_none() {
            self.auto_save(Some(ctx));
        }
    }
}

impl WhiteboardApp {
    fn render_daily_canvas(&mut self, ui: &mut Ui) {
        let lang = self.active_lang();
        let s = tr(lang);
        let is_rtl = lang.is_rtl();
        let panes_count = self.data.current_day.panes.len();
        if panes_count == 0 {
            let (_, date) = clock_strings_in_lang(lang);
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                Frame::new()
                    .fill(theme::PANE)
                    .stroke(Stroke::new(theme::GRID_LINE, theme::GRID))
                    .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
                    .inner_margin(Margin::same(28))
                    .show(ui, |ui| {
                        if is_rtl {
                            ui.label(
                                RichText::new(disp(&date))
                                    .size(18.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                        } else {
                            ui.label(mono(&date, 18.0).strong());
                        }
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(disp(s.empty_board_title))
                                .size(12.5)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(14.0);
                        if !self.is_zen
                            && pill_button(ui, &disp(s.empty_board_add), theme::GRID, true)
                                .clicked()
                        {
                            self.show_quick_entry_modal = true;
                            self.quick_entry_input.clear();
                            self.quick_entry_list.clear();
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
                    let tiles =
                        compute_page_grid_layout_dir(&page_indices, width, viewport_height, is_rtl);
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
                let any_modal_open = self.data.language.is_none()
                    || self.show_quick_entry_modal
                    || self.show_profiles_modal
                    || self.show_onboarding_modal
                    || self.show_data_io_modal;
                let prev_hovered_card: Option<String> =
                    if self.is_zen || self.editing_node_id.is_some() || any_modal_open {
                        None
                    } else {
                        self.active_hover_card.as_ref().and_then(|(id, since)| {
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
                        lang,
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
        let any_modal_open = self.data.language.is_none()
            || self.show_quick_entry_modal
            || self.show_profiles_modal
            || self.show_onboarding_modal
            || self.show_data_io_modal;
        if self.is_zen || self.editing_node_id.is_some() || any_modal_open {
            self.active_hover_card = None;
        } else {
            let prev_id = self.active_hover_card.as_ref().map(|(id, _)| id.as_str());
            let winner = actions
                .hover_controls
                .iter()
                .max_by_key(|c| (c.priority, prev_id == Some(c.node_id.as_str())))
                .cloned();
            if let Some(control) = winner {
                let (strip_hovered, strip_acted) = render_foreground_hover_strip(
                    ui.ctx(),
                    &control,
                    lang,
                    &mut self.selected_node_ids,
                    &mut self.focused_node_id,
                    &mut actions,
                );
                if strip_acted {
                    // Foreground strip click must never also open an underlying card's drawer.
                    actions.select_open_drawer = None;
                    self.pending_card_drawer_click = None;
                }
                if control.priority >= 1 || strip_hovered {
                    self.active_hover_card = Some((control.node_id, hover_now));
                    ui.ctx().request_repaint_after(HOVER_REPAINT_INTERVAL);
                } else if let Some((_, since)) = &self.active_hover_card {
                    let elapsed = hover_now.saturating_duration_since(*since);
                    if elapsed >= HOVER_GRACE_DURATION {
                        self.active_hover_card = None;
                    } else {
                        ui.ctx().request_repaint_after(HOVER_REPAINT_INTERVAL);
                    }
                }
            } else {
                self.active_hover_card = None;
            }
        }

        if actions.dirty {
            self.dirty = true;
        }

        if let Some(idx) = actions.delete_pane {
            self.pending_card_drawer_click = None;
            self.delete_pane_at(idx, Some(ui.ctx()));
        }

        if let Some((node_id, status)) = actions.set_node_status {
            self.set_single_node_status(&node_id, status);
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
            self.pending_card_drawer_click = None;
            self.delete_node(&target_id);
        }
        if let Some(node_id) = actions.focus_node {
            self.next_focus_node_id = Some(node_id);
            self.pending_card_drawer_click = None;
        }
        if self.editing_node_id.is_some() {
            self.pending_card_drawer_click = None;
        }
        if let Some(node_id) = actions.select_open_drawer {
            if self.editing_node_id.is_none() {
                let now = ui.ctx().input(|i| i.time);
                self.pending_card_drawer_click =
                    Some((node_id, now + CARD_SINGLE_CLICK_DELAY_SECS));
                ui.ctx()
                    .request_repaint_after(Duration::from_secs_f64(CARD_SINGLE_CLICK_DELAY_SECS));
            }
        }
        if let Some(node_id) = actions.focus_open_drawer {
            self.pending_card_drawer_click = None;
            self.open_note_drawer(node_id, true);
        }
        if let Some(pane_id) = actions.close_drawer_pane {
            self.pending_card_drawer_click = None;
            self.close_note_drawer_for_pane(&pane_id, Some(ui.ctx()));
        }

        if finish_lasso {
            self.is_lassoing = false;
            self.lasso_start = None;
            self.lasso_current = None;
        }
    }

    /// Slim whiteboard header used in Zen mode: digital clock + localized date
    /// over a 1px charcoal rule, nothing else.
    fn render_zen_header(&mut self, ui: &mut Ui) {
        let lang = self.active_lang();
        let s = tr(lang);
        let (time, _date) = clock_strings_in_lang(lang);
        let short_date = short_date_string_in_lang(lang);
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
                ui.label(
                    RichText::new(disp(s.zen_badge))
                        .size(14.0)
                        .strong()
                        .color(theme::SLATE),
                );
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
    /// Append a top-level branch to a pane with undo history tracking.
    append_branch: Option<(String, String)>,
    /// Open the embedded note drawer on card click without stealing card focus.
    select_open_drawer: Option<String>,
    /// Open the embedded note drawer and immediately focus the notes editor.
    focus_open_drawer: Option<String>,
    /// Close the embedded note drawer for the specified pane ID.
    close_drawer_pane: Option<String>,
    /// Candidate contextual hover strips collected across all visible panes,
    /// rendered on `Order::Foreground` so they are never clipped or blocked.
    hover_controls: Vec<HoverControl>,
    node_rects: Vec<(String, Rect)>,
    pane_header_rects: Vec<(String, Rect)>,
    dirty: bool,
    /// Node that must grab keyboard focus on the next frame (carried over).
    focus_node: Option<String>,
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
#[cfg(test)]
fn compute_page_grid_layout(
    page_indices: &[usize],
    width: f32,
    height: f32,
) -> Vec<TilePlacement> {
    compute_page_grid_layout_dir(page_indices, width, height, true)
}

fn compute_page_grid_layout_dir(
    page_indices: &[usize],
    width: f32,
    height: f32,
    is_rtl: bool,
) -> Vec<TilePlacement> {
    let n = page_indices.len().min(PANES_PER_PAGE);
    let mut out = Vec::with_capacity(n);
    let first_x = |half_w: f32| if is_rtl { half_w } else { 0.0 };
    let second_x = |half_w: f32| if is_rtl { 0.0 } else { half_w };
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
                pos: pos2(first_x(half_w), 0.0),
                size: vec2(half_w, height),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[1],
                pos: pos2(second_x(half_w), 0.0),
                size: vec2(half_w, height),
            });
        }
        3 => {
            let half_w = width * 0.5;
            let half_h = height * 0.5;
            out.push(TilePlacement {
                pane_idx: page_indices[0],
                pos: pos2(first_x(half_w), 0.0),
                size: vec2(half_w, height),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[1],
                pos: pos2(second_x(half_w), 0.0),
                size: vec2(half_w, half_h),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[2],
                pos: pos2(second_x(half_w), half_h),
                size: vec2(half_w, half_h),
            });
        }
        _ => {
            let half_w = width * 0.5;
            let half_h = height * 0.5;
            out.push(TilePlacement {
                pane_idx: page_indices[0],
                pos: pos2(first_x(half_w), 0.0),
                size: vec2(half_w, half_h),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[1],
                pos: pos2(second_x(half_w), 0.0),
                size: vec2(half_w, half_h),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[2],
                pos: pos2(first_x(half_w), half_h),
                size: vec2(half_w, half_h),
            });
            out.push(TilePlacement {
                pane_idx: page_indices[3],
                pos: pos2(second_x(half_w), half_h),
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

/// A bounded-height directional row (`RTL` for Arabic, `LTR` for English).
fn dir_row<R>(
    ui: &mut Ui,
    height: f32,
    is_rtl: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let size = Vec2::new(ui.available_width(), height);
    let layout = if is_rtl {
        Layout::right_to_left(Align::Center)
    } else {
        Layout::left_to_right(Align::Center)
    };
    ui.allocate_ui_with_layout(size, layout, add_contents)
}

/// Pointer position helper used by the lasso selection.
fn ctx_pointer_pos(ui: &Ui) -> Option<Pos2> {
    ui.input(|i| i.pointer.hover_pos())
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

/// Draws a high-contrast Swiss `0.0px` keyboard focus ring around `rect`.
fn draw_focus_ring(painter: &Painter, rect: Rect) {
    painter.rect_stroke(
        rect.shrink(1.0),
        CornerRadius::same(0),
        Stroke::new(2.0, theme::AMBER_INK),
        StrokeKind::Inside,
    );
}

fn arabic_weekday_name(weekday: chrono::Weekday) -> &'static str {
    match weekday {
        chrono::Weekday::Sat => "السبت",
        chrono::Weekday::Sun => "الأحد",
        chrono::Weekday::Mon => "الاثنين",
        chrono::Weekday::Tue => "الثلاثاء",
        chrono::Weekday::Wed => "الأربعاء",
        chrono::Weekday::Thu => "الخميس",
        chrono::Weekday::Fri => "الجمعة",
    }
}

fn arabic_month_name(month: u32) -> &'static str {
    match month {
        1 => "يناير",
        2 => "فبراير",
        3 => "مارس",
        4 => "أبريل",
        5 => "مايو",
        6 => "يونيو",
        7 => "يوليو",
        8 => "أغسطس",
        9 => "سبتمبر",
        10 => "أكتوبر",
        11 => "نوفمبر",
        _ => "ديسمبر",
    }
}

/// Live localized digital clock strings (`time`, `full_date`).
fn clock_strings_in_lang(lang: AppLang) -> (String, String) {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    match lang {
        AppLang::En => {
            let time = now.format("%I:%M:%S %p").to_string();
            let date = now.format("%a, %b %d, %Y").to_string().to_uppercase();
            (time, date)
        }
        AppLang::Ar => {
            let (is_pm, hour12) = now.hour12();
            let suffix = if is_pm { "م" } else { "ص" };
            let time = format!(
                "{:02}:{:02}:{:02} {}",
                hour12,
                now.minute(),
                now.second(),
                suffix
            );
            let date = format!(
                "{}، {} {} {}",
                arabic_weekday_name(now.weekday()),
                now.day(),
                arabic_month_name(now.month()),
                now.year()
            );
            (time, date)
        }
    }
}

/// Compact localized date for the top bar clock (`MON, NOV 23` or `الاثنين، 23 نوفمبر`).
fn short_date_string_in_lang(lang: AppLang) -> String {
    use chrono::Datelike;
    let now = chrono::Local::now();
    match lang {
        AppLang::En => now.format("%a, %b %d").to_string().to_uppercase(),
        AppLang::Ar => format!(
            "{}، {} {}",
            arabic_weekday_name(now.weekday()),
            now.day(),
            arabic_month_name(now.month())
        ),
    }
}

/// Live digital clock strings in English (kept for tests).
#[cfg(test)]
fn clock_strings() -> (String, String) {
    clock_strings_in_lang(AppLang::En)
}

/// Compact English date for tests.
#[cfg(test)]
fn short_date_string() -> String {
    short_date_string_in_lang(AppLang::En)
}

/// Monospace technical text used for all UI chrome.
fn mono(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::monospace(size))
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
