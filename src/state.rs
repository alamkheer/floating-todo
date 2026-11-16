use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, HBRUSH};

/// Represents a nested sub-task under a parent task.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SubTask {
    pub id: u64,
    pub text: String,
    pub completed: bool,
}

/// Represents a single TODO task.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub completed: bool,
    #[serde(default)]
    pub sub_tasks: Vec<SubTask>,
}

/// A reference to either a parent task or a sub-task for list rendering and interaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ListItemRef {
    Parent(u64),
    SubTask { parent_id: u64, sub_id: u64 },
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
    pub hwnd_voice_btn: HWND,
    pub hwnd_add_sub_btn: HWND,
    pub hwnd_list: HWND,
    pub hwnd_pin_btn: HWND,
    pub hwnd_close_btn: HWND,
    pub hwnd_filter_all: HWND,
    pub hwnd_filter_active: HWND,
    pub hwnd_filter_done: HWND,
    pub hwnd_tooltip: HWND,
    // GDI Brushes for child controls
    pub input_brush: HBRUSH,
    pub list_brush: HBRUSH,
    // Dirty flag for debounced persistence
    pub dirty: bool,
    // Hover tracking for list items
    pub hover_item: i32,
    // Selection and sub-task input state
    pub selected_task_id: Option<u64>,
    pub sub_task_input_mode: bool,
    // Voice recording state
    pub is_recording_voice: bool,
    pub voice_seconds_remaining: u32,
    // Flattened list currently displayed in the listbox
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
            is_topmost: true,
            hwnd: HWND::default(),
            hwnd_input: HWND::default(),
            hwnd_voice_btn: HWND::default(),
            hwnd_add_sub_btn: HWND::default(),
            hwnd_list: HWND::default(),
            hwnd_pin_btn: HWND::default(),
            hwnd_close_btn: HWND::default(),
            hwnd_filter_all: HWND::default(),
            hwnd_filter_active: HWND::default(),
            hwnd_filter_done: HWND::default(),
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

    /// Filtered list items (both parent tasks and their sub-tasks) for listbox display.
    pub fn visible_items(&self) -> Vec<ListItemRef> {
        let mut items = Vec::new();
        for task in &self.tasks {
            let show_task = match self.filter {
                Filter::All => true,
                Filter::Active => !task.completed,
                Filter::Completed => task.completed,
            };
            if show_task {
                items.push(ListItemRef::Parent(task.id));
                for sub in &task.sub_tasks {
                    items.push(ListItemRef::SubTask {
                        parent_id: task.id,
                        sub_id: sub.id,
                    });
                }
            }
        }
        items
    }

    pub fn find_task(&self, id: u64) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    #[allow(dead_code)]
    pub fn find_task_mut(&mut self, id: u64) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

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
            sub_tasks: Vec::new(),
        });
        self.dirty = true;
    }

    pub fn add_sub_task(&mut self, parent_id: u64, text: String) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        let sub_id = self.next_id;
        let mut added = false;
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == parent_id) {
            task.sub_tasks.push(SubTask {
                id: sub_id,
                text: trimmed.to_string(),
                completed: false,
            });
            if task.completed {
                task.completed = false;
            }
            added = true;
        }
        if added {
            self.next_id += 1;
            self.dirty = true;
        }
        added
    }

    /// Toggles a task. When parent toggles, all its sub-tasks match the parent's new completion status.
    pub fn toggle_task(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.completed = !task.completed;
            let new_status = task.completed;
            for sub in &mut task.sub_tasks {
                sub.completed = new_status;
            }
            self.dirty = true;
        }
    }

    /// Toggles a sub-task. If all sub-tasks are complete, parent also completes.
    /// If any sub-task is incomplete, parent becomes incomplete.
    pub fn toggle_sub_task(&mut self, parent_id: u64, sub_id: u64) {
        let mut toggled = false;
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == parent_id) {
            if let Some(sub) = task.sub_tasks.iter_mut().find(|s| s.id == sub_id) {
                sub.completed = !sub.completed;
                toggled = true;
            }
            if !task.sub_tasks.is_empty() {
                task.completed = task.sub_tasks.iter().all(|s| s.completed);
            }
        }
        if toggled {
            self.dirty = true;
        }
    }

    pub fn delete_task(&mut self, id: u64) {
        self.tasks.retain(|t| t.id != id);
        if self.selected_task_id == Some(id) {
            self.selected_task_id = None;
            self.sub_task_input_mode = false;
        }
        self.dirty = true;
    }

    pub fn delete_sub_task(&mut self, parent_id: u64, sub_id: u64) {
        let mut deleted = false;
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == parent_id) {
            let before = task.sub_tasks.len();
            task.sub_tasks.retain(|s| s.id != sub_id);
            if task.sub_tasks.len() != before {
                deleted = true;
                if !task.sub_tasks.is_empty() {
                    task.completed = task.sub_tasks.iter().all(|s| s.completed);
                }
            }
        }
        if deleted {
            self.dirty = true;
        }
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
        assert!(state.tasks[0].sub_tasks.is_empty());
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
    fn test_sub_task_add_and_cascade_complete() {
        let mut state = AppState::new();
        state.add_task("Parent Task".to_string());

        // Add sub-tasks
        assert!(state.add_sub_task(1, "Sub 1".to_string()));
        assert!(state.add_sub_task(1, "Sub 2".to_string()));

        let parent = state.find_task(1).unwrap();
        assert_eq!(parent.sub_tasks.len(), 2);
        assert_eq!(parent.sub_tasks[0].text, "Sub 1");
        assert_eq!(parent.sub_tasks[1].text, "Sub 2");
        assert!(!parent.completed);

        // Complete 1st sub-task -> parent should remain incomplete
        state.toggle_sub_task(1, parent.sub_tasks[0].id);
        let parent = state.find_task(1).unwrap();
        assert!(parent.sub_tasks[0].completed);
        assert!(!parent.sub_tasks[1].completed);
        assert!(!parent.completed);

        // Complete 2nd sub-task -> all sub-tasks complete, parent MUST automatically complete!
        let sub2_id = parent.sub_tasks[1].id;
        state.toggle_sub_task(1, sub2_id);
        let parent = state.find_task(1).unwrap();
        assert!(parent.sub_tasks[1].completed);
        assert!(parent.completed, "Parent must complete when all sub-tasks are complete");

        // Uncomplete 1st sub-task -> parent MUST automatically uncomplete
        let sub1_id = parent.sub_tasks[0].id;
        state.toggle_sub_task(1, sub1_id);
        let parent = state.find_task(1).unwrap();
        assert!(!parent.sub_tasks[0].completed);
        assert!(!parent.completed, "Parent must uncomplete when any sub-task is uncompleted");

        // Completing parent directly should complete all sub-tasks
        state.toggle_task(1);
        let parent = state.find_task(1).unwrap();
        assert!(parent.completed);
        assert!(parent.sub_tasks.iter().all(|s| s.completed));
    }

    #[test]
    fn test_sub_task_delete_rechecks_parent_completion() {
        let mut state = AppState::new();
        state.add_task("Parent Task".to_string());
        state.add_sub_task(1, "Sub 1".to_string());
        state.add_sub_task(1, "Sub 2".to_string());

        let parent = state.find_task(1).unwrap();
        let sub1_id = parent.sub_tasks[0].id;
        let sub2_id = parent.sub_tasks[1].id;

        // Complete Sub 1
        state.toggle_sub_task(1, sub1_id);
        assert!(!state.find_task(1).unwrap().completed);

        // Delete incomplete Sub 2 -> only Sub 1 remains and it is completed -> parent should complete
        state.delete_sub_task(1, sub2_id);
        let parent = state.find_task(1).unwrap();
        assert_eq!(parent.sub_tasks.len(), 1);
        assert!(parent.completed);
    }

    #[test]
    fn test_visible_items_filter() {
        let mut state = AppState::new();
        state.add_task("Task 1".to_string());
        let t1_id = state.tasks.last().unwrap().id;
        state.add_sub_task(t1_id, "Sub 1-1".to_string());
        let sub1_id = state.tasks.last().unwrap().sub_tasks.last().unwrap().id;
        state.add_task("Task 2".to_string());
        let t2_id = state.tasks.last().unwrap().id;

        // Mark Task 2 as completed
        state.toggle_task(t2_id);

        // Filter::All should show both tasks and the sub-task
        state.filter = Filter::All;
        let items = state.visible_items();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], ListItemRef::Parent(t1_id));
        assert_eq!(items[1], ListItemRef::SubTask { parent_id: t1_id, sub_id: sub1_id });
        assert_eq!(items[2], ListItemRef::Parent(t2_id));

        // Filter::Active should show Task 1 and Sub 1-1
        state.filter = Filter::Active;
        let active = state.visible_items();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0], ListItemRef::Parent(t1_id));
        assert_eq!(active[1], ListItemRef::SubTask { parent_id: t1_id, sub_id: sub1_id });

        // Filter::Completed should show Task 2
        state.filter = Filter::Completed;
        let completed = state.visible_items();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], ListItemRef::Parent(t2_id));
    }
}
