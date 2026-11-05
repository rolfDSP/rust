//! In-memory task storage. All data is lost when the service stops.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::models::Task;

#[derive(Default)]
struct Inner {
    last_id: u64,
    // A BTreeMap keeps the tasks sorted by id, so listings have a stable order.
    tasks: BTreeMap<u64, Task>,
}

/// A handle to the shared storage. Cloning it is cheap, and all clones see the same
/// data, which is how axum hands it to concurrently running handlers.
///
/// The std `RwLock` is the right choice here because it is never held across an
/// `.await`. A poisoned lock means another thread panicked in the middle of a change,
/// so these methods panic as well instead of working with possibly broken data.
#[derive(Clone, Default)]
pub struct TaskStore {
    inner: Arc<RwLock<Inner>>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> Vec<Task> {
        let inner = self.inner.read().expect("task store lock poisoned");
        inner.tasks.values().cloned().collect()
    }

    pub fn get(&self, id: u64) -> Option<Task> {
        let inner = self.inner.read().expect("task store lock poisoned");
        inner.tasks.get(&id).cloned()
    }

    pub fn create(&self, title: String) -> Task {
        let mut inner = self.inner.write().expect("task store lock poisoned");
        inner.last_id += 1;
        let task = Task {
            id: inner.last_id,
            title,
            completed: false,
        };
        inner.tasks.insert(task.id, task.clone());
        task
    }

    /// Changes the given fields and returns the new task, or `None` if the id is unknown.
    pub fn update(&self, id: u64, title: Option<String>, completed: Option<bool>) -> Option<Task> {
        let mut inner = self.inner.write().expect("task store lock poisoned");
        let task = inner.tasks.get_mut(&id)?;
        if let Some(title) = title {
            task.title = title;
        }
        if let Some(completed) = completed {
            task.completed = completed;
        }
        Some(task.clone())
    }

    /// Returns whether a task with this id existed.
    pub fn delete(&self, id: u64) -> bool {
        let mut inner = self.inner.write().expect("task store lock poisoned");
        inner.tasks.remove(&id).is_some()
    }
}
