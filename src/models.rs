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
            children: Vec::new(),
            is_expanded: true,
        }
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppData {
    pub current_day: DayBoard,
    #[serde(default)]
    pub months: Vec<MonthBoard>,
    #[serde(default)]
    pub harbor_reports: Vec<HarborReport>,
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
