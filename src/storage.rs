use crate::models::{AppData, DayBoard, TaskNode, TaskStatus};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub file_path: PathBuf,
    pub message: String,
    pub backup_path: Option<PathBuf>,
    pub backup_error: Option<String>,
}

pub fn get_data_path() -> PathBuf {
    if let Some(mut path) = dirs::data_local_dir() {
        path.push("whiteboard_tracker");
        path
    } else {
        PathBuf::from(".whiteboard_tracker")
    }
}

pub fn get_data_file_path() -> PathBuf {
    let mut path = get_data_path();
    path.push("data.json");
    path
}

/// Copies a corrupt or unreadable `data.json` to a unique timestamped `.bak` file
/// without ever overwriting an existing backup.
pub fn backup_corrupted_file(file_path: &Path) -> Result<PathBuf, String> {
    let parent = file_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data.json");
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

    let mut source = File::open(file_path)
        .map_err(|e| format!("Failed to open {} for backup: {}", file_path.display(), e))?;

    for attempt in 0u32..1000 {
        let candidate_name = if attempt == 0 {
            format!("{}.{}.bak", file_name, timestamp)
        } else {
            format!("{}.{}_{}.bak", file_name, timestamp, attempt)
        };
        let candidate_path = parent.join(candidate_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate_path)
        {
            Ok(mut dest) => {
                if let Err(e) = io::copy(&mut source, &mut dest) {
                    drop(dest);
                    let _ = fs::remove_file(&candidate_path);
                    return Err(format!(
                        "Failed to copy {} to backup {}: {}",
                        file_path.display(),
                        candidate_path.display(),
                        e
                    ));
                }
                let _ = dest.flush();
                return Ok(candidate_path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!(
                    "Failed to create backup {}: {}",
                    candidate_path.display(),
                    e
                ));
            }
        }
    }

    Err(format!(
        "Could not allocate a unique backup filename for {}",
        file_path.display()
    ))
}

pub fn load_app_data() -> Result<AppData, LoadError> {
    let file_path = get_data_file_path();
    load_app_data_from_path(&file_path)
}

pub fn load_app_data_from_path(file_path: &Path) -> Result<AppData, LoadError> {
    let content = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(AppData::default());
        }
        Err(e) => {
            let message = format!("Failed to read {}: {}", file_path.display(), e);
            eprintln!("{}", message);
            let (backup_path, backup_error) = match backup_corrupted_file(file_path) {
                Ok(p) => (Some(p), None),
                Err(be) => (None, Some(be)),
            };
            return Err(LoadError {
                file_path: file_path.to_path_buf(),
                message,
                backup_path,
                backup_error,
            });
        }
    };

    match serde_json::from_str::<AppData>(&content) {
        Ok(data) => Ok(data),
        Err(e) => {
            let message = format!("Failed to parse {}: {}", file_path.display(), e);
            eprintln!("{}", message);
            let (backup_path, backup_error) = match backup_corrupted_file(file_path) {
                Ok(p) => (Some(p), None),
                Err(be) => (None, Some(be)),
            };
            Err(LoadError {
                file_path: file_path.to_path_buf(),
                message,
                backup_path,
                backup_error,
            })
        }
    }
}

pub fn save_app_data(data: &AppData) -> Result<(), String> {
    let dir_path = get_data_path();
    save_app_data_to_dir(data, &dir_path)
}

pub fn save_app_data_to_dir(data: &AppData, dir_path: &Path) -> Result<(), String> {
    if let Err(e) = fs::create_dir_all(dir_path) {
        return Err(format!("Failed to create directory {:?}: {}", dir_path, e));
    }

    let file_path = dir_path.join("data.json");
    let tmp_path = dir_path.join("data.tmp");

    let json_bytes = match serde_json::to_string_pretty(data) {
        Ok(json) => json,
        Err(e) => return Err(format!("Failed to serialize app data: {}", e)),
    };

    let write_res: io::Result<()> = (|| {
        let mut tmp_file = File::create(&tmp_path)?;
        tmp_file.write_all(json_bytes.as_bytes())?;
        tmp_file.flush()?;
        tmp_file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Failed to write tmp file {:?}: {}", tmp_path, e));
    }

    if let Err(e) = fs::rename(&tmp_path, &file_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Failed to rename tmp file to data.json: {}", e));
    }

    Ok(())
}

/// Exports the full `AppData` as pretty-printed JSON to `dest_path`.
pub fn export_app_data_to_path(data: &AppData, dest_path: &Path) -> Result<(), String> {
    if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize app data: {}", e))?;
    fs::write(dest_path, json.as_bytes())
        .map_err(|e| format!("Failed to write {}: {}", dest_path.display(), e))
}

/// Reads and validates an exported JSON file into `AppData`.
pub fn import_app_data_from_path(src_path: &Path) -> Result<AppData, String> {
    let content = fs::read_to_string(src_path)
        .map_err(|e| format!("Failed to read {}: {}", src_path.display(), e))?;
    serde_json::from_str::<AppData>(&content)
        .map_err(|e| format!("Invalid JSON in {}: {}", src_path.display(), e))
}

/// Writes a unique timestamped `.bak` snapshot of `data` inside `dir_path`,
/// never overwriting an existing backup file.
pub fn backup_app_data_to_dir(data: &AppData, dir_path: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dir_path)
        .map_err(|e| format!("Failed to create backup directory {}: {}", dir_path.display(), e))?;
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize backup data: {}", e))?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

    for attempt in 0u32..1000 {
        let candidate_name = if attempt == 0 {
            format!("data.json.{}.bak", timestamp)
        } else {
            format!("data.json.{}_{}.bak", timestamp, attempt)
        };
        let candidate_path = dir_path.join(candidate_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate_path)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(json.as_bytes()).and_then(|_| file.flush()) {
                    drop(file);
                    let _ = fs::remove_file(&candidate_path);
                    return Err(format!(
                        "Failed to write backup {}: {}",
                        candidate_path.display(),
                        e
                    ));
                }
                return Ok(candidate_path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!(
                    "Failed to create backup {}: {}",
                    candidate_path.display(),
                    e
                ));
            }
        }
    }

    Err(format!(
        "Could not allocate a unique backup filename in {}",
        dir_path.display()
    ))
}

/// Writes a timestamped `.bak` snapshot of the current in-memory `AppData` into the app data directory.
pub fn backup_current_app_data(data: &AppData) -> Result<PathBuf, String> {
    let dir_path = get_data_path();
    backup_app_data_to_dir(data, &dir_path)
}

/// Formats the given day board as clean, hierarchical Markdown with leaf-task progress summary.
pub fn format_day_as_markdown(day: &DayBoard, profile_title: Option<&str>) -> String {
    fn write_nodes(out: &mut String, nodes: &[TaskNode], depth: usize) {
        let indent = "  ".repeat(depth);
        for node in nodes {
            match node.status {
                TaskStatus::Active => {
                    out.push_str(&format!("{indent}- [ ] {}\n", node.title.trim()));
                }
                TaskStatus::Completed => {
                    out.push_str(&format!("{indent}- [x] {}\n", node.title.trim()));
                }
                TaskStatus::Cancelled => {
                    out.push_str(&format!("{indent}- [-] ~~{}~~\n", node.title.trim()));
                }
            }
            let notes = node.notes.trim();
            if !notes.is_empty() {
                for line in notes.lines() {
                    out.push_str(&format!("{indent}  > {}\n", line));
                }
            }
            if !node.children.is_empty() {
                write_nodes(out, &node.children, depth + 1);
            }
        }
    }

    let mut out = String::new();
    let heading = profile_title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(day.date_iso.as_str());
    out.push_str(&format!("# {} ({})\n\n", heading, day.date_iso));

    let counts = day.calculate_counts();
    let (done, actionable) = counts.progress_counts();
    let rate = counts.completion_rate();
    out.push_str(&format!(
        "> Progress: {}/{} ({:.0}%) — Done: {} | Active: {} | Cancelled: {}\n\n",
        done, actionable, rate, counts.completed_count, counts.active_count, counts.cancelled_count
    ));

    if day.panes.is_empty() {
        out.push_str("_No task panes._\n");
        return out;
    }

    for (idx, pane) in day.panes.iter().enumerate() {
        let (p_done, p_actionable) = pane.progress_counts();
        out.push_str(&format!(
            "## {:02}. {} ({}/{})\n\n",
            idx + 1,
            pane.title.trim(),
            p_done,
            p_actionable
        ));
        if pane.subtasks.is_empty() {
            out.push_str("_No subtasks._\n\n");
        } else {
            write_nodes(&mut out, &pane.subtasks, 0);
            out.push('\n');
        }
    }

    out
}

/// Writes Markdown text to `dest_path`.
pub fn export_markdown_to_path(markdown: &str, dest_path: &Path) -> Result<(), String> {
    if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }
    fs::write(dest_path, markdown.as_bytes())
        .map_err(|e| format!("Failed to write {}: {}", dest_path.display(), e))
}

fn ensure_extension(mut path: PathBuf, ext: &str) -> PathBuf {
    let has_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext));
    if !has_ext {
        let mut name = path.as_os_str().to_os_string();
        name.push(".");
        name.push(ext);
        path = PathBuf::from(name);
    }
    path
}

fn default_dialog_path(default_name: &str) -> PathBuf {
    if let Some(mut dir) = dirs::document_dir().or_else(dirs::home_dir) {
        dir.push(default_name);
        dir
    } else {
        PathBuf::from(default_name)
    }
}

/// Opens a native OS Save File dialog. Returns `Ok(None)` if the user cancelled.
pub fn pick_save_file(
    title: &str,
    default_name: &str,
    filter_label: &str,
    filter_ext: &str,
) -> Result<Option<PathBuf>, String> {
    let suggested = default_dialog_path(default_name);

    #[cfg(target_os = "linux")]
    {
        let filter_arg = format!("{} | *.{}", filter_label, filter_ext);
        let zenity = Command::new("zenity")
            .args([
                "--file-selection",
                "--save",
                "--title",
                title,
                "--filename",
                &suggested.to_string_lossy(),
                "--file-filter",
                &filter_arg,
            ])
            .output();
        match zenity {
            Ok(out) => {
                if out.status.success() {
                    let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if chosen.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(ensure_extension(PathBuf::from(chosen), filter_ext)));
                }
                return Ok(None);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let kfilter = format!("*.{}|{}", filter_ext, filter_label);
                let kd = Command::new("kdialog")
                    .args([
                        "--title",
                        title,
                        "--getsavefilename",
                        &suggested.to_string_lossy(),
                        &kfilter,
                    ])
                    .output()
                    .map_err(|ke| {
                        format!("No file dialog utility found (install zenity or kdialog): {}", ke)
                    })?;
                if kd.status.success() {
                    let chosen = String::from_utf8_lossy(&kd.stdout).trim().to_string();
                    if chosen.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(ensure_extension(PathBuf::from(chosen), filter_ext)));
                }
                return Ok(None);
            }
            Err(e) => return Err(format!("Failed to launch file dialog: {}", e)),
        }
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "POSIX path of (choose file name with prompt \"{}\" default name \"{}\")",
            title.replace('"', "\\\""),
            default_name.replace('"', "\\\"")
        );
        let out = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("Failed to launch file dialog: {}", e))?;
        if out.status.success() {
            let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if chosen.is_empty() {
                return Ok(None);
            }
            return Ok(Some(ensure_extension(PathBuf::from(chosen), filter_ext)));
        }
        return Ok(None);
    }

    #[cfg(target_os = "windows")]
    {
        let ps = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $d = New-Object System.Windows.Forms.SaveFileDialog; \
             $d.Title = '{}'; $d.FileName = '{}'; \
             $d.Filter = '{} (*.{})|*.{}|All files (*.*)|*.*'; \
             if ($d.ShowDialog() -eq 'OK') {{ Write-Output $d.FileName }}",
            title.replace('\'', "''"),
            default_name.replace('\'', "''"),
            filter_label.replace('\'', "''"),
            filter_ext,
            filter_ext
        );
        let out = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .output()
            .map_err(|e| format!("Failed to launch file dialog: {}", e))?;
        if out.status.success() {
            let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if chosen.is_empty() {
                return Ok(None);
            }
            return Ok(Some(ensure_extension(PathBuf::from(chosen), filter_ext)));
        }
        return Ok(None);
    }

    #[allow(unreachable_code)]
    Err("Native file dialog is not supported on this platform".to_string())
}

/// Opens a native OS Open File dialog. Returns `Ok(None)` if the user cancelled.
pub fn pick_open_file(
    title: &str,
    filter_label: &str,
    filter_ext: &str,
) -> Result<Option<PathBuf>, String> {
    let start_dir = dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));

    #[cfg(target_os = "linux")]
    {
        let filter_arg = format!("{} | *.{}", filter_label, filter_ext);
        let zenity = Command::new("zenity")
            .args([
                "--file-selection",
                "--title",
                title,
                "--file-filter",
                &filter_arg,
            ])
            .output();
        match zenity {
            Ok(out) => {
                if out.status.success() {
                    let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if chosen.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(PathBuf::from(chosen)));
                }
                return Ok(None);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let kfilter = format!("*.{}|{}", filter_ext, filter_label);
                let kd = Command::new("kdialog")
                    .args([
                        "--title",
                        title,
                        "--getopenfilename",
                        &start_dir.to_string_lossy(),
                        &kfilter,
                    ])
                    .output()
                    .map_err(|ke| {
                        format!("No file dialog utility found (install zenity or kdialog): {}", ke)
                    })?;
                if kd.status.success() {
                    let chosen = String::from_utf8_lossy(&kd.stdout).trim().to_string();
                    if chosen.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(PathBuf::from(chosen)));
                }
                return Ok(None);
            }
            Err(e) => return Err(format!("Failed to launch file dialog: {}", e)),
        }
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "POSIX path of (choose file with prompt \"{}\")",
            title.replace('"', "\\\"")
        );
        let out = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("Failed to launch file dialog: {}", e))?;
        if out.status.success() {
            let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if chosen.is_empty() {
                return Ok(None);
            }
            return Ok(Some(PathBuf::from(chosen)));
        }
        return Ok(None);
    }

    #[cfg(target_os = "windows")]
    {
        let ps = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $d = New-Object System.Windows.Forms.OpenFileDialog; \
             $d.Title = '{}'; $d.InitialDirectory = '{}'; \
             $d.Filter = '{} (*.{})|*.{}|All files (*.*)|*.*'; \
             if ($d.ShowDialog() -eq 'OK') {{ Write-Output $d.FileName }}",
            title.replace('\'', "''"),
            start_dir.to_string_lossy().replace('\'', "''"),
            filter_label.replace('\'', "''"),
            filter_ext,
            filter_ext
        );
        let out = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .output()
            .map_err(|e| format!("Failed to launch file dialog: {}", e))?;
        if out.status.success() {
            let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if chosen.is_empty() {
                return Ok(None);
            }
            return Ok(Some(PathBuf::from(chosen)));
        }
        return Ok(None);
    }

    #[allow(unreachable_code)]
    Err("Native file dialog is not supported on this platform".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MainTaskPane;
    use uuid::Uuid;

    fn make_temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wb_storage_{}_{}", label, Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn load_missing_file_returns_default_without_error() {
        let dir = make_temp_dir("missing");
        let file = dir.join("data.json");
        let data = load_app_data_from_path(&file).expect("missing file should return Ok(default)");
        assert!(data.current_day.panes.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_corrupt_json_creates_unique_timestamped_backups_never_overwriting() {
        let dir = make_temp_dir("corrupt");
        let file = dir.join("data.json");
        fs::write(&file, "{ not valid json").expect("write corrupt data");

        let err1 = load_app_data_from_path(&file).expect_err("corrupt JSON must return Err");
        let bak1 = err1
            .backup_path
            .expect("first load error must produce a backup copy");
        assert!(bak1.exists());
        assert_eq!(
            fs::read_to_string(&bak1).unwrap(),
            "{ not valid json",
            "first backup must preserve original corrupt content"
        );
        assert!(
            bak1.file_name()
                .and_then(|n| n.to_str())
                .unwrap()
                .ends_with(".bak"),
            "backup filename must end with .bak"
        );

        // Second failure in the same second must NOT overwrite the first backup.
        fs::write(&file, "{ second corrupt payload").expect("write second corrupt data");
        let err2 = load_app_data_from_path(&file).expect_err("second corrupt JSON must return Err");
        let bak2 = err2
            .backup_path
            .expect("second load error must produce a distinct backup copy");
        assert_ne!(bak1, bak2, "existing .bak must never be overwritten");
        assert_eq!(fs::read_to_string(&bak1).unwrap(), "{ not valid json");
        assert_eq!(
            fs::read_to_string(&bak2).unwrap(),
            "{ second corrupt payload"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_failure_deletes_data_tmp() {
        let dir = make_temp_dir("save_fail");
        // Make `data.json` a directory so `fs::rename(data.tmp, data.json)` fails.
        let blocking_dir = dir.join("data.json");
        fs::create_dir_all(&blocking_dir).expect("create blocking directory at data.json");

        let mut data = AppData::default();
        data.current_day.panes.push(MainTaskPane::new("Test Pane"));

        let err = save_app_data_to_dir(&data, &dir)
            .expect_err("rename over a directory must return Err");
        assert!(err.contains("rename"), "unexpected error: {err}");
        assert!(
            !dir.join("data.tmp").exists(),
            "data.tmp must be deleted when save fails"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_load_roundtrip_succeeds() {
        let dir = make_temp_dir("roundtrip");
        let mut data = AppData::default();
        data.current_day.panes.push(MainTaskPane::new("Saved Pane"));

        save_app_data_to_dir(&data, &dir).expect("save should succeed");
        assert!(!dir.join("data.tmp").exists());

        let loaded = load_app_data_from_path(&dir.join("data.json")).expect("load should succeed");
        assert_eq!(loaded.current_day.panes.len(), 1);
        assert_eq!(loaded.current_day.panes[0].title, "Saved Pane");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_and_import_json_roundtrip_and_invalid_rejected() {
        let dir = make_temp_dir("export_import");
        let export_file = dir.join("backup.json");

        let mut data = AppData::default();
        let mut pane = MainTaskPane::new("Exported Pane");
        pane.subtasks.push(TaskNode::new("Subtask 1"));
        data.current_day.panes.push(pane);

        export_app_data_to_path(&data, &export_file).expect("export should succeed");
        let imported = import_app_data_from_path(&export_file).expect("import should succeed");
        assert_eq!(imported.current_day.panes.len(), 1);
        assert_eq!(imported.current_day.panes[0].title, "Exported Pane");
        assert_eq!(imported.current_day.panes[0].subtasks[0].title, "Subtask 1");

        let invalid_file = dir.join("invalid.json");
        fs::write(&invalid_file, "{ broken").expect("write invalid json");
        let err = import_app_data_from_path(&invalid_file).expect_err("invalid JSON must fail");
        assert!(err.contains("Invalid JSON"), "unexpected error: {err}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_app_data_creates_unique_timestamped_bak_files() {
        let dir = make_temp_dir("pre_import_bak");
        let mut data1 = AppData::default();
        data1.current_day.panes.push(MainTaskPane::new("First"));
        let mut data2 = AppData::default();
        data2.current_day.panes.push(MainTaskPane::new("Second"));

        let bak1 = backup_app_data_to_dir(&data1, &dir).expect("first backup");
        let bak2 = backup_app_data_to_dir(&data2, &dir).expect("second backup");
        assert_ne!(bak1, bak2, "repeated backups must never overwrite each other");
        assert_eq!(
            import_app_data_from_path(&bak1).unwrap().current_day.panes[0].title,
            "First"
        );
        assert_eq!(
            import_app_data_from_path(&bak2).unwrap().current_day.panes[0].title,
            "Second"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_day_as_markdown_renders_hierarchy_statuses_notes_and_progress() {
        let mut pane = MainTaskPane::new("Sprint Work");
        let mut parent = TaskNode::new("Backend");
        let mut done_leaf = TaskNode::new("Write API");
        done_leaf.status = TaskStatus::Completed;
        done_leaf.notes = "Verified locally\nAll green".to_string();
        let active_leaf = TaskNode::new("Add docs");
        let mut cancelled_leaf = TaskNode::new("Legacy hook");
        cancelled_leaf.status = TaskStatus::Cancelled;
        parent.children = vec![done_leaf, active_leaf, cancelled_leaf];
        pane.subtasks.push(parent);

        let day = DayBoard {
            date_iso: "2026-11-23".to_string(),
            panes: vec![pane],
        };

        let md = format_day_as_markdown(&day, Some("Monday — Today's Tasks"));
        assert!(md.contains("# Monday — Today's Tasks (2026-11-23)"));
        assert!(md.contains("Progress: 1/2 (50%) — Done: 1 | Active: 1 | Cancelled: 1"));
        assert!(md.contains("## 01. Sprint Work (1/2)"));
        assert!(md.contains("- [ ] Backend\n"));
        assert!(md.contains("  - [x] Write API\n"));
        assert!(md.contains("    > Verified locally\n    > All green\n"));
        assert!(md.contains("  - [ ] Add docs\n"));
        assert!(md.contains("  - [-] ~~Legacy hook~~\n"));
    }
}
