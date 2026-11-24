use crate::models::{AppData, DayBoard, TaskNode, TaskStatus};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;

/// Environment variable that overrides the default application data directory.
pub const DATA_DIR_ENV_VAR: &str = "WHITEBOARD_TRACKER_DATA_DIR";

/// Optional CLI override set via `--data-dir <PATH>` (takes precedence over `WHITEBOARD_TRACKER_DATA_DIR`).
static DATA_DIR_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub file_path: PathBuf,
    pub message: String,
    pub backup_path: Option<PathBuf>,
    pub backup_error: Option<String>,
}

/// Sets or clears the process-wide data directory override (typically from `--data-dir`).
pub fn set_data_dir_override(path: Option<PathBuf>) {
    let cleaned = path.filter(|p| !p.as_os_str().is_empty());
    let mut guard = DATA_DIR_OVERRIDE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = cleaned;
}

/// Parses `--data-dir <PATH>` or `--data-dir=<PATH>` from CLI arguments (skipping argv[0]).
/// Returns an error if `--data-dir` is given without a non-empty path so the app never
/// silently falls back to the default user `data.json`.
pub fn parse_data_dir_arg<I, S>(args: I) -> Result<Option<PathBuf>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut iter = args.into_iter().skip(1);
    let mut result = None;

    while let Some(arg) = iter.next() {
        let arg_os = arg.as_ref();
        if arg_os == OsStr::new("--data-dir") {
            let Some(val) = iter.next() else {
                return Err("--data-dir requires a directory path".to_string());
            };
            let val_os = val.as_ref();
            if val_os.is_empty() || val_os.to_string_lossy().trim().is_empty() {
                return Err("--data-dir requires a non-empty directory path".to_string());
            }
            result = Some(PathBuf::from(val_os));
        } else if let Some(arg_str) = arg_os.to_str() {
            if let Some(rest) = arg_str.strip_prefix("--data-dir=") {
                if rest.trim().is_empty() {
                    return Err("--data-dir= requires a non-empty directory path".to_string());
                }
                result = Some(PathBuf::from(rest));
            }
        }
    }

    Ok(result)
}

fn default_data_path() -> PathBuf {
    if let Some(mut path) = dirs::data_local_dir() {
        path.push("whiteboard_tracker");
        path
    } else {
        PathBuf::from(".whiteboard_tracker")
    }
}

/// Resolves the active data directory with strict precedence:
/// 1. CLI `--data-dir` override
/// 2. `WHITEBOARD_TRACKER_DATA_DIR` environment variable
/// 3. OS-standard local data directory
pub fn resolve_data_path(cli_override: Option<&Path>, env_override: Option<&OsStr>) -> PathBuf {
    if let Some(cli) = cli_override {
        if !cli.as_os_str().is_empty() && !cli.to_string_lossy().trim().is_empty() {
            return cli.to_path_buf();
        }
    }
    if let Some(env_val) = env_override {
        if !env_val.is_empty() && !env_val.to_string_lossy().trim().is_empty() {
            return PathBuf::from(env_val);
        }
    }
    default_data_path()
}

pub fn get_data_path() -> PathBuf {
    let cli_override = DATA_DIR_OVERRIDE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let env_override = std::env::var_os(DATA_DIR_ENV_VAR);
    resolve_data_path(cli_override.as_deref(), env_override.as_deref())
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

    #[test]
    fn resolve_data_path_prefers_cli_over_env_over_default() {
        let cli = PathBuf::from("/tmp/wb_cli_dir");
        let env_val = OsStr::new("/tmp/wb_env_dir");

        assert_eq!(resolve_data_path(Some(&cli), Some(env_val)), cli);
        assert_eq!(
            resolve_data_path(None, Some(env_val)),
            PathBuf::from("/tmp/wb_env_dir")
        );
        assert_eq!(
            resolve_data_path(Some(Path::new("")), Some(env_val)),
            PathBuf::from("/tmp/wb_env_dir")
        );
        assert_eq!(
            resolve_data_path(None, Some(OsStr::new("   "))),
            default_data_path()
        );
        assert_eq!(resolve_data_path(None, None), default_data_path());
    }

    #[test]
    fn parse_data_dir_arg_supports_separate_and_equals_forms_and_rejects_missing() {
        assert_eq!(
            parse_data_dir_arg(["whiteboard_tracker"]).unwrap(),
            None
        );
        assert_eq!(
            parse_data_dir_arg(["whiteboard_tracker", "--data-dir", "demo/en"]).unwrap(),
            Some(PathBuf::from("demo/en"))
        );
        assert_eq!(
            parse_data_dir_arg(["whiteboard_tracker", "--data-dir=demo/en"]).unwrap(),
            Some(PathBuf::from("demo/en"))
        );
        assert!(parse_data_dir_arg(["whiteboard_tracker", "--data-dir"]).is_err());
        assert!(parse_data_dir_arg(["whiteboard_tracker", "--data-dir", "   "]).is_err());
        assert!(parse_data_dir_arg(["whiteboard_tracker", "--data-dir="]).is_err());
    }

    #[test]
    fn data_dir_override_isolates_reads_writes_tmp_and_backups() {
        use std::sync::Mutex;
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        struct EnvRestore {
            prev_env: Option<std::ffi::OsString>,
        }
        impl Drop for EnvRestore {
            fn drop(&mut self) {
                set_data_dir_override(None);
                match &self.prev_env {
                    Some(v) => std::env::set_var(DATA_DIR_ENV_VAR, v),
                    None => std::env::remove_var(DATA_DIR_ENV_VAR),
                }
            }
        }
        let _guard = EnvRestore {
            prev_env: std::env::var_os(DATA_DIR_ENV_VAR),
        };

        let env_dir = make_temp_dir("env_override");
        let cli_dir = make_temp_dir("cli_override");

        // 1. WHITEBOARD_TRACKER_DATA_DIR routes reads, writes, tmp, and backups into env_dir.
        set_data_dir_override(None);
        std::env::set_var(DATA_DIR_ENV_VAR, &env_dir);
        assert_eq!(get_data_path(), env_dir);
        assert_eq!(get_data_file_path(), env_dir.join("data.json"));

        let mut env_data = AppData::default();
        env_data
            .current_day
            .panes
            .push(MainTaskPane::new("Env Scoped Pane"));
        save_app_data(&env_data).expect("save via env override");
        assert!(env_dir.join("data.json").exists());
        assert!(!env_dir.join("data.tmp").exists());

        let loaded_env = load_app_data().expect("load via env override");
        assert_eq!(loaded_env.current_day.panes.len(), 1);
        assert_eq!(loaded_env.current_day.panes[0].title, "Env Scoped Pane");

        let env_bak = backup_current_app_data(&env_data).expect("backup via env override");
        assert_eq!(env_bak.parent(), Some(env_dir.as_path()));

        // Corrupt env_dir/data.json and verify recovery .bak lands in env_dir.
        fs::write(env_dir.join("data.json"), "{ invalid").expect("corrupt env data.json");
        let err = load_app_data().expect_err("corrupt env data.json must fail");
        assert_eq!(err.file_path, env_dir.join("data.json"));
        let corrupt_bak = err.backup_path.expect("corrupt backup path");
        assert_eq!(corrupt_bak.parent(), Some(env_dir.as_path()));

        // 2. --data-dir CLI override takes precedence over WHITEBOARD_TRACKER_DATA_DIR and isolates cli_dir.
        set_data_dir_override(Some(cli_dir.clone()));
        assert_eq!(get_data_path(), cli_dir);
        assert_eq!(get_data_file_path(), cli_dir.join("data.json"));

        let mut cli_data = AppData::default();
        cli_data
            .current_day
            .panes
            .push(MainTaskPane::new("CLI Scoped Pane"));
        save_app_data(&cli_data).expect("save via CLI override");
        assert!(cli_dir.join("data.json").exists());
        assert!(!cli_dir.join("data.tmp").exists());

        let loaded_cli = load_app_data().expect("load via CLI override");
        assert_eq!(loaded_cli.current_day.panes.len(), 1);
        assert_eq!(loaded_cli.current_day.panes[0].title, "CLI Scoped Pane");

        let cli_bak = backup_current_app_data(&cli_data).expect("backup via CLI override");
        assert_eq!(cli_bak.parent(), Some(cli_dir.as_path()));

        // Verify env_dir/data.json was untouched by the CLI override save.
        assert_eq!(
            fs::read_to_string(env_dir.join("data.json")).unwrap(),
            "{ invalid"
        );

        let _ = fs::remove_dir_all(&env_dir);
        let _ = fs::remove_dir_all(&cli_dir);
    }

    #[test]
    fn demo_en_data_json_loads_cleanly_and_meets_screenshot_spec() {
        use crate::models::AppLang;

        fn count_nodes(nodes: &[TaskNode]) -> usize {
            nodes.iter().map(|n| 1 + count_nodes(&n.children)).sum()
        }
        fn max_depth(nodes: &[TaskNode]) -> usize {
            nodes
                .iter()
                .map(|n| 1 + max_depth(&n.children))
                .max()
                .unwrap_or(0)
        }
        fn collect_stats(
            nodes: &[TaskNode],
            statuses: &mut Vec<TaskStatus>,
            notes_count: &mut usize,
            has_folded: &mut bool,
            has_completed_group: &mut bool,
        ) {
            for n in nodes {
                statuses.push(n.status);
                if !n.notes.trim().is_empty() {
                    *notes_count += 1;
                }
                if n.collapsed && !n.children.is_empty() {
                    *has_folded = true;
                }
                if n.status == TaskStatus::Completed && n.all_children_completed() {
                    *has_completed_group = true;
                }
                collect_stats(
                    &n.children,
                    statuses,
                    notes_count,
                    has_folded,
                    has_completed_group,
                );
            }
        }

        let demo_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("demo/en/data.json");
        let data = load_app_data_from_path(&demo_path)
            .expect("demo/en/data.json must deserialize cleanly into AppData");

        assert_eq!(data.language, Some(AppLang::En));
        assert!(data.has_seen_onboarding);
        assert_eq!(data.profiles.len(), 2);
        assert_eq!(data.active_profile_idx, 0);
        assert!(data.profiles[0].created_ts > data.profiles[1].created_ts);

        let today = &data.profiles[0];
        let titles: Vec<&str> = today.panes.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(
            titles,
            vec![
                "Launch the app",
                "Prepare weekly meeting",
                "Learn Rust",
                "Home renovation"
            ]
        );
        assert_eq!(data.current_day.panes.len(), 4);

        // Pane 0: 3 levels of depth and 8-10 nodes.
        let p0_nodes = count_nodes(&today.panes[0].subtasks);
        assert!((8..=10).contains(&p0_nodes), "expected 8-10 nodes, got {p0_nodes}");
        assert_eq!(max_depth(&today.panes[0].subtasks), 3);

        // Pane 3: nearly empty (1-2 nodes).
        assert!(count_nodes(&today.panes[3].subtasks) <= 2);

        // Mixed statuses, 2-3 notes, folded branch, and fully completed group.
        let mut statuses = Vec::new();
        let mut notes_count = 0usize;
        let mut has_folded = false;
        let mut has_completed_group = false;
        for pane in &today.panes {
            collect_stats(
                &pane.subtasks,
                &mut statuses,
                &mut notes_count,
                &mut has_folded,
                &mut has_completed_group,
            );
        }
        assert!(statuses.contains(&TaskStatus::Active));
        assert!(statuses.contains(&TaskStatus::Completed));
        assert!(statuses.contains(&TaskStatus::Cancelled));
        assert!((2..=3).contains(&notes_count), "expected 2-3 notes, got {notes_count}");
        assert!(has_folded, "expected at least one folded branch with children");
        assert!(has_completed_group, "expected at least one fully completed group");

        // Yesterday profile has unfinished active panes for carry-over.
        let yesterday = &data.profiles[1];
        assert!(
            yesterday
                .panes
                .iter()
                .any(|p| p.clone_active_only().is_some()),
            "yesterday must have unfinished active tasks to carry over"
        );
    }
}
