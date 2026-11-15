use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskStatus {
    #[default]
    Default,
    Yellow,
    Green,
    Red,
}

impl TaskStatus {
    pub fn label_ar(&self) -> &'static str {
        match self {
            TaskStatus::Default => "مخطط",
            TaskStatus::Yellow => "قيد التنفيذ",
            TaskStatus::Green => "مكتمل",
            TaskStatus::Red => "ملغى",
        }
    }

    pub fn label_en(&self) -> &'static str {
        match self {
            TaskStatus::Default => "Planned",
            TaskStatus::Yellow => "In Progress",
            TaskStatus::Green => "Completed",
            TaskStatus::Red => "Cancelled",
        }
    }

    /// Cycles through the tri-color states: planned → in progress → completed
    /// → cancelled → planned.
    pub fn next(self) -> Self {
        match self {
            TaskStatus::Default => TaskStatus::Yellow,
            TaskStatus::Yellow => TaskStatus::Green,
            TaskStatus::Green => TaskStatus::Red,
            TaskStatus::Red => TaskStatus::Default,
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
            status: TaskStatus::Default,
            children: Vec::new(),
            is_expanded: true,
        }
    }

    pub fn count_statuses(&self, counts: &mut StatusCounts) {
        counts.total += 1;
        match self.status {
            TaskStatus::Default => counts.default_count += 1,
            TaskStatus::Yellow => counts.yellow_count += 1,
            TaskStatus::Green => counts.green_count += 1,
            TaskStatus::Red => counts.red_count += 1,
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
            status: TaskStatus::Default,
            subtasks: Vec::new(),
        }
    }

    pub fn count_statuses(&self, counts: &mut StatusCounts) {
        counts.total += 1;
        match self.status {
            TaskStatus::Default => counts.default_count += 1,
            TaskStatus::Yellow => counts.yellow_count += 1,
            TaskStatus::Green => counts.green_count += 1,
            TaskStatus::Red => counts.red_count += 1,
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
        let display_title = format!(
            "{} - {} {}",
            arabic_weekday(now.format("%A").to_string().as_str()),
            now.format("%-d"),
            arabic_month(now.format("%-m").to_string().as_str())
        );
        Self {
            date_iso,
            display_title,
            panes: Vec::new(),
        }
    }
}

fn arabic_weekday(english: &str) -> &'static str {
    match english {
        "Monday" => "الاثنين",
        "Tuesday" => "الثلاثاء",
        "Wednesday" => "الأربعاء",
        "Thursday" => "الخميس",
        "Friday" => "الجمعة",
        "Saturday" => "السبت",
        "Sunday" => "الأحد",
        _ => "اليوم",
    }
}

fn arabic_month(month: &str) -> &'static str {
    match month {
        "1" => "كانون الثاني",
        "2" => "شباط",
        "3" => "آذار",
        "4" => "نيسان",
        "5" => "أيار",
        "6" => "حزيران",
        "7" => "تموز",
        "8" => "آب",
        "9" => "أيلول",
        "10" => "تشرين الأول",
        "11" => "تشرين الثاني",
        "12" => "كانون الأول",
        _ => "الشهر الحالي",
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
            month_label: "الشهر الحالي".to_string(),
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
    pub default_count: usize,
    pub yellow_count: usize,
    pub green_count: usize,
    pub red_count: usize,
    pub total: usize,
}

impl StatusCounts {
    /// Completion % = Green / (Total - Red) * 100
    /// Excludes Red (cancelled) items from failure denominator.
    pub fn completion_rate(&self) -> f32 {
        let denominator = self.total.saturating_sub(self.red_count);
        if denominator == 0 {
            0.0
        } else {
            (self.green_count as f32 / denominator as f32) * 100.0
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

    fn counts(total: usize, green: usize, red: usize) -> StatusCounts {
        StatusCounts {
            total,
            green_count: green,
            red_count: red,
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
        pane.status = TaskStatus::Green;
        let mut child = TaskNode::new("Child");
        child.status = TaskStatus::Yellow;
        let mut grandchild = TaskNode::new("Grandchild");
        grandchild.status = TaskStatus::Red;
        child.children.push(grandchild);
        pane.subtasks.push(child);
        pane.subtasks.push(TaskNode::new("Sibling"));

        let mut c = StatusCounts::default();
        pane.count_statuses(&mut c);
        assert_eq!(c.total, 4);
        assert_eq!(c.green_count, 1);
        assert_eq!(c.yellow_count, 1);
        assert_eq!(c.red_count, 1);
        assert_eq!(c.default_count, 1);
    }

    #[test]
    fn serde_roundtrip() {
        let day = DayBoard::default();
        let json = serde_json::to_string(&day).unwrap();
        let back: DayBoard = serde_json::from_str(&json).unwrap();
        assert_eq!(day.date_iso, back.date_iso);
        assert_eq!(day.display_title, back.display_title);
    }
}
