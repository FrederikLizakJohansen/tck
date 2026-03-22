use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

impl Task {
    pub fn new(id: u64, text: String) -> Self {
        Self {
            id,
            text,
            status: TaskStatus::Open,
            created_at: Utc::now(),
            closed_at: None,
        }
    }

    pub fn close(&mut self) {
        self.status = TaskStatus::Closed;
        self.closed_at = Some(Utc::now());
    }

    pub fn reopen(&mut self) {
        self.status = TaskStatus::Open;
        self.closed_at = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub tasks: Vec<Task>,
}

impl Group {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            tasks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub groups: Vec<Group>,
    pub next_group_id: u64,
    pub next_task_id: u64,
    #[serde(default)]
    pub theme_index: usize,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        let mut project = Self {
            name: name.into(),
            groups: Vec::new(),
            next_group_id: 1,
            next_task_id: 1,
            theme_index: 0,
        };
        project.add_group("Inbox");
        project
    }

    pub fn add_group(&mut self, name: impl Into<String>) -> usize {
        let index = self.groups.len();
        self.groups.push(Group::new(self.next_group_id, name));
        self.next_group_id += 1;
        index
    }

    pub fn add_task(&mut self, group_index: usize, text: String) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        if let Some(group) = self.groups.get_mut(group_index) {
            group.tasks.insert(0, Task::new(id, text));
        }
        id
    }
}
