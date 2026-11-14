# Project Specification: Native Rust Tiling Whiteboard & Execution Tracker

## 1. Overview & Architecture
Build a fast, lightweight, local-first native desktop application in **Rust** designed to replace physical whiteboards. It combines an **auto-tiling window layout (i3wm/Hyprland style)**, a **3-level semantic zoom hierarchy**, **rapid keyboard-driven tree outlining**, a **tri-color execution system with batch selection**, and an **End-of-Day/Week Reporting Harbor**.

### Tech Stack & Environment (Fedora Linux / Cross-Platform)
- **Language:** Rust (2021 edition).
- **GUI Engine:** `eframe` / `egui` (Immediate Mode GUI for custom dynamic tiling, tree connector rendering, and lasso/batch selection at 60+ FPS).
- **Arabic RTL & Typography:** Full bidirectional Arabic + English text rendering. Load system Arabic fonts (`NotoSansArabic-Regular.ttf` from `/usr/share/fonts/google-noto/` with bundled fallback) into `egui::FontDefinitions`, and shape Arabic glyphs properly using `arabic-reshaper` + `unicode-bidi` so Arabic text renders connected and right-to-left without reversed letters.
- **Local-First Storage:** `serde` + `serde_json` saving automatically on every state change to `~/.local/share/whiteboard_tracker/data.json` (via `dirs::data_local_dir()`).

---

## 2. Core Data Models (`src/models.rs`)
- **`TaskStatus` (Enum):**
  - `Default`: Uncolored (planned, not yet started).
  - `Yellow`: In Progress / Priority Focus (marked to start now).
  - `Green`: Completed.
  - `Red`: Cancelled / Dropped (intentionally discarded; excluded from failure denominator in metrics).
- **`TaskNode` (Struct):**
  - `id: String` (UUID)
  - `title: String`
  - `status: TaskStatus`
  - `children: Vec<TaskNode>`
  - `is_expanded: bool` (defaults to `true` so full trees are visible in Overview)
- **`MainTaskPane` (Struct):**
  - `id: String`
  - `title: String`
  - `status: TaskStatus`
  - `subtasks: Vec<TaskNode>`
- **`DayBoard` (Struct):**
  - `date_iso: String` (e.g., `"2026-11-14"`)
  - `display_title: String` (e.g., `"الخميس - 14 تشرين الثاني"`)
  - `panes: Vec<MainTaskPane>`
- **`WeekBoard` & `MonthBoard` (Structs):**
  - `MonthBoard`: `month_label: String`, `goals: [String; 4]` (4 plain-text monthly objectives without sub-trees), `weeks: [WeekBoard; 4]`.
  - `WeekBoard`: `week_index: u8`, `weekly_focus: String`, `days: [DayBoard; 7]`, `appointments: Vec<(String, String, String)>` (day_index, time_slot, note).
- **`HarborReport` (Struct):**
  - `id: String`, `timestamp: String`, `period_label: String`, `completion_rate: f32`, `counts: StatusCounts`, `markdown_body: String`, `user_notes: String`.

---

## 3. UI & UX Workflows

### A. Semantic Zoom Levels (Top Bar Switcher + `Ctrl + Scroll`)
1. **Macro Monthly View (Level 1):**
   - Top/Side bar with **4 Monthly Goal slots** (text-only high-level goals, no branching).
   - 2x2 Grid of **4 Weeks**, each showing its primary weekly task. Clicking a week zooms into Level 2.
2. **Weekly Schedule View (Level 2):**
   - **7 vertical columns** representing the 7 days of the week formatted as a clean **Time-Blocking / Appointment Schedule**.
   - Clicking a day header zooms into Level 3 for that day.
3. **Daily Tiling Whiteboard (Level 3 — Default Entry):**
   - **Quick Entry Modal (when day is empty):** Floating modal pre-filled with Day Name & Date. User enters main tasks separated by `/` or `Enter` (e.g., `علاج بابا / كتابة اللائحة للمرافعة / التقديم على المناقصة`).
   - **Auto-Tiling Canvas:** Immediately splits the screen into balanced side-by-side panes matching the number of main tasks.
   - **Dual Daily View Modes:**
     - **Tiled Overview Mode (Default):** Displays all panes simultaneously with their **entire expanded sub-task trees** visible at once.
     - **Focus Mode:** Double-clicking a pane header (or pressing `Enter` on a pane) maximizes that pane to full screen while minimizing others to a top pill bar. `Esc` restores Tiled Overview.

### B. Zero-Friction Tree Outlining
- Inside any pane, pressing `Enter` creates a new sibling sub-task immediately below the focused node and focuses its text input.
- Clicking a sub-task or pressing `Tab` creates a nested child branch underneath with clean visual tree lines (`├─`, `└─`) and auto-focuses the input. `Shift + Tab` outdents. No confirmation dialogs or popups.

### C. Tri-Color Execution & Batch Selection
- **Multi-Select:** Support `Ctrl + Click` and drag-box (Lasso) selection across any nodes/panes.
- **Hotkeys:** Pressing `1` sets selected node(s) to **Yellow**, `2` to **Green**, `3` to **Red**, `0` to **Default (Uncolored)**.
- **Smart Cascading:** When all children of a parent node become `Green`, automatically set the parent to `Green` (user can still override manually).
- **Carry-Over:** Unfinished (`Default` / `Yellow`) items at day's end can be carried over to the next day with one click, kept in the log, or marked `Red`.

### D. Dual App Modes: Live Whiteboard vs. Reporting Harbor
- Persistent top toggle between **`[السبورة الحية | Live Board]`** and **`[ميناء التقارير | Reporting Harbor]`**.
- **Dock & Report Workflow:**
  1. Clicking **"إرساء وسحب تقرير (Dock & Report)"** snapshots the active day or week.
  2. **Auto-Metrics Panel:** Computes `Completion % = Green / (Total - Red) * 100` (excluding `Red` cancelled items from failure penalties) and displays breakdown badges.
  3. **Editable Staging Area:** Allows editing the structured report text, adjusting statuses, and adding closing reflections.
  4. **Commit to Harbor:** Saves an immutable timestamped `HarborReport` into the **Harbor Archive** (with one-click Copy to Clipboard and Markdown export) and optionally resets the board for the next day.

---

## 4. Modular AI Integration Hook (`src/ai.rs`)
- Define a decoupled async/threaded trait `AiProvider` (supporting local Ollama HTTP endpoint or Gemini REST API via `reqwest` / `ureq`) ready for Milestone 4:
  - `suggest_subtasks(task_title: &str) -> Vec<String>`
  - `summarize_harbor_report(report: &HarborReport) -> String`

---

## 5. Implementation Milestones (Execute Sequentially)
- **Milestone 1:** `Cargo.toml` dependencies, Arabic RTL text shaping helper (`src/rtl_text.rs`), core data models (`src/models.rs`), JSON auto-save (`src/storage.rs`), and the **Daily Tiled Overview UI** with Quick Entry modal, full-tree rendering, keyboard outlining (`Enter`/`Tab`), and tri-color batch selection (`1`/`2`/`3`/`0`). Verify with `cargo check`.
- **Milestone 2:** Focus Mode (`Double-Click`/`Esc`), Smart Cascading colors, Carry-Over button, and Undo stack (`Ctrl+Z`).
- **Milestone 3:** Weekly Schedule View (7-column agenda) and Monthly Macro View (4 goals + 4 weeks) with smooth semantic zoom transitions.
- **Milestone 4:** Reporting Harbor Mode (Auto-metrics excluding Red, editable staging area, Harbor Archive log, Markdown export) + `AiProvider` integration.
