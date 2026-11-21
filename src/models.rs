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
    pub fn label_en(&self) -> &'static str {
        match self {
            TaskStatus::Active => "Active",
            TaskStatus::Completed => "Completed",
            TaskStatus::Cancelled => "Cancelled",
        }
    }

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

/// Subtle monochrome geometric watermark stamp placed in a card's bottom-left
/// interior corner (`22×22px` bounds, `4px` inset).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardStamp {
    Dog,
    Cat,
    Duck,
    Bird,
    Star,
}

impl CardStamp {
    /// Cycles through `None -> Dog -> Cat -> Duck -> Bird -> Star -> None`.
    pub fn next_option(current: Option<Self>) -> Option<Self> {
        match current {
            None => Some(Self::Dog),
            Some(Self::Dog) => Some(Self::Cat),
            Some(Self::Cat) => Some(Self::Duck),
            Some(Self::Duck) => Some(Self::Bird),
            Some(Self::Bird) => Some(Self::Star),
            Some(Self::Star) => None,
        }
    }

    pub fn label_en(self) -> &'static str {
        match self {
            Self::Dog => "DOG",
            Self::Cat => "CAT",
            Self::Duck => "DUCK",
            Self::Bird => "BIRD",
            Self::Star => "STAR",
        }
    }
}

/// A single task node in the outline. Nodes can nest arbitrarily deep, which is
/// what powers the `Enter` (sibling) / `Tab` (child) / `Shift+Tab` (outdent)
/// outlining workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub stamp: Option<CardStamp>,
    #[serde(default)]
    pub children: Vec<TaskNode>,
    #[serde(default = "default_expanded")]
    pub is_expanded: bool,
}

fn default_expanded() -> bool {
    true
}

impl TaskNode {
    pub fn new(title: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            status: TaskStatus::Active,
            notes: String::new(),
            attachments: Vec::new(),
            collapsed: false,
            stamp: None,
            children: Vec::new(),
            is_expanded: true,
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
            attachments: self.attachments.clone(),
            collapsed: self.collapsed,
            stamp: self.stamp,
            children: active_children,
            is_expanded: self.is_expanded,
        })
    }

    /// True when this node's child branches are visually folded.
    pub fn is_folded(&self) -> bool {
        self.collapsed || !self.is_expanded
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

    pub fn count_statuses(&self, counts: &mut StatusCounts) {
        counts.total += 1;
        match self.status {
            TaskStatus::Active => counts.active_count += 1,
            TaskStatus::Completed => counts.completed_count += 1,
            TaskStatus::Cancelled => counts.cancelled_count += 1,
        }
        for child in &self.children {
            child.count_statuses(counts);
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

    pub fn count_statuses(&self, counts: &mut StatusCounts) {
        counts.total += 1;
        match self.status {
            TaskStatus::Active => counts.active_count += 1,
            TaskStatus::Completed => counts.completed_count += 1,
            TaskStatus::Cancelled => counts.cancelled_count += 1,
        }
        for sub in &self.subtasks {
            sub.count_statuses(counts);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayBoard {
    pub date_iso: String,
    pub display_title: String,
    pub panes: Vec<MainTaskPane>,
}

impl Default for DayBoard {
    fn default() -> Self {
        let now = chrono::Local::now();
        let date_iso = now.format("%Y-%m-%d").to_string();
        let display_title = now.format("%a, %b %d, %Y").to_string().to_uppercase();
        Self {
            date_iso,
            display_title,
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
pub struct WeekBoard {
    pub week_index: u8,
    pub weekly_focus: String,
    pub days: Vec<DayBoard>,
    pub appointments: Vec<(String, String, String)>, // (day_index, time_slot, note)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthBoard {
    pub month_label: String,
    pub goals: [String; 4],
    pub weeks: Vec<WeekBoard>,
}

impl Default for MonthBoard {
    fn default() -> Self {
        Self {
            month_label: "CURRENT MONTH".to_string(),
            goals: [String::new(), String::new(), String::new(), String::new()],
            weeks: vec![
                WeekBoard {
                    week_index: 1,
                    ..Default::default()
                },
                WeekBoard {
                    week_index: 2,
                    ..Default::default()
                },
                WeekBoard {
                    week_index: 3,
                    ..Default::default()
                },
                WeekBoard {
                    week_index: 4,
                    ..Default::default()
                },
            ],
        }
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
    /// Completion % = completed / (total - cancelled) * 100.
    /// Cancelled items are excluded from the denominator.
    pub fn completion_rate(&self) -> f32 {
        let denominator = self.total.saturating_sub(self.cancelled_count);
        if denominator == 0 {
            0.0
        } else {
            (self.completed_count as f32 / denominator as f32) * 100.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarborReport {
    pub id: String,
    pub timestamp: String,
    pub period_label: String,
    pub completion_rate: f32,
    pub counts: StatusCounts,
    pub markdown_body: String,
    pub user_notes: String,
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

    pub fn new_for_today(panes: Vec<MainTaskPane>) -> Self {
        use chrono::Datelike;
        let now = chrono::Local::now();
        Self {
            id: Uuid::new_v4().to_string(),
            date_str: now.format("%Y-%m-%d").to_string(),
            title: Self::arabic_weekday_title(now.weekday()),
            created_ts: now.timestamp(),
            panes,
        }
    }

    /// Returns `(completed_subtasks, total_subtasks)` across all panes in this profile.
    pub fn completion_counts(&self) -> (usize, usize) {
        fn count_nodes(nodes: &[TaskNode]) -> (usize, usize) {
            let mut done = 0;
            let mut total = 0;
            for n in nodes {
                total += 1;
                if n.status == TaskStatus::Completed {
                    done += 1;
                }
                let (cd, ct) = count_nodes(&n.children);
                done += cd;
                total += ct;
            }
            (done, total)
        }
        let mut done = 0;
        let mut total = 0;
        for pane in &self.panes {
            let (pd, pt) = count_nodes(&pane.subtasks);
            done += pd;
            total += pt;
        }
        (done, total)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppData {
    pub current_day: DayBoard,
    #[serde(default)]
    pub months: Vec<MonthBoard>,
    #[serde(default)]
    pub harbor_reports: Vec<HarborReport>,
    #[serde(default)]
    pub archived_panes: Vec<MainTaskPane>,
    #[serde(default)]
    pub profiles: Vec<DayProfile>,
    #[serde(default)]
    pub active_profile_idx: usize,
    #[serde(default)]
    pub has_seen_onboarding: bool,
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
        let today_title = DayProfile::arabic_weekday_title(now.weekday());
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
    fn node_counts_include_children() {
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
        assert_eq!(c.total, 4);
        assert_eq!(c.completed_count, 1);
        // The pane's second child stays active by default.
        assert_eq!(c.active_count, 2);
        assert_eq!(c.cancelled_count, 1);
    }

    #[test]
    fn serde_roundtrip() {
        let day = DayBoard::default();
        let json = serde_json::to_string(&day).unwrap();
        let back: DayBoard = serde_json::from_str(&json).unwrap();
        assert_eq!(day.date_iso, back.date_iso);
        assert_eq!(day.display_title, back.display_title);
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
        assert!(node.attachments.is_empty());
        assert!(!node.collapsed);
        assert!(!node.is_folded());
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
