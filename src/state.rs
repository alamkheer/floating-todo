use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, HBRUSH};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SystemTime {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

extern "system" {
    fn GetLocalTime(lpSystemTime: *mut SystemTime);
}

/// Helper returning formatted local datetime with 3 digits of fractional seconds: datetime(3).
/// Example: "2026-11-16 18:04:20.123"
pub fn current_datetime_3() -> String {
    let mut st = SystemTime::default();
    unsafe {
        GetLocalTime(&mut st);
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        st.w_year, st.w_month, st.w_day, st.w_hour, st.w_minute, st.w_second, st.w_milliseconds
    )
}

/// Diagnostic Log level
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Diagnostic log entry
#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

static LOGS: std::sync::Mutex<Vec<LogEntry>> = std::sync::Mutex::new(Vec::new());

/// Log a diagnostic event into the global in-memory ring buffer (thread-safe).
pub fn log_event(level: LogLevel, message: &str) {
    let mut st = SystemTime::default();
    unsafe {
        GetLocalTime(&mut st);
    }
    let timestamp = format!("{:02}:{:02}:{:02}", st.w_hour, st.w_minute, st.w_second);

    if let Ok(mut logs) = LOGS.lock() {
        if logs.len() >= 30 {
            logs.remove(0);
        }
        logs.push(LogEntry {
            timestamp,
            level,
            message: message.to_string(),
        });
    }
}

/// Retrieve the recent `limit` logs.
pub fn get_recent_logs(limit: usize) -> Vec<LogEntry> {
    if let Ok(logs) = LOGS.lock() {
        let count = logs.len();
        if count <= limit {
            logs.clone()
        } else {
            logs[count - limit..].to_vec()
        }
    } else {
        Vec::new()
    }
}

/// Clear diagnostic logs.
pub fn clear_logs() {
    if let Ok(mut logs) = LOGS.lock() {
        logs.clear();
    }
}

/// Application configuration persisted across sessions.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default = "default_record_time")]
    pub record_time_seconds: u32,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default = "default_is_topmost")]
    pub is_topmost: bool,
}

fn default_record_time() -> u32 {
    7
}

fn default_max_depth() -> usize {
    3
}

fn default_is_topmost() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            record_time_seconds: 7,
            max_depth: 3,
            is_topmost: true,
        }
    }
}

/// Active view mode in the main floating window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewMode {
    Tasks,
    Settings(SettingsSubPage),
}

/// Sub-pages under the Settings view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsSubPage {
    AppSettings,
    Info,
    Logs,
}

/// Represents a nested sub-task (recursively supports sub-tasks up to depth 3).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SubTask {
    pub id: u64,
    pub text: String,
    pub completed: bool,
    #[serde(default = "current_datetime_3")]
    pub created_at: String,
    #[serde(default)]
    pub sub_tasks: Vec<SubTask>,
}

/// Represents a top-level TODO task (depth 0).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub completed: bool,
    #[serde(default = "current_datetime_3")]
    pub created_at: String,
    #[serde(default)]
    pub sub_tasks: Vec<SubTask>,
}

/// A reference to an item in the flattened list with its current hierarchy depth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListItemRef {
    pub id: u64,
    pub depth: usize, // 0 = root task, 1..3 = sub-tasks
    pub root_id: u64,
}

/// Filter mode for the task list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    All,
    Active,
    Completed,
}

/// Central application state, stored in GWLP_USERDATA.
pub struct AppState {
    pub tasks: Vec<Task>,
    pub next_id: u64,
    pub filter: Filter,
    pub config: AppConfig,
    pub view_mode: ViewMode,
    pub hwnd: HWND,
    // Task View child control handles
    pub hwnd_input: HWND,
    pub hwnd_voice_btn: HWND,
    pub hwnd_add_sub_btn: HWND,
    pub hwnd_list: HWND,
    pub hwnd_filter_all: HWND,
    pub hwnd_filter_active: HWND,
    pub hwnd_filter_done: HWND,
    // Header buttons
    pub hwnd_pin_btn: HWND,
    pub hwnd_settings_btn: HWND,
    pub hwnd_close_btn: HWND,
    // Settings View child control handles
    pub hwnd_tab_settings: HWND,
    pub hwnd_tab_info: HWND,
    pub hwnd_tab_logs: HWND,
    pub hwnd_btn_back: HWND,
    pub hwnd_btn_rec_7: HWND,
    pub hwnd_btn_rec_15: HWND,
    pub hwnd_btn_rec_30: HWND,
    pub hwnd_btn_depth_1: HWND,
    pub hwnd_btn_depth_2: HWND,
    pub hwnd_btn_depth_3: HWND,
    pub hwnd_btn_test_mic: HWND,
    pub hwnd_btn_clear_logs: HWND,
    // Tooltips & GDI Brushes
    pub hwnd_tooltip: HWND,
    pub input_brush: HBRUSH,
    pub list_brush: HBRUSH,
    // Persistence and interaction state
    pub dirty: bool,
    pub hover_item: i32,
    pub selected_task_id: Option<u64>,
    pub sub_task_input_mode: bool,
    pub is_recording_voice: bool,
    pub voice_seconds_remaining: u32,
    pub current_list: Vec<ListItemRef>,
}

impl AppState {
    pub fn new() -> Self {
        let input_brush = unsafe { CreateSolidBrush(COLORREF(crate::ui::CLR_INPUT_BG)) };
        let list_brush = unsafe { CreateSolidBrush(COLORREF(crate::ui::CLR_BG_DARK)) };

        Self {
            tasks: Vec::new(),
            next_id: 1,
            filter: Filter::All,
            config: AppConfig::default(),
            view_mode: ViewMode::Tasks,
            hwnd: HWND::default(),
            hwnd_input: HWND::default(),
            hwnd_voice_btn: HWND::default(),
            hwnd_add_sub_btn: HWND::default(),
            hwnd_list: HWND::default(),
            hwnd_filter_all: HWND::default(),
            hwnd_filter_active: HWND::default(),
            hwnd_filter_done: HWND::default(),
            hwnd_pin_btn: HWND::default(),
            hwnd_settings_btn: HWND::default(),
            hwnd_close_btn: HWND::default(),
            hwnd_tab_settings: HWND::default(),
            hwnd_tab_info: HWND::default(),
            hwnd_tab_logs: HWND::default(),
            hwnd_btn_back: HWND::default(),
            hwnd_btn_rec_7: HWND::default(),
            hwnd_btn_rec_15: HWND::default(),
            hwnd_btn_rec_30: HWND::default(),
            hwnd_btn_depth_1: HWND::default(),
            hwnd_btn_depth_2: HWND::default(),
            hwnd_btn_depth_3: HWND::default(),
            hwnd_btn_test_mic: HWND::default(),
            hwnd_btn_clear_logs: HWND::default(),
            hwnd_tooltip: HWND::default(),
            input_brush,
            list_brush,
            dirty: false,
            hover_item: -1,
            selected_task_id: None,
            sub_task_input_mode: false,
            is_recording_voice: false,
            voice_seconds_remaining: 0,
            current_list: Vec::new(),
        }
    }

    /// Filtered list items (parent tasks and nested sub-tasks) for listbox display.
    pub fn visible_items(&self) -> Vec<ListItemRef> {
        let mut items = Vec::new();
        for task in &self.tasks {
            let show_task = match self.filter {
                Filter::All => true,
                Filter::Active => !task.completed,
                Filter::Completed => task.completed,
            };
            if show_task {
                items.push(ListItemRef {
                    id: task.id,
                    depth: 0,
                    root_id: task.id,
                });
                self.collect_visible_subs(&task.sub_tasks, 1, task.id, &mut items);
            }
        }
        items
    }

    fn collect_visible_subs(
        &self,
        subs: &[SubTask],
        depth: usize,
        root_id: u64,
        items: &mut Vec<ListItemRef>,
    ) {
        if depth > self.config.max_depth {
            return;
        }
        for sub in subs {
            let show_sub = match self.filter {
                Filter::All => true,
                Filter::Active => !sub.completed,
                Filter::Completed => sub.completed,
            };
            if show_sub {
                items.push(ListItemRef {
                    id: sub.id,
                    depth,
                    root_id,
                });
                self.collect_visible_subs(&sub.sub_tasks, depth + 1, root_id, items);
            }
        }
    }

    #[allow(dead_code)]
    pub fn find_task(&self, id: u64) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Find an item's details (text, completed status, created_at timestamp, depth) by ID anywhere in the hierarchy.
    pub fn find_item_details(&self, id: u64) -> Option<(String, bool, String, usize, bool)> {
        for task in &self.tasks {
            if task.id == id {
                let has_children = !task.sub_tasks.is_empty();
                return Some((
                    task.text.clone(),
                    task.completed,
                    task.created_at.clone(),
                    0,
                    has_children,
                ));
            }
            if let Some(res) = Self::find_sub_details(&task.sub_tasks, id, 1) {
                return Some(res);
            }
        }
        None
    }

    fn find_sub_details(
        subs: &[SubTask],
        id: u64,
        depth: usize,
    ) -> Option<(String, bool, String, usize, bool)> {
        for sub in subs {
            if sub.id == id {
                let has_children = !sub.sub_tasks.is_empty();
                return Some((
                    sub.text.clone(),
                    sub.completed,
                    sub.created_at.clone(),
                    depth,
                    has_children,
                ));
            }
            if let Some(res) = Self::find_sub_details(&sub.sub_tasks, id, depth + 1) {
                return Some(res);
            }
        }
        None
    }

    /// Retrieve the depth of an item by its ID.
    #[allow(dead_code)]
    pub fn get_item_depth(&self, id: u64) -> Option<usize> {
        self.find_item_details(id).map(|(_, _, _, depth, _)| depth)
    }

    /// Add a top-level task (depth 0) with a millisecond timestamp.
    pub fn add_task(&mut self, text: String) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task {
            id,
            text: trimmed.to_string(),
            completed: false,
            created_at: current_datetime_3(),
            sub_tasks: Vec::new(),
        });
        self.dirty = true;
        log_event(LogLevel::Info, &format!("Added task #{}: \"{}\"", id, trimmed));
    }

    /// Add a sub-task under a target parent (root task or existing sub-task) if target depth < max_depth.
    pub fn add_sub_task(&mut self, parent_id: u64, text: String) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        let sub_id = self.next_id;
        let created_at = current_datetime_3();
        let max_depth = self.config.max_depth;

        // Try adding under root tasks
        for task in &mut self.tasks {
            if task.id == parent_id {
                if 1 > max_depth {
                    return false;
                }
                task.sub_tasks.push(SubTask {
                    id: sub_id,
                    text: trimmed.to_string(),
                    completed: false,
                    created_at,
                    sub_tasks: Vec::new(),
                });
                task.completed = false;
                self.next_id += 1;
                self.dirty = true;
                Self::recompute_completion(task);
                log_event(
                    LogLevel::Info,
                    &format!("Added sub-task #{} under task #{}", sub_id, parent_id),
                );
                return true;
            }

            if Self::add_sub_recursive(
                &mut task.sub_tasks,
                parent_id,
                sub_id,
                trimmed,
                &created_at,
                1,
                max_depth,
            ) {
                self.next_id += 1;
                self.dirty = true;
                Self::recompute_completion(task);
                log_event(
                    LogLevel::Info,
                    &format!("Added nested sub-task #{} under parent #{}", sub_id, parent_id),
                );
                return true;
            }
        }
        false
    }

    fn add_sub_recursive(
        subs: &mut [SubTask],
        parent_id: u64,
        sub_id: u64,
        text: &str,
        created_at: &str,
        current_depth: usize,
        max_depth: usize,
    ) -> bool {
        for sub in subs {
            if sub.id == parent_id {
                if current_depth + 1 > max_depth {
                    return false;
                }
                sub.sub_tasks.push(SubTask {
                    id: sub_id,
                    text: text.to_string(),
                    completed: false,
                    created_at: created_at.to_string(),
                    sub_tasks: Vec::new(),
                });
                sub.completed = false;
                return true;
            }
            if Self::add_sub_recursive(
                &mut sub.sub_tasks,
                parent_id,
                sub_id,
                text,
                created_at,
                current_depth + 1,
                max_depth,
            ) {
                return true;
            }
        }
        false
    }

    /// Toggle completion of any item (root task or sub-task).
    /// Cascades completion to all descendants and recomputes all ancestor completion.
    pub fn toggle_item(&mut self, id: u64) {
        fn set_descendants(subs: &mut [SubTask], state: bool) {
            for sub in subs {
                sub.completed = state;
                set_descendants(&mut sub.sub_tasks, state);
            }
        }

        let mut found = false;
        for task in &mut self.tasks {
            if task.id == id {
                task.completed = !task.completed;
                let new_st = task.completed;
                set_descendants(&mut task.sub_tasks, new_st);
                found = true;
                break;
            }

            fn toggle_in_subs(subs: &mut [SubTask], id: u64) -> bool {
                for sub in subs {
                    if sub.id == id {
                        sub.completed = !sub.completed;
                        let new_st = sub.completed;
                        set_descendants(&mut sub.sub_tasks, new_st);
                        return true;
                    }
                    if toggle_in_subs(&mut sub.sub_tasks, id) {
                        return true;
                    }
                }
                false
            }

            if toggle_in_subs(&mut task.sub_tasks, id) {
                found = true;
                break;
            }
        }

        if found {
            for task in &mut self.tasks {
                Self::recompute_completion(task);
            }
            self.dirty = true;
        }
    }

    /// Delete an item (root task or nested sub-task) anywhere in the hierarchy.
    pub fn delete_item(&mut self, id: u64) {
        let before_len = self.tasks.len();
        self.tasks.retain(|t| t.id != id);
        let mut deleted = self.tasks.len() != before_len;

        if !deleted {
            fn del_in_subs(subs: &mut Vec<SubTask>, id: u64) -> bool {
                let b = subs.len();
                subs.retain(|s| s.id != id);
                if subs.len() != b {
                    return true;
                }
                for s in subs {
                    if del_in_subs(&mut s.sub_tasks, id) {
                        return true;
                    }
                }
                false
            }

            for task in &mut self.tasks {
                if del_in_subs(&mut task.sub_tasks, id) {
                    deleted = true;
                    break;
                }
            }
        }

        if deleted {
            if self.selected_task_id == Some(id) {
                self.selected_task_id = None;
                self.sub_task_input_mode = false;
            }
            for task in &mut self.tasks {
                Self::recompute_completion(task);
            }
            self.dirty = true;
            log_event(LogLevel::Info, &format!("Deleted item #{}", id));
        }
    }

    /// Indent the item (Tab key): nests the item under its immediately preceding sibling.
    /// Supports nesting up to `config.max_depth` (max allowed: 3).
    pub fn indent_item(&mut self, id: u64) -> bool {
        let max_depth = self.config.max_depth;

        // 1. Check if it's a top-level task
        if let Some(idx) = self.tasks.iter().position(|t| t.id == id) {
            if idx == 0 {
                // First task has no preceding sibling to indent under
                return false;
            }
            if 1 > max_depth {
                return false;
            }
            let task_to_indent = self.tasks.remove(idx);
            let prev_task = &mut self.tasks[idx - 1];

            let sub = SubTask {
                id: task_to_indent.id,
                text: task_to_indent.text,
                completed: task_to_indent.completed,
                created_at: task_to_indent.created_at,
                sub_tasks: task_to_indent.sub_tasks,
            };
            prev_task.sub_tasks.push(sub);
            Self::recompute_completion(prev_task);
            self.dirty = true;
            log_event(
                LogLevel::Info,
                &format!("Indented task #{} under task #{}", id, prev_task.id),
            );
            return true;
        }

        // 2. Check within nested sub-tasks
        for task in &mut self.tasks {
            if Self::indent_in_subs(&mut task.sub_tasks, id, 1, max_depth) {
                Self::recompute_completion(task);
                self.dirty = true;
                log_event(LogLevel::Info, &format!("Indented sub-task #{}", id));
                return true;
            }
        }

        false
    }

    fn indent_in_subs(
        subs: &mut Vec<SubTask>,
        id: u64,
        current_depth: usize,
        max_depth: usize,
    ) -> bool {
        if let Some(idx) = subs.iter().position(|s| s.id == id) {
            if idx == 0 {
                return false;
            }
            if current_depth + 1 > max_depth {
                return false;
            }
            let sub_to_indent = subs.remove(idx);
            let prev_sibling = &mut subs[idx - 1];
            prev_sibling.sub_tasks.push(sub_to_indent);
            return true;
        }

        for sub in subs {
            if Self::indent_in_subs(&mut sub.sub_tasks, id, current_depth + 1, max_depth) {
                return true;
            }
        }
        false
    }

    /// Outdent the item (Shift+Tab key): lifts the item one level higher towards root.
    pub fn outdent_item(&mut self, id: u64) -> bool {
        // 1. If it's already a top-level task (depth 0), it cannot be outdented
        if self.tasks.iter().any(|t| t.id == id) {
            return false;
        }

        // 2. Check if it's a direct child of a top-level task (depth 1 -> depth 0)
        for task_idx in 0..self.tasks.len() {
            if let Some(sub_idx) = self.tasks[task_idx].sub_tasks.iter().position(|s| s.id == id) {
                let sub = self.tasks[task_idx].sub_tasks.remove(sub_idx);
                let new_root = Task {
                    id: sub.id,
                    text: sub.text,
                    completed: sub.completed,
                    created_at: sub.created_at,
                    sub_tasks: sub.sub_tasks,
                };
                self.tasks.insert(task_idx + 1, new_root);
                Self::recompute_completion(&mut self.tasks[task_idx]);
                self.dirty = true;
                log_event(LogLevel::Info, &format!("Outdented sub-task #{} to root task", id));
                return true;
            }
        }

        // 3. Check for deeper nesting (depth 3 -> depth 2, or depth 2 -> depth 1)
        for task in &mut self.tasks {
            if Self::outdent_in_subs(&mut task.sub_tasks, id) {
                Self::recompute_completion(task);
                self.dirty = true;
                log_event(LogLevel::Info, &format!("Outdented sub-task #{}", id));
                return true;
            }
        }

        false
    }

    fn outdent_in_subs(subs: &mut Vec<SubTask>, id: u64) -> bool {
        for parent_idx in 0..subs.len() {
            if let Some(sub_idx) = subs[parent_idx].sub_tasks.iter().position(|s| s.id == id) {
                let child = subs[parent_idx].sub_tasks.remove(sub_idx);
                subs.insert(parent_idx + 1, child);
                return true;
            }
            if Self::outdent_in_subs(&mut subs[parent_idx].sub_tasks, id) {
                return true;
            }
        }
        false
    }

    /// Recomputes bottom-up completion status: a parent is completed iff all children are completed.
    fn recompute_completion(task: &mut Task) {
        for sub in &mut task.sub_tasks {
            Self::recompute_sub_completion(sub);
        }
        if !task.sub_tasks.is_empty() {
            task.completed = task.sub_tasks.iter().all(|s| s.completed);
        }
    }

    fn recompute_sub_completion(sub: &mut SubTask) {
        for child in &mut sub.sub_tasks {
            Self::recompute_sub_completion(child);
        }
        if !sub.sub_tasks.is_empty() {
            sub.completed = sub.sub_tasks.iter().all(|c| c.completed);
        }
    }

    // Compatibility wrappers for existing code
    #[allow(dead_code)]
    pub fn toggle_task(&mut self, id: u64) {
        self.toggle_item(id);
    }

    #[allow(dead_code)]
    pub fn toggle_sub_task(&mut self, _parent_id: u64, sub_id: u64) {
        self.toggle_item(sub_id);
    }

    #[allow(dead_code)]
    pub fn delete_task(&mut self, id: u64) {
        self.delete_item(id);
    }

    #[allow(dead_code)]
    pub fn delete_sub_task(&mut self, _parent_id: u64, sub_id: u64) {
        self.delete_item(sub_id);
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.input_brush.into());
            let _ = DeleteObject(self.list_brush.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_add_and_timestamp() {
        let mut state = AppState::new();
        state.add_task("  Test task  ".to_string());
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.tasks[0].id, 1);
        assert_eq!(state.tasks[0].text, "Test task");
        assert!(!state.tasks[0].completed);
        assert!(!state.tasks[0].created_at.is_empty());
        assert_eq!(state.tasks[0].created_at.len(), 23); // "YYYY-MM-DD HH:mm:ss.fff"
    }

    #[test]
    fn test_nested_subtasks_depth_3() {
        let mut state = AppState::new();
        state.config.max_depth = 3;
        state.add_task("Root 1".to_string());
        let root_id = 1;

        // Add depth 1
        assert!(state.add_sub_task(root_id, "Sub 1".to_string()));
        let sub1_id = 2;

        // Add depth 2
        assert!(state.add_sub_task(sub1_id, "Sub 2".to_string()));
        let sub2_id = 3;

        // Add depth 3
        assert!(state.add_sub_task(sub2_id, "Sub 3".to_string()));
        let sub3_id = 4;

        // Attempting depth 4 should fail because max_depth is 3
        assert!(!state.add_sub_task(sub3_id, "Sub 4".to_string()));

        let items = state.visible_items();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].depth, 0);
        assert_eq!(items[1].depth, 1);
        assert_eq!(items[2].depth, 2);
        assert_eq!(items[3].depth, 3);
    }

    #[test]
    fn test_tab_indent_and_shift_tab_outdent() {
        let mut state = AppState::new();
        state.config.max_depth = 3;
        state.add_task("Task 1".to_string());
        state.add_task("Task 2".to_string());

        // Indent Task 2 under Task 1
        assert!(state.indent_item(2));
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.tasks[0].sub_tasks.len(), 1);
        assert_eq!(state.tasks[0].sub_tasks[0].id, 2);

        // Outdent Task 2 back to root
        assert!(state.outdent_item(2));
        assert_eq!(state.tasks.len(), 2);
        assert_eq!(state.tasks[0].id, 1);
        assert_eq!(state.tasks[1].id, 2);
    }

    #[test]
    fn test_max_depth_enforcement() {
        let mut state = AppState::new();
        state.config.max_depth = 1;
        state.add_task("Task 1".to_string());
        state.add_sub_task(1, "Sub 1".to_string());

        // Under max_depth = 1, Sub 1 is at depth 1. It cannot be nested further
        assert!(!state.add_sub_task(2, "Sub 2".to_string()));

        // Under max_depth = 1, indenting Sub 1 deeper must return false
        assert!(!state.indent_item(2));
    }

    #[test]
    fn test_recursive_completion_cascade() {
        let mut state = AppState::new();
        state.config.max_depth = 3;
        state.add_task("Root".to_string());
        state.add_sub_task(1, "Sub 1".to_string());
        state.add_sub_task(2, "Sub 2".to_string());

        // Complete the deep leaf: Sub 2
        state.toggle_item(3);
        assert!(state.find_item_details(3).unwrap().1); // Sub 2 completed
        assert!(state.find_item_details(2).unwrap().1); // Sub 1 completed
        assert!(state.find_task(1).unwrap().completed); // Root completed

        // Uncomplete Sub 2 -> root and parent uncomplete
        state.toggle_item(3);
        assert!(!state.find_item_details(3).unwrap().1);
        assert!(!state.find_item_details(2).unwrap().1);
        assert!(!state.find_task(1).unwrap().completed);
    }

    #[test]
    fn test_diagnostic_logger_ring_buffer() {
        clear_logs();
        for i in 0..10 {
            log_event(LogLevel::Info, &format!("Event {}", i));
        }
        let recent = get_recent_logs(5);
        assert_eq!(recent.len(), 5);
        assert_eq!(recent[0].message, "Event 5");
        assert_eq!(recent[4].message, "Event 9");
        clear_logs();
        assert_eq!(get_recent_logs(5).len(), 0);
    }
}
