use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::layout::Rect;

use crate::{
    anim::Animations,
    model::{Project, TaskStatus},
    storage,
    theme::{Theme, THEMES},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Groups,
    Composer,
    Tasks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    EditingCapture,
    EditingTask,
    CreatingGroup,
    RenamingGroup,
    ConfirmClearClosed,
    ConfirmDeleteGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskMotion {
    Added,
    Closed,
    Reopened,
}

pub struct App {
    pub project: Project,
    pub project_path: PathBuf,
    pub focus: Pane,
    pub mode: Mode,
    pub active_group: usize,
    pub selected_group: usize,
    pub selected_task: usize,
    pub composer: String,
    pub composer_cursor: usize,
    pub group_input: String,
    pub group_input_cursor: usize,
    pub status: String,
    pub should_quit: bool,
    pub animations: Animations,
    pub recent_task_index: Option<usize>,
    pub recent_task_motion: Option<TaskMotion>,
    pub theme_index: usize,
    pub undo_snapshot: Option<Project>,
    // Updated each frame by draw(); used by mouse handler.
    pub groups_outer: Rect,
    pub composer_outer: Rect,
    pub tasks_outer: Rect,
    pub tasks_inner: Rect,
    pub tasks_scroll_top: usize,
    pub task_visual_starts: Vec<usize>,
    pub total_task_visual_rows: usize,
}

impl App {
    pub fn new(project: Project, project_path: PathBuf) -> Self {
        let theme_index = project.theme_index;
        Self {
            project,
            project_path,
            focus: Pane::Composer,
            mode: Mode::Normal,
            active_group: 0,
            selected_group: 0,
            selected_task: 0,
            composer: String::new(),
            composer_cursor: 0,
            group_input: String::new(),
            group_input_cursor: 0,
            status: "Ready".into(),
            should_quit: false,
            animations: Animations::new(),
            recent_task_index: None,
            recent_task_motion: None,
            theme_index,
            undo_snapshot: None,
            groups_outer: Rect::default(),
            composer_outer: Rect::default(),
            tasks_outer: Rect::default(),
            tasks_inner: Rect::default(),
            tasks_scroll_top: 0,
            task_visual_starts: Vec::new(),
            total_task_visual_rows: 0,
        }
    }

    pub fn tick(&mut self) {
        self.animations.tick();
    }

    pub fn theme(&self) -> &'static Theme {
        &THEMES[self.theme_index]
    }

    fn cycle_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % THEMES.len();
        self.project.theme_index = self.theme_index;
        self.set_status(format!("Theme: {}", self.theme().name));
        self.persist().ok();
    }

    fn take_undo_snapshot(&mut self) {
        self.undo_snapshot = Some(self.project.clone());
    }

    fn undo(&mut self) -> Result<()> {
        if let Some(snapshot) = self.undo_snapshot.take() {
            self.project = snapshot;
            self.active_group =
                self.active_group.min(self.project.groups.len().saturating_sub(1));
            self.selected_group =
                self.selected_group.min(self.project.groups.len().saturating_sub(1));
            let task_count = self
                .project
                .groups
                .get(self.active_group)
                .map(|g| g.tasks.len())
                .unwrap_or(0);
            self.selected_task = self.selected_task.min(task_count.saturating_sub(1));
            self.recent_task_index = None;
            self.set_status("Undone");
            self.persist()
        } else {
            self.invalid("Nothing to undo");
            Ok(())
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent) -> Result<()> {
        let x = event.column;
        let y = event.row;
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(self.tasks_outer, x, y) {
                    self.set_focus(Pane::Tasks);
                    let inner = self.tasks_inner;
                    if y >= inner.y && x >= inner.x && x < inner.x + inner.width {
                        let visual_row =
                            (y - inner.y) as usize + self.tasks_scroll_top;
                        let idx = self
                            .task_visual_starts
                            .partition_point(|&s| s <= visual_row);
                        if idx > 0 {
                            let task_idx = idx - 1;
                            let task_count = self
                                .project
                                .groups
                                .get(self.active_group)
                                .map(|g| g.tasks.len())
                                .unwrap_or(0);
                            if task_idx < task_count {
                                self.selected_task = task_idx;
                            }
                        }
                    }
                } else if rect_contains(self.groups_outer, x, y) {
                    self.set_focus(Pane::Groups);
                } else if rect_contains(self.composer_outer, x, y) {
                    self.set_focus(Pane::Composer);
                }
            }
            MouseEventKind::ScrollDown => {
                if rect_contains(self.tasks_outer, x, y) {
                    self.navigate_down_tasks();
                } else if rect_contains(self.groups_outer, x, y) {
                    let last = self.project.groups.len().saturating_sub(1);
                    self.selected_group = (self.selected_group + 1).min(last);
                }
            }
            MouseEventKind::ScrollUp => {
                if rect_contains(self.tasks_outer, x, y) {
                    self.navigate_up_tasks();
                } else if rect_contains(self.groups_outer, x, y) {
                    self.selected_group = self.selected_group.saturating_sub(1);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn current_group_task_count(&self) -> usize {
        self.project
            .groups
            .get(self.active_group)
            .map(|group| group.tasks.len())
            .unwrap_or(0)
    }

    pub fn on_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.on_key(key),
            Event::Paste(text) => self.on_paste(text),
            Event::Mouse(mouse) => self.on_mouse(mouse),
            _ => Ok(()),
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }

        match self.mode {
            Mode::EditingCapture => return self.handle_capture_edit_key(key),
            Mode::EditingTask => return self.handle_task_edit_key(key),
            Mode::CreatingGroup => return self.handle_group_creation_key(key),
            Mode::RenamingGroup => return self.handle_group_rename_key(key),
            Mode::ConfirmClearClosed => return self.handle_clear_closed_confirmation_key(key),
            Mode::ConfirmDeleteGroup => return self.handle_delete_group_confirmation_key(key),
            Mode::Normal => {}
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('d') => self.begin_clear_closed(),
            KeyCode::Char('u') => self.undo()?,
            KeyCode::Char('t') => self.cycle_theme(),
            KeyCode::Char('1') => self.set_focus(Pane::Groups),
            KeyCode::Char('2') => self.set_focus(Pane::Composer),
            KeyCode::Char('3') => self.set_focus(Pane::Tasks),
            KeyCode::Up => self.navigate_up(),
            KeyCode::Down => self.navigate_down(),
            KeyCode::Left => self.navigate_left(),
            KeyCode::Right => self.navigate_right(),
            KeyCode::Esc => self.clear_status("Cancelled"),
            _ => self.handle_focused_key(key)?,
        }

        Ok(())
    }

    fn handle_focused_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.focus {
            Pane::Composer => self.handle_composer_key(key)?,
            Pane::Groups => self.handle_groups_key(key)?,
            Pane::Tasks => self.handle_tasks_key(key)?,
        }
        Ok(())
    }

    fn handle_composer_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => self.begin_capture_edit(),
            _ => {}
        }
        Ok(())
    }

    fn handle_groups_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                self.active_group = self.selected_group;
                self.selected_task = 0;
                self.animations.pulse_group();
                self.set_status(format!(
                    "Group: {}",
                    self.project.groups[self.active_group].name
                ));
            }
            KeyCode::Char('n') => {
                self.begin_new_group();
            }
            KeyCode::Char('r') => {
                self.begin_rename_group();
            }
            KeyCode::Char('x') => {
                self.begin_delete_group();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_tasks_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => self.toggle_task_status()?,
            KeyCode::Char('c') => self.copy_selected_task()?,
            KeyCode::Char('e') => self.begin_task_edit()?,
            KeyCode::Char('x') => self.close_task()?,
            KeyCode::Char('o') | KeyCode::Char('r') => self.reopen_task()?,
            _ => {}
        }
        Ok(())
    }

    fn handle_capture_edit_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => self.submit_task()?,
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.composer.clear();
                self.composer_cursor = 0;
                self.clear_status("Cancelled capture");
            }
            KeyCode::Backspace => self.remove_composer_left(),
            KeyCode::Delete => self.remove_composer_right(),
            KeyCode::Left => {
                self.composer_cursor = prev_boundary(&self.composer, self.composer_cursor);
                self.animations.note_cursor_activity();
            }
            KeyCode::Right => {
                self.composer_cursor = next_boundary(&self.composer, self.composer_cursor);
                self.animations.note_cursor_activity();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_composer_char(ch);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_task_edit_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => self.commit_task_edit()?,
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.composer.clear();
                self.composer_cursor = 0;
                self.clear_status("Cancelled task edit");
            }
            KeyCode::Backspace => self.remove_composer_left(),
            KeyCode::Delete => self.remove_composer_right(),
            KeyCode::Left => {
                self.composer_cursor = prev_boundary(&self.composer, self.composer_cursor);
                self.animations.note_cursor_activity();
            }
            KeyCode::Right => {
                self.composer_cursor = next_boundary(&self.composer, self.composer_cursor);
                self.animations.note_cursor_activity();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_composer_char(ch);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_group_creation_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.group_input.clear();
                self.group_input_cursor = 0;
                self.clear_status("Cancelled new group");
            }
            KeyCode::Enter => self.commit_new_group()?,
            KeyCode::Backspace => {
                self.remove_group_input_left();
            }
            KeyCode::Delete => {
                self.remove_group_input_right();
            }
            KeyCode::Left => {
                self.group_input_cursor = prev_boundary(&self.group_input, self.group_input_cursor);
                self.animations.note_cursor_activity();
            }
            KeyCode::Right => {
                self.group_input_cursor = next_boundary(&self.group_input, self.group_input_cursor);
                self.animations.note_cursor_activity();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_group_input_char(ch);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_group_rename_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.group_input.clear();
                self.group_input_cursor = 0;
                self.clear_status("Cancelled rename");
            }
            KeyCode::Enter => self.commit_group_rename()?,
            KeyCode::Backspace => self.remove_group_input_left(),
            KeyCode::Delete => self.remove_group_input_right(),
            KeyCode::Left => {
                self.group_input_cursor = prev_boundary(&self.group_input, self.group_input_cursor);
                self.animations.note_cursor_activity();
            }
            KeyCode::Right => {
                self.group_input_cursor = next_boundary(&self.group_input, self.group_input_cursor);
                self.animations.note_cursor_activity();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_group_input_char(ch);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_clear_closed_confirmation_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.clear_closed_tasks()?,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_status("Cancelled clear closed tasks");
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_delete_group_confirmation_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.delete_group()?,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_status("Cancelled delete group");
            }
            _ => {}
        }
        Ok(())
    }

    fn submit_task(&mut self) -> Result<()> {
        self.take_undo_snapshot();
        let text = self.composer.trim();
        if text.is_empty() {
            self.invalid("Task cannot be empty");
            return Ok(());
        }

        self.project.add_task(self.active_group, text.to_string());
        self.composer.clear();
        self.composer_cursor = 0;
        self.mode = Mode::Normal;
        self.animations.note_cursor_activity();
        self.selected_task = 0;
        self.recent_task_index = Some(0);
        self.recent_task_motion = Some(TaskMotion::Added);
        self.animations.pulse_task();
        self.set_status("Task captured");
        self.persist()
    }

    fn begin_capture_edit(&mut self) {
        self.mode = Mode::EditingCapture;
        self.composer.clear();
        self.composer_cursor = 0;
        self.animations.note_cursor_activity();
        self.set_status("Typing capture");
        self.animations.pulse_focus();
    }

    fn begin_task_edit(&mut self) -> Result<()> {
        let Some(group) = self.project.groups.get(self.active_group) else {
            self.invalid("No active group");
            return Ok(());
        };
        let Some(task) = group.tasks.get(self.selected_task) else {
            self.invalid("No task selected");
            return Ok(());
        };

        self.mode = Mode::EditingTask;
        self.focus = Pane::Composer;
        self.composer = task.text.clone();
        self.composer_cursor = self.composer.len();
        self.animations.note_cursor_activity();
        self.set_status("Editing task");
        self.animations.pulse_focus();
        Ok(())
    }

    fn commit_new_group(&mut self) -> Result<()> {
        let name = self.group_input.trim();
        if name.is_empty() {
            self.invalid("Group name cannot be empty");
            return Ok(());
        }

        let index = self.project.add_group(name.to_string());
        self.mode = Mode::Normal;
        self.group_input.clear();
        self.group_input_cursor = 0;
        self.active_group = index;
        self.selected_group = index;
        self.selected_task = 0;
        self.animations.pulse_group();
        self.set_status("Group created");
        self.animations.pulse_task();
        self.persist()
    }

    fn commit_task_edit(&mut self) -> Result<()> {
        self.take_undo_snapshot();
        let text = self.composer.trim();
        if text.is_empty() {
            self.invalid("Task cannot be empty");
            return Ok(());
        }

        let Some(group) = self.project.groups.get_mut(self.active_group) else {
            self.invalid("No active group");
            return Ok(());
        };
        let Some(task) = group.tasks.get_mut(self.selected_task) else {
            self.invalid("No task selected");
            return Ok(());
        };

        task.text = text.to_string();
        self.mode = Mode::Normal;
        self.composer.clear();
        self.composer_cursor = 0;
        self.recent_task_index = Some(self.selected_task);
        self.recent_task_motion = None;
        self.set_status("Task updated");
        self.animations.pulse_task();
        self.persist()
    }

    fn commit_group_rename(&mut self) -> Result<()> {
        let name = self.group_input.trim();
        if name.is_empty() {
            self.invalid("Group name cannot be empty");
            return Ok(());
        }

        if let Some(group) = self.project.groups.get_mut(self.selected_group) {
            group.name = name.to_string();
        }
        self.mode = Mode::Normal;
        self.group_input.clear();
        self.group_input_cursor = 0;
        self.animations.pulse_group();
        self.set_status("Group renamed");
        self.persist()
    }

    fn clear_closed_tasks(&mut self) -> Result<()> {
        self.take_undo_snapshot();
        let Some(group) = self.project.groups.get_mut(self.active_group) else {
            self.invalid("No active group");
            return Ok(());
        };

        let before = group.tasks.len();
        group.tasks.retain(|task| task.status != TaskStatus::Closed);
        let removed = before.saturating_sub(group.tasks.len());
        self.mode = Mode::Normal;
        self.selected_task = self.selected_task.min(group.tasks.len().saturating_sub(1));

        if removed == 0 {
            self.invalid("No closed tasks to clear");
            return Ok(());
        }

        self.recent_task_index = None;
        self.recent_task_motion = None;
        self.set_status(format!("Cleared {removed} closed task(s)"));
        self.animations.pulse_task();
        self.persist()
    }

    fn delete_group(&mut self) -> Result<()> {
        self.take_undo_snapshot();
        if self.project.groups.len() <= 1 {
            self.invalid("Cannot delete the last group");
            self.mode = Mode::Normal;
            return Ok(());
        }

        let index = self.selected_group.min(self.project.groups.len().saturating_sub(1));
        self.project.groups.remove(index);
        let fallback = index.min(self.project.groups.len().saturating_sub(1));
        self.selected_group = fallback;
        self.active_group = fallback.min(self.project.groups.len().saturating_sub(1));
        self.selected_task = 0;
        self.mode = Mode::Normal;
        self.set_status("Group deleted");
        self.animations.pulse_group();
        self.persist()
    }

    fn close_task(&mut self) -> Result<()> {
        self.take_undo_snapshot();
        let Some(group) = self.project.groups.get_mut(self.active_group) else {
            self.invalid("No active group");
            return Ok(());
        };

        let Some(task) = group.tasks.get_mut(self.selected_task) else {
            self.invalid("No task selected");
            return Ok(());
        };

        if task.status == TaskStatus::Closed {
            self.invalid("Task already closed");
            return Ok(());
        }

        task.close();
        self.recent_task_index = Some(self.selected_task);
        self.recent_task_motion = Some(TaskMotion::Closed);
        self.set_status("Task closed");
        self.animations.pulse_task();
        self.persist()
    }

    fn reopen_task(&mut self) -> Result<()> {
        self.take_undo_snapshot();
        let Some(group) = self.project.groups.get_mut(self.active_group) else {
            self.invalid("No active group");
            return Ok(());
        };

        let Some(task) = group.tasks.get_mut(self.selected_task) else {
            self.invalid("No task selected");
            return Ok(());
        };

        if task.status == TaskStatus::Open {
            self.invalid("Task already open");
            return Ok(());
        }

        task.reopen();
        self.recent_task_index = Some(self.selected_task);
        self.recent_task_motion = Some(TaskMotion::Reopened);
        self.set_status("Task reopened");
        self.animations.pulse_task();
        self.persist()
    }

    fn navigate_up(&mut self) {
        match self.focus {
            Pane::Groups => {}
            Pane::Composer => {
                self.focus = Pane::Groups;
                self.animations.pulse_focus();
            }
            Pane::Tasks => {
                if self.selected_task > 0 {
                    self.selected_task -= 1;
                } else {
                    self.focus = Pane::Composer;
                    self.animations.pulse_focus();
                }
            }
        }
    }

    fn navigate_down(&mut self) {
        match self.focus {
            Pane::Groups => {
                self.focus = Pane::Composer;
                self.animations.pulse_focus();
            }
            Pane::Composer => {
                self.focus = Pane::Tasks;
                self.animations.pulse_focus();
            }
            Pane::Tasks => self.navigate_down_tasks(),
        }
    }

    fn navigate_down_tasks(&mut self) {
        let count = self.current_group_task_count();
        if count == 0 {
            return;
        }
        self.selected_task = (self.selected_task + 1).min(count.saturating_sub(1));
    }

    fn navigate_up_tasks(&mut self) {
        self.selected_task = self.selected_task.saturating_sub(1);
    }

    fn navigate_left(&mut self) {
        match self.focus {
            Pane::Groups => {
                self.selected_group = self.selected_group.saturating_sub(1);
            }
            Pane::Composer => {
                self.composer_cursor = prev_boundary(&self.composer, self.composer_cursor);
                self.animations.note_cursor_activity();
            }
            Pane::Tasks => {
                self.focus = Pane::Composer;
                self.animations.note_cursor_activity();
                self.animations.pulse_focus();
            }
        }
    }

    fn navigate_right(&mut self) {
        match self.focus {
            Pane::Groups => {
                let last = self.project.groups.len().saturating_sub(1);
                self.selected_group = (self.selected_group + 1).min(last);
            }
            Pane::Composer => {
                self.composer_cursor = next_boundary(&self.composer, self.composer_cursor);
                self.animations.note_cursor_activity();
            }
            Pane::Tasks => {}
        }
    }

    fn toggle_task_status(&mut self) -> Result<()> {
        let Some(group) = self.project.groups.get(self.active_group) else {
            self.invalid("No active group");
            return Ok(());
        };
        let Some(task) = group.tasks.get(self.selected_task) else {
            self.invalid("No task selected");
            return Ok(());
        };
        if task.status == TaskStatus::Open {
            self.close_task()
        } else {
            self.reopen_task()
        }
    }

    fn persist(&mut self) -> Result<()> {
        storage::save_project(&self.project_path, &self.project)?;
        self.animations.pulse_status();
        Ok(())
    }

    fn invalid(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.animations.shake();
        self.animations.pulse_status();
    }

    fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.animations.pulse_status();
    }

    fn clear_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.animations.pulse_status();
    }

    pub fn tick_rate() -> Duration {
        Duration::from_millis(33)
    }

    fn active_group_closed_task_count(&self) -> usize {
        self.project
            .groups
            .get(self.active_group)
            .map(|group| {
                group
                    .tasks
                    .iter()
                    .filter(|task| task.status == TaskStatus::Closed)
                    .count()
            })
            .unwrap_or(0)
    }

    fn set_focus(&mut self, pane: Pane) {
        self.focus = pane;
        if pane == Pane::Composer {
            self.animations.note_cursor_activity();
        }
        self.animations.pulse_focus();
    }

    fn begin_new_group(&mut self) {
        self.mode = Mode::CreatingGroup;
        self.group_input.clear();
        self.group_input_cursor = 0;
        self.animations.note_cursor_activity();
        self.set_status("Name the new group");
        self.animations.pulse_focus();
    }

    fn begin_rename_group(&mut self) {
        let current_name = self
            .project
            .groups
            .get(self.selected_group)
            .map(|group| group.name.clone())
            .unwrap_or_default();
        self.mode = Mode::RenamingGroup;
        self.group_input = current_name;
        self.group_input_cursor = self.group_input.len();
        self.animations.note_cursor_activity();
        self.set_status("Rename the group");
        self.animations.pulse_focus();
    }

    fn begin_clear_closed(&mut self) {
        if self.active_group_closed_task_count() == 0 {
            self.invalid("No closed tasks to clear");
        } else {
            self.mode = Mode::ConfirmClearClosed;
            self.set_status("Clear closed tasks? y/n");
            self.animations.pulse_focus();
        }
    }

    fn begin_delete_group(&mut self) {
        if self.project.groups.len() <= 1 {
            self.invalid("Cannot delete the last group");
        } else {
            self.mode = Mode::ConfirmDeleteGroup;
            self.set_status("Delete group? y/n");
            self.animations.pulse_focus();
        }
    }

    fn copy_selected_task(&mut self) -> Result<()> {
        let Some(group) = self.project.groups.get(self.active_group) else {
            self.invalid("No active group");
            return Ok(());
        };
        let Some(task) = group.tasks.get(self.selected_task) else {
            self.invalid("No task selected");
            return Ok(());
        };

        let mut clipboard = match Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(_) => {
                self.invalid("Clipboard unavailable");
                return Ok(());
            }
        };

        match clipboard.set_text(task.text.clone()) {
            Ok(()) => {
                self.set_status("Task copied");
                Ok(())
            }
            Err(_) => {
                self.invalid("Failed to copy task");
                Ok(())
            }
        }
    }

    fn on_paste(&mut self, text: String) -> Result<()> {
        match self.mode {
            Mode::EditingCapture | Mode::EditingTask => {
                for ch in text.chars() {
                    match ch {
                        '\r' => {}
                        '\n' => self.insert_composer_char(' '),
                        ch if ch.is_control() => {}
                        ch => self.insert_composer_char(ch),
                    }
                }
            }
            Mode::CreatingGroup | Mode::RenamingGroup => {
                for ch in text.chars() {
                    if !ch.is_control() {
                        self.insert_group_input_char(ch);
                    }
                }
            }
            Mode::Normal | Mode::ConfirmClearClosed | Mode::ConfirmDeleteGroup => {}
        }
        Ok(())
    }

    fn insert_composer_char(&mut self, ch: char) {
        self.composer.insert(self.composer_cursor, ch);
        self.composer_cursor += ch.len_utf8();
        self.animations.note_cursor_activity();
    }

    fn remove_composer_left(&mut self) {
        if self.composer_cursor == 0 {
            return;
        }
        let remove_at = prev_boundary(&self.composer, self.composer_cursor);
        self.composer.drain(remove_at..self.composer_cursor);
        self.composer_cursor = remove_at;
        self.animations.note_cursor_activity();
    }

    fn remove_composer_right(&mut self) {
        if self.composer_cursor >= self.composer.len() {
            return;
        }
        let next = next_boundary(&self.composer, self.composer_cursor);
        self.composer.drain(self.composer_cursor..next);
        self.animations.note_cursor_activity();
    }

    fn insert_group_input_char(&mut self, ch: char) {
        self.group_input.insert(self.group_input_cursor, ch);
        self.group_input_cursor += ch.len_utf8();
        self.animations.note_cursor_activity();
    }

    fn remove_group_input_left(&mut self) {
        if self.group_input_cursor == 0 {
            return;
        }
        let remove_at = prev_boundary(&self.group_input, self.group_input_cursor);
        self.group_input.drain(remove_at..self.group_input_cursor);
        self.group_input_cursor = remove_at;
        self.animations.note_cursor_activity();
    }

    fn remove_group_input_right(&mut self) {
        if self.group_input_cursor >= self.group_input.len() {
            return;
        }
        let next = next_boundary(&self.group_input, self.group_input_cursor);
        self.group_input.drain(self.group_input_cursor..next);
        self.animations.note_cursor_activity();
    }
}

fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

fn prev_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_boundary(text: &str, cursor: usize) -> usize {
    if cursor >= text.len() {
        return text.len();
    }
    text[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(text.len())
}
