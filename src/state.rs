use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, HBRUSH};

/// Represents a single TODO task.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub completed: bool,
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
    pub is_topmost: bool,
    pub hwnd: HWND,
    // Child control handles
    pub hwnd_input: HWND,
    pub hwnd_list: HWND,
    pub hwnd_pin_btn: HWND,
    pub hwnd_close_btn: HWND,
    pub hwnd_filter_all: HWND,
    pub hwnd_filter_active: HWND,
    pub hwnd_filter_done: HWND,
    // GDI Brushes for child controls
    pub input_brush: HBRUSH,
    pub list_brush: HBRUSH,
    // Dirty flag for debounced persistence
    pub dirty: bool,
    // Hover tracking for list items
    pub hover_item: i32,
}

impl AppState {
    pub fn new() -> Self {
        let input_brush = unsafe { CreateSolidBrush(COLORREF(crate::ui::CLR_INPUT_BG)) };
        let list_brush = unsafe { CreateSolidBrush(COLORREF(crate::ui::CLR_BG_DARK)) };

        Self {
            tasks: Vec::new(),
            next_id: 1,
            filter: Filter::All,
            is_topmost: true,
            hwnd: HWND::default(),
            hwnd_input: HWND::default(),
            hwnd_list: HWND::default(),
            hwnd_pin_btn: HWND::default(),
            hwnd_close_btn: HWND::default(),
            hwnd_filter_all: HWND::default(),
            hwnd_filter_active: HWND::default(),
            hwnd_filter_done: HWND::default(),
            input_brush,
            list_brush,
            dirty: false,
            hover_item: -1,
        }
    }

    /// Filtered view of tasks based on current filter mode.
    pub fn visible_tasks(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| match self.filter {
                Filter::All => true,
                Filter::Active => !t.completed,
                Filter::Completed => t.completed,
            })
            .collect()
    }

    pub fn add_task(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task {
            id,
            text: text.trim().to_string(),
            completed: false,
        });
        self.dirty = true;
    }

    pub fn toggle_task(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.completed = !task.completed;
            self.dirty = true;
        }
    }

    pub fn delete_task(&mut self, id: u64) {
        self.tasks.retain(|t| t.id != id);
        self.dirty = true;
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
    fn test_task_add() {
        let mut state = AppState::new();
        assert_eq!(state.tasks.len(), 0);

        // Add valid tasks
        state.add_task("  First task  ".to_string());
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.tasks[0].id, 1);
        assert_eq!(state.tasks[0].text, "First task");
        assert!(!state.tasks[0].completed);
        assert!(state.dirty);

        state.add_task("Second task".to_string());
        assert_eq!(state.tasks.len(), 2);
        assert_eq!(state.tasks[1].id, 2);

        // Blank/whitespace should be ignored
        state.dirty = false;
        state.add_task("   ".to_string());
        assert_eq!(state.tasks.len(), 2);
        assert!(!state.dirty);
    }

    #[test]
    fn test_task_toggle() {
        let mut state = AppState::new();
        state.add_task("Toggle me".to_string());
        assert!(!state.tasks[0].completed);

        state.toggle_task(1);
        assert!(state.tasks[0].completed);
        assert!(state.dirty);

        state.toggle_task(1);
        assert!(!state.tasks[0].completed);

        // Non-existent ID is graceful no-op
        state.toggle_task(999);
    }

    #[test]
    fn test_task_delete() {
        let mut state = AppState::new();
        state.add_task("Task 1".to_string());
        state.add_task("Task 2".to_string());
        assert_eq!(state.tasks.len(), 2);

        state.delete_task(1);
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.tasks[0].id, 2);
        assert!(state.dirty);

        // Non-existent ID is graceful no-op
        state.delete_task(999);
        assert_eq!(state.tasks.len(), 1);
    }

    #[test]
    fn test_visible_tasks_filter() {
        let mut state = AppState::new();
        state.add_task("Task 1".to_string());
        state.add_task("Task 2".to_string());
        state.add_task("Task 3".to_string());
        state.toggle_task(2); // Complete Task 2

        state.filter = Filter::All;
        assert_eq!(state.visible_tasks().len(), 3);

        state.filter = Filter::Active;
        let active = state.visible_tasks();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].id, 1);
        assert_eq!(active[1].id, 3);

        state.filter = Filter::Completed;
        let completed = state.visible_tasks();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, 2);
    }
}

