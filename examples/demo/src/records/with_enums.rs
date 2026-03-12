use boltffi::*;

use crate::enums::repr_int::Priority;

#[data]
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub title: String,
    pub priority: Priority,
    pub completed: bool,
}

#[export]
pub fn echo_task(task: Task) -> Task {
    task
}

#[export]
pub fn make_task(title: String, priority: Priority) -> Task {
    Task {
        title,
        priority,
        completed: false,
    }
}

#[export]
pub fn is_urgent(task: Task) -> bool {
    matches!(task.priority, Priority::High | Priority::Critical)
}

#[data]
#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    pub message: String,
    pub priority: Priority,
    pub read: bool,
}

#[export]
pub fn echo_notification(notification: Notification) -> Notification {
    notification
}
