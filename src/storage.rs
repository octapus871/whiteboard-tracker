use crate::models::AppData;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

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

pub fn load_app_data() -> AppData {
    let file_path = get_data_file_path();
    if file_path.exists() {
        match fs::read_to_string(&file_path) {
            Ok(content) => match serde_json::from_str::<AppData>(&content) {
                Ok(data) => return data,
                Err(e) => {
                    eprintln!("Failed to parse data.json: {}, backing up...", e);
                    let mut backup = file_path.clone();
                    backup.set_extension("json.bak");
                    let _ = fs::copy(&file_path, backup);
                }
            },
            Err(e) => {
                eprintln!("Failed to read data.json: {}", e);
            }
        }
    }
    AppData::default()
}

pub fn save_app_data(data: &AppData) -> Result<(), String> {
    let dir_path = get_data_path();
    if let Err(e) = fs::create_dir_all(&dir_path) {
        return Err(format!("Failed to create directory {:?}: {}", dir_path, e));
    }

    let file_path = get_data_file_path();
    let tmp_path = dir_path.join("data.tmp");

    let json_bytes = match serde_json::to_string_pretty(data) {
        Ok(json) => json,
        Err(e) => return Err(format!("Failed to serialize app data: {}", e)),
    };

    let mut tmp_file = match File::create(&tmp_path) {
        Ok(file) => file,
        Err(e) => return Err(format!("Failed to create tmp file {:?}: {}", tmp_path, e)),
    };

    if let Err(e) = tmp_file.write_all(json_bytes.as_bytes()) {
        return Err(format!("Failed to write tmp file: {}", e));
    }
    if let Err(e) = tmp_file.flush() {
        return Err(format!("Failed to flush tmp file: {}", e));
    }
    drop(tmp_file);

    if let Err(e) = fs::rename(&tmp_path, &file_path) {
        return Err(format!("Failed to rename tmp file to data.json: {}", e));
    }

    Ok(())
}
