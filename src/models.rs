use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The three-state task system: active → completed → cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskStatus {
    /// In progress (amber hard shadow).
    #[default]
    #[serde(alias = "Yellow", alias = "Default")]
    Active,
    /// Done (emerald hard shadow).
    #[serde(alias = "Green")]
    Completed,
    /// Cancelled (red hard shadow, strikethrough).
    #[serde(alias = "Red")]
    Cancelled,
}

impl TaskStatus {
    /// Cycles through the three states:
    /// active → completed → cancelled → active.
    pub fn next(self) -> Self {
        match self {
            TaskStatus::Active => TaskStatus::Completed,
            TaskStatus::Completed => TaskStatus::Cancelled,
            TaskStatus::Cancelled => TaskStatus::Active,
        }
    }
}

/// Active UI language (`Ar` = Arabic RTL, `En` = English LTR).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppLang {
    Ar,
    En,
}

impl AppLang {
    pub fn is_rtl(self) -> bool {
        matches!(self, Self::Ar)
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Ar => Self::En,
            Self::En => Self::Ar,
        }
    }
}

/// A single task node in the outline. Nodes can nest arbitrarily deep, which is
/// what powers the `Enter` (sibling) / `Tab` (child) / `Shift+Tab` (outdent)
/// outlining workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "TaskNodeWire")]
pub struct TaskNode {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub children: Vec<TaskNode>,
}

/// Deserialization wire format that transparently migrates legacy `is_expanded`
/// (and/or `collapsed`) fields from older `data.json` files into a single `collapsed` bool.
#[derive(Deserialize)]
struct TaskNodeWire {
    id: String,
    title: String,
    status: TaskStatus,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    collapsed: Option<bool>,
    #[serde(default)]
    is_expanded: Option<bool>,
    #[serde(default)]
    children: Vec<TaskNode>,
}

impl From<TaskNodeWire> for TaskNode {
    fn from(raw: TaskNodeWire) -> Self {
        let collapsed = raw.collapsed.unwrap_or(false) || !raw.is_expanded.unwrap_or(true);
        Self {
            id: raw.id,
            title: raw.title,
            status: raw.status,
            notes: raw.notes,
            collapsed,
            children: raw.children,
        }
    }
}

impl TaskNode {
    pub fn new(title: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            status: TaskStatus::Active,
            notes: String::new(),
            collapsed: false,
            children: Vec::new(),
        }
    }

    /// Deep-clones only active (non-Completed, non-Cancelled) branches with fresh UUIDs
    /// for carrying tasks forward into a new daily profile.
    pub fn clone_active_only(&self) -> Option<Self> {
        if self.status != TaskStatus::Active {
            return None;
        }
        let active_children: Vec<TaskNode> = self
            .children
            .iter()
            .filter_map(TaskNode::clone_active_only)
            .collect();
        Some(Self {
            id: Uuid::new_v4().to_string(),
            title: self.title.clone(),
            status: TaskStatus::Active,
            notes: self.notes.clone(),
            collapsed: self.collapsed,
            children: active_children,
        })
    }

    /// True when this node's child branches are visually folded.
    pub fn is_folded(&self) -> bool {
        self.collapsed
    }

    /// Total number of descendant nodes hidden beneath this node.
    pub fn descendant_count(&self) -> usize {
        self.children
            .iter()
            .map(|child| 1 + child.descendant_count())
            .sum()
    }

    /// True when this node has children and 100% of its descendants are Completed.
    pub fn all_children_completed(&self) -> bool {
        !self.children.is_empty()
            && self.children.iter().all(|child| {
                child.status == TaskStatus::Completed
                    && (child.children.is_empty() || child.all_children_completed())
            })
    }

    /// Accumulates leaf-only task counts into `counts`.
    /// Non-leaf parent nodes delegate to their children so tasks are never double-counted.
    pub fn count_statuses(&self, counts: &mut StatusCounts) {
        if self.children.is_empty() {
            counts.total += 1;
            match self.status {
                TaskStatus::Active => counts.active_count += 1,
                TaskStatus::Completed => counts.completed_count += 1,
                TaskStatus::Cancelled => counts.cancelled_count += 1,
            }
        } else {
            for child in &self.children {
                child.count_statuses(counts);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainTaskPane {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub subtasks: Vec<TaskNode>,
}

impl MainTaskPane {
    pub fn new(title: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            status: TaskStatus::Active,
            subtasks: Vec::new(),
        }
    }

    /// Deep-clones this pane with fresh UUIDs keeping only active (non-Completed,
    /// non-Cancelled) subtasks. Returns `None` if the pane itself is not Active
    /// or if it had subtasks and all of them are completed/cancelled.
    pub fn clone_active_only(&self) -> Option<Self> {
        if self.status != TaskStatus::Active {
            return None;
        }
        let had_subtasks = !self.subtasks.is_empty();
        let active_subtasks: Vec<TaskNode> = self
            .subtasks
            .iter()
            .filter_map(TaskNode::clone_active_only)
            .collect();
        if had_subtasks && active_subtasks.is_empty() {
            return None;
        }
        Some(Self {
            id: Uuid::new_v4().to_string(),
            title: self.title.clone(),
            status: TaskStatus::Active,
            subtasks: active_subtasks,
        })
    }

    /// Accumulates leaf-only subtask counts for this pane (the pane container itself is not counted).
    pub fn count_statuses(&self, counts: &mut StatusCounts) {
        for sub in &self.subtasks {
            sub.count_statuses(counts);
        }
    }

    /// Returns `(completed_leaves, actionable_leaves)` where cancelled leaves are excluded
    /// from the denominator.
    pub fn progress_counts(&self) -> (usize, usize) {
        let mut counts = StatusCounts::default();
        self.count_statuses(&mut counts);
        counts.progress_counts()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayBoard {
    pub date_iso: String,
    pub panes: Vec<MainTaskPane>,
}

impl Default for DayBoard {
    fn default() -> Self {
        let now = chrono::Local::now();
        let date_iso = now.format("%Y-%m-%d").to_string();
        Self {
            date_iso,
            panes: Vec::new(),
        }
    }
}

impl DayBoard {
    pub fn calculate_counts(&self) -> StatusCounts {
        let mut counts = StatusCounts::default();
        for pane in &self.panes {
            pane.count_statuses(&mut counts);
        }
        counts
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusCounts {
    /// Completed tasks.
    #[serde(rename = "completed_count", alias = "green_count", default)]
    pub completed_count: usize,
    /// Active (in progress) tasks.
    #[serde(rename = "active_count", alias = "yellow_count", default)]
    pub active_count: usize,
    /// Cancelled tasks.
    #[serde(rename = "cancelled_count", alias = "red_count", default)]
    pub cancelled_count: usize,
    #[serde(default)]
    pub total: usize,
    /// Legacy "planned" counter, kept only so older data files still load.
    #[serde(
        rename = "legacy_planned",
        alias = "default_count",
        default,
        skip_serializing
    )]
    #[allow(dead_code)]
    pub legacy_planned: usize,
}

impl StatusCounts {
    /// Actionable leaf task count used as the progress denominator (`total - cancelled`).
    pub fn actionable_total(&self) -> usize {
        self.total.saturating_sub(self.cancelled_count)
    }

    /// Returns `(completed, actionable_total)` for consistent `done/total` progress display.
    pub fn progress_counts(&self) -> (usize, usize) {
        (self.completed_count, self.actionable_total())
    }

    /// Completion % = completed / (total - cancelled) * 100.
    /// Cancelled items are excluded from the denominator.
    pub fn completion_rate(&self) -> f32 {
        let denominator = self.actionable_total();
        if denominator == 0 {
            0.0
        } else {
            (self.completed_count as f32 / denominator as f32) * 100.0
        }
    }
}

/// A chronological daily workspace profile holding a day's panes and tasks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DayProfile {
    pub id: String,
    /// ISO date string, e.g. `"2026-11-22"`.
    pub date_str: String,
    /// Editable RTL profile title, e.g. `"الأحد — مهام اليوم"`.
    pub title: String,
    /// Unix timestamp for chronological sorting (newest first).
    pub created_ts: i64,
    pub panes: Vec<MainTaskPane>,
}

impl DayProfile {
    pub fn arabic_weekday_title(weekday: chrono::Weekday) -> String {
        let day_ar = match weekday {
            chrono::Weekday::Sat => "السبت",
            chrono::Weekday::Sun => "الأحد",
            chrono::Weekday::Mon => "الاثنين",
            chrono::Weekday::Tue => "الثلاثاء",
            chrono::Weekday::Wed => "الأربعاء",
            chrono::Weekday::Thu => "الخميس",
            chrono::Weekday::Fri => "الجمعة",
        };
        format!("{day_ar} — مهام اليوم")
    }

    pub fn title_for_weekday(weekday: chrono::Weekday, lang: AppLang) -> String {
        match lang {
            AppLang::Ar => Self::arabic_weekday_title(weekday),
            AppLang::En => {
                let day_en = match weekday {
                    chrono::Weekday::Sat => "Saturday",
                    chrono::Weekday::Sun => "Sunday",
                    chrono::Weekday::Mon => "Monday",
                    chrono::Weekday::Tue => "Tuesday",
                    chrono::Weekday::Wed => "Wednesday",
                    chrono::Weekday::Thu => "Thursday",
                    chrono::Weekday::Fri => "Friday",
                };
                format!("{day_en} — Today's Tasks")
            }
        }
    }

    pub fn new_for_date_in_lang(
        date: chrono::NaiveDate,
        panes: Vec<MainTaskPane>,
        lang: AppLang,
    ) -> Self {
        use chrono::Datelike;
        let now = chrono::Local::now();
        let created_ts = if date == now.date_naive() {
            now.timestamp()
        } else {
            date.and_hms_opt(12, 0, 0)
                .map(|dt| dt.and_utc().timestamp())
                .unwrap_or_else(|| now.timestamp())
        };
        Self {
            id: Uuid::new_v4().to_string(),
            date_str: date.format("%Y-%m-%d").to_string(),
            title: Self::title_for_weekday(date.weekday(), lang),
            created_ts,
            panes,
        }
    }

    /// Returns `(completed_leaves, actionable_leaves)` across all panes in this profile
    /// (leaf subtasks only; cancelled subtasks excluded from the denominator).
    pub fn completion_counts(&self) -> (usize, usize) {
        let mut counts = StatusCounts::default();
        for pane in &self.panes {
            pane.count_statuses(&mut counts);
        }
        counts.progress_counts()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppData {
    pub current_day: DayBoard,
    #[serde(default)]
    pub archived_panes: Vec<MainTaskPane>,
    #[serde(default)]
    pub profiles: Vec<DayProfile>,
    #[serde(default)]
    pub active_profile_idx: usize,
    #[serde(default)]
    pub has_seen_onboarding: bool,
    #[serde(default)]
    pub language: Option<AppLang>,
}

impl AppData {
    /// Migrates legacy `current_day.panes` into `profiles` if needed, sorts
    /// profiles chronologically (`created_ts` descending, newest first), and
    /// ensures a profile exists for `today_date` (`YYYY-MM-DD`).
    pub fn ensure_profiles_for_date(&mut self, today_date: &str, now_ts: i64, today_title: String) {
        // 1. Safe backward-compatible legacy migration when `profiles` is empty.
        if self.profiles.is_empty() {
            let legacy_panes = self.current_day.panes.clone();
            self.profiles.push(DayProfile {
                id: Uuid::new_v4().to_string(),
                date_str: today_date.to_string(),
                title: today_title.clone(),
                created_ts: now_ts,
                panes: legacy_panes,
            });
            self.active_profile_idx = 0;
        }

        // Remember active profile ID across chronological sort.
        let prev_active_id = self
            .profiles
            .get(self.active_profile_idx)
            .map(|p| p.id.clone());

        // 2. Sort chronologically (newest first: created_ts descending).
        self.profiles
            .sort_by(|a, b| b.created_ts.cmp(&a.created_ts));

        // 3. On startup, if no profile matches today's local date, prepend a new empty
        //    profile for today (never duplicating yesterday's tasks without consent).
        let has_today = self.profiles.iter().any(|p| p.date_str == today_date);
        if !has_today {
            let max_ts = self
                .profiles
                .iter()
                .map(|p| p.created_ts)
                .max()
                .unwrap_or(now_ts);
            let new_ts = now_ts.max(max_ts + 1);
            self.profiles.insert(
                0,
                DayProfile {
                    id: Uuid::new_v4().to_string(),
                    date_str: today_date.to_string(),
                    title: today_title,
                    created_ts: new_ts,
                    panes: Vec::new(),
                },
            );
            self.active_profile_idx = 0;
        } else if let Some(active_id) = prev_active_id {
            self.active_profile_idx = self
                .profiles
                .iter()
                .position(|p| p.id == active_id)
                .unwrap_or(0);
        } else {
            self.active_profile_idx = 0;
        }

        self.active_profile_idx = self
            .active_profile_idx
            .min(self.profiles.len().saturating_sub(1));

        // Sync active profile panes into `current_day` for seamless canvas rendering.
        if let Some(active) = self.profiles.get(self.active_profile_idx) {
            self.current_day.panes = active.panes.clone();
            self.current_day.date_iso = active.date_str.clone();
        }
    }

    /// Initializes/migrates profiles against the current local date and clock.
    pub fn ensure_profiles_initialized(&mut self) {
        use chrono::Datelike;
        let now = chrono::Local::now();
        let today_date = now.format("%Y-%m-%d").to_string();
        let lang = self.language.unwrap_or(AppLang::Ar);
        let today_title = DayProfile::title_for_weekday(now.weekday(), lang);
        self.ensure_profiles_for_date(&today_date, now.timestamp(), today_title);
    }

    /// Writes `current_day.panes` back into the currently active `DayProfile`.
    pub fn sync_active_profile_from_current_day(&mut self) {
        if let Some(active) = self.profiles.get_mut(self.active_profile_idx) {
            active.panes = self.current_day.panes.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(total: usize, completed: usize, cancelled: usize) -> StatusCounts {
        StatusCounts {
            total,
            completed_count: completed,
            cancelled_count: cancelled,
            ..Default::default()
        }
    }

    #[test]
    fn completion_rate_excludes_red() {
        assert_eq!(counts(10, 5, 2).completion_rate(), 62.5);
    }

    #[test]
    fn completion_rate_handles_empty_denominator() {
        assert_eq!(counts(2, 0, 2).completion_rate(), 0.0);
        assert_eq!(counts(0, 0, 0).completion_rate(), 0.0);
    }

    #[test]
    fn node_counts_count_only_leaf_subtasks() {
        let mut pane = MainTaskPane::new("Main");
        pane.status = TaskStatus::Completed;
        let mut child = TaskNode::new("Child");
        child.status = TaskStatus::Active;
        let mut grandchild = TaskNode::new("Grandchild");
        grandchild.status = TaskStatus::Cancelled;
        child.children.push(grandchild);
        pane.subtasks.push(child);
        pane.subtasks.push(TaskNode::new("Sibling"));

        let mut c = StatusCounts::default();
        pane.count_statuses(&mut c);
        // Only the 2 leaf subtasks ("Grandchild" Cancelled, "Sibling" Active) are counted;
        // neither the pane container nor the non-leaf parent "Child" is counted.
        assert_eq!(c.total, 2);
        assert_eq!(c.completed_count, 0);
        assert_eq!(c.active_count, 1);
        assert_eq!(c.cancelled_count, 1);
        assert_eq!(c.actionable_total(), 1);
        assert_eq!(c.progress_counts(), (0, 1));
        assert_eq!(c.completion_rate(), 0.0);
    }

    #[test]
    fn day_board_pane_and_profile_counts_are_unified() {
        let mut pane1 = MainTaskPane::new("Project A");
        let mut parent = TaskNode::new("Group");
        parent.status = TaskStatus::Active; // non-leaf: must not be counted
        let mut leaf_done = TaskNode::new("Done leaf");
        leaf_done.status = TaskStatus::Completed;
        let mut leaf_cancelled = TaskNode::new("Cancelled leaf");
        leaf_cancelled.status = TaskStatus::Cancelled;
        parent.children = vec![leaf_done, leaf_cancelled];
        pane1.subtasks.push(parent);
        pane1.subtasks.push(TaskNode::new("Active leaf"));

        let empty_pane = MainTaskPane::new("Empty Pane"); // 0 leaves: must not count as 1 active

        let day = DayBoard {
            date_iso: "2026-11-23".to_string(),
            panes: vec![pane1.clone(), empty_pane.clone()],
        };
        let profile = DayProfile {
            id: "p1".to_string(),
            date_str: "2026-11-23".to_string(),
            title: "Monday".to_string(),
            created_ts: 100,
            panes: vec![pane1.clone(), empty_pane.clone()],
        };

        let day_counts = day.calculate_counts();
        assert_eq!(day_counts.completed_count, 1);
        assert_eq!(day_counts.active_count, 1);
        assert_eq!(day_counts.cancelled_count, 1);
        assert_eq!(day_counts.total, 3);
        assert_eq!(day_counts.actionable_total(), 2);
        assert_eq!(day_counts.completion_rate(), 50.0);

        assert_eq!(pane1.progress_counts(), (1, 2));
        assert_eq!(empty_pane.progress_counts(), (0, 0));
        assert_eq!(profile.completion_counts(), (1, 2));
        assert_eq!(profile.completion_counts(), day_counts.progress_counts());
    }

    #[test]
    fn serde_roundtrip() {
        let day = DayBoard::default();
        let json = serde_json::to_string(&day).unwrap();
        let back: DayBoard = serde_json::from_str(&json).unwrap();
        assert_eq!(day.date_iso, back.date_iso);
    }

    #[test]
    fn legacy_task_node_deserializes_with_default_notes_and_collapsed() {
        let legacy_json = r#"{
            "id": "n-1",
            "title": "مهمة قديمة",
            "status": "Active",
            "children": []
        }"#;
        let node: TaskNode = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(node.notes, "");
        assert!(!node.collapsed);
        assert!(!node.is_folded());
    }

    #[test]
    fn legacy_json_with_removed_fields_and_is_expanded_loads_cleanly() {
        let legacy_json = r#"{
            "id": "n-stamp-1",
            "title": "مهمة بختم قديم",
            "status": "Active",
            "notes": "",
            "attachments": [],
            "children": [
                {
                    "id": "n-folded",
                    "title": "فرع مطوي",
                    "status": "Active",
                    "is_expanded": false
                }
            ],
            "is_expanded": true,
            "collapsed": false,
            "stamp": "Dog"
        }"#;
        let node: TaskNode = serde_json::from_str(legacy_json)
            .expect("serde must migrate legacy is_expanded and ignore removed fields cleanly");
        assert_eq!(node.id, "n-stamp-1");
        assert_eq!(node.title, "مهمة بختم قديم");
        assert!(!node.collapsed);
        assert!(
            node.children[0].collapsed,
            "legacy is_expanded: false must migrate to collapsed: true"
        );

        let legacy_app_json = r#"{
            "current_day": {
                "date_iso": "2026-11-20",
                "display_title": "FRI, NOV 20, 2026",
                "panes": []
            },
            "months": [{"month_label": "NOV", "goals": ["", "", "", ""], "weeks": []}],
            "harbor_reports": []
        }"#;
        let app_data: AppData = serde_json::from_str(legacy_app_json)
            .expect("old AppData with months, harbor_reports, and display_title must still load");
        assert_eq!(app_data.current_day.date_iso, "2026-11-20");
    }

    #[test]
    fn descendant_count_includes_all_nested_levels() {
        let mut root = TaskNode::new("root");
        let mut child = TaskNode::new("child");
        child.children.push(TaskNode::new("grandchild 1"));
        child.children.push(TaskNode::new("grandchild 2"));
        root.children.push(child);
        root.children.push(TaskNode::new("sibling"));
        assert_eq!(root.descendant_count(), 4);
    }
}
