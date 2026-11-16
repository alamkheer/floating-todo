use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::state::{AppConfig, Task};

/// Serializable wrapper for the task list and application configuration.
#[derive(serde::Serialize, serde::Deserialize)]
struct TaskFile {
    next_id: u64,
    #[serde(default)]
    config: Option<AppConfig>,
    tasks: Vec<Task>,
}

/// Returns `%APPDATA%\FloatingTodo\`.
fn data_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(appdata).join("FloatingTodo")
}

/// Load tasks and config from a specific directory path. Returns default on error.
pub fn load_tasks_from(dir: &std::path::Path) -> (Vec<Task>, u64, AppConfig) {
    let path = dir.join("tasks.json");
    match fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<TaskFile>(&contents) {
            Ok(tf) => (tf.tasks, tf.next_id, tf.config.unwrap_or_default()),
            Err(_) => (Vec::new(), 1, AppConfig::default()),
        },
        Err(_) => (Vec::new(), 1, AppConfig::default()),
    }
}

/// Load tasks and config from disk (%APPDATA%\FloatingTodo). Returns defaults on any error.
pub fn load_tasks() -> (Vec<Task>, u64, AppConfig) {
    load_tasks_from(&data_dir())
}

/// Atomic write to a specific directory path: write to `.tmp`, then rename over target.
pub fn save_tasks_to(
    dir: &std::path::Path,
    tasks: &[Task],
    next_id: u64,
    config: &AppConfig,
) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;

    let path = dir.join("tasks.json");
    let tmp_path = dir.join("tasks.json.tmp");

    let tf = TaskFile {
        next_id,
        config: Some(config.clone()),
        tasks: tasks.to_vec(),
    };
    let json = serde_json::to_string_pretty(&tf)?;

    // Write to temp file
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;

    // Atomic rename (on Windows, this overwrites the destination)
    fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// Atomic write: write to `.tmp`, then rename over the target (%APPDATA%\FloatingTodo).
/// This prevents data corruption if the process is killed mid-write.
pub fn save_tasks(tasks: &[Task], next_id: u64, config: &AppConfig) -> std::io::Result<()> {
    save_tasks_to(&data_dir(), tasks, next_id, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_temp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("floating_todo_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = test_temp_dir("roundtrip");
        let tasks = vec![
            Task {
                id: 1,
                text: "Task A".to_string(),
                completed: false,
                created_at: "2026-11-16 18:00:00.000".to_string(),
                sub_tasks: Vec::new(),
            },
            Task {
                id: 2,
                text: "Task B".to_string(),
                completed: true,
                created_at: "2026-11-16 18:01:00.000".to_string(),
                sub_tasks: Vec::new(),
            },
        ];
        let mut config = AppConfig::default();
        config.record_time_seconds = 30;
        config.max_depth = 2;

        let save_res = save_tasks_to(&dir, &tasks, 3, &config);
        assert!(save_res.is_ok());

        let (loaded, next_id, loaded_config) = load_tasks_from(&dir);
        assert_eq!(next_id, 3);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "Task A");
        assert_eq!(loaded[0].created_at, "2026-11-16 18:00:00.000");
        assert!(!loaded[0].completed);
        assert_eq!(loaded[1].text, "Task B");
        assert!(loaded[1].completed);
        assert_eq!(loaded_config.record_time_seconds, 30);
        assert_eq!(loaded_config.max_depth, 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupt_json_fallback() {
        let dir = test_temp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tasks.json");
        fs::write(&path, "{ corrupt json ...").unwrap();

        let (loaded, next_id, config) = load_tasks_from(&dir);
        assert_eq!(loaded.len(), 0);
        assert_eq!(next_id, 1);
        assert_eq!(config.record_time_seconds, 7);
        assert_eq!(config.max_depth, 3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_missing_file_fallback() {
        let dir = test_temp_dir("missing");
        let (loaded, next_id, config) = load_tasks_from(&dir);
        assert_eq!(loaded.len(), 0);
        assert_eq!(next_id, 1);
        assert_eq!(config.record_time_seconds, 7);
        assert_eq!(config.max_depth, 3);
    }

    #[test]
    fn test_legacy_v1_json_compatibility() {
        let dir = test_temp_dir("legacy_v1");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tasks.json");
        let v1_json = r#"{
  "next_id": 3,
  "tasks": [
    {
      "id": 1,
      "text": "Legacy task 1",
      "completed": false
    },
    {
      "id": 2,
      "text": "Legacy task 2",
      "completed": true
    }
  ]
}"#;
        fs::write(&path, v1_json).unwrap();

        let (loaded, next_id, config) = load_tasks_from(&dir);
        assert_eq!(next_id, 3);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "Legacy task 1");
        assert!(loaded[0].sub_tasks.is_empty());
        assert!(!loaded[0].created_at.is_empty());
        assert_eq!(loaded[1].text, "Legacy task 2");
        assert!(loaded[1].sub_tasks.is_empty());
        assert_eq!(config.record_time_seconds, 7);

        let _ = fs::remove_dir_all(&dir);
    }
}
