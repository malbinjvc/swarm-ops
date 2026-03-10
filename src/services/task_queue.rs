use std::collections::{HashSet, VecDeque};
use crate::models::{SwarmTask, TaskType};

/// A task queue that supports ordering and deduplication by task data + type.
pub struct TaskQueue {
    queue: VecDeque<SwarmTask>,
    seen: HashSet<(String, String)>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            seen: HashSet::new(),
        }
    }

    /// Enqueue a task. Returns false if a duplicate (same data + task_type) is
    /// already in the queue.
    pub fn enqueue(&mut self, task: SwarmTask) -> bool {
        let key = Self::dedup_key(&task.data, &task.task_type);
        if self.seen.contains(&key) {
            return false;
        }
        self.seen.insert(key);
        self.queue.push_back(task);
        true
    }

    /// Dequeue the next task (FIFO).
    pub fn dequeue(&mut self) -> Option<SwarmTask> {
        if let Some(task) = self.queue.pop_front() {
            let key = Self::dedup_key(&task.data, &task.task_type);
            self.seen.remove(&key);
            Some(task)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Peek at the front without removing.
    pub fn peek(&self) -> Option<&SwarmTask> {
        self.queue.front()
    }

    fn dedup_key(data: &str, task_type: &TaskType) -> (String, String) {
        (data.to_string(), format!("{:?}", task_type))
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TaskType;

    #[test]
    fn test_enqueue_dequeue() {
        let mut q = TaskQueue::new();
        let t = SwarmTask::new(TaskType::LogAnalysis, "log data".into(), vec![]);
        assert!(q.enqueue(t));
        assert_eq!(q.len(), 1);
        let out = q.dequeue().unwrap();
        assert_eq!(out.data, "log data");
        assert!(q.is_empty());
    }

    #[test]
    fn test_dedup_same_data_and_type() {
        let mut q = TaskQueue::new();
        let t1 = SwarmTask::new(TaskType::LogAnalysis, "same".into(), vec![]);
        let t2 = SwarmTask::new(TaskType::LogAnalysis, "same".into(), vec![]);
        assert!(q.enqueue(t1));
        assert!(!q.enqueue(t2));
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn test_dedup_different_type_allowed() {
        let mut q = TaskQueue::new();
        let t1 = SwarmTask::new(TaskType::LogAnalysis, "data".into(), vec![]);
        let t2 = SwarmTask::new(TaskType::HealthCheck, "data".into(), vec![]);
        assert!(q.enqueue(t1));
        assert!(q.enqueue(t2));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn test_fifo_ordering() {
        let mut q = TaskQueue::new();
        let t1 = SwarmTask::new(TaskType::LogAnalysis, "first".into(), vec![]);
        let t2 = SwarmTask::new(TaskType::HealthCheck, "second".into(), vec![]);
        let t3 = SwarmTask::new(TaskType::Incident, "third".into(), vec![]);
        q.enqueue(t1);
        q.enqueue(t2);
        q.enqueue(t3);
        assert_eq!(q.dequeue().unwrap().data, "first");
        assert_eq!(q.dequeue().unwrap().data, "second");
        assert_eq!(q.dequeue().unwrap().data, "third");
    }

    #[test]
    fn test_peek_does_not_remove() {
        let mut q = TaskQueue::new();
        let t = SwarmTask::new(TaskType::Incident, "peek me".into(), vec![]);
        q.enqueue(t);
        assert_eq!(q.peek().unwrap().data, "peek me");
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn test_dequeue_empty() {
        let mut q = TaskQueue::new();
        assert!(q.dequeue().is_none());
    }

    #[test]
    fn test_re_enqueue_after_dequeue() {
        let mut q = TaskQueue::new();
        let t = SwarmTask::new(TaskType::LogAnalysis, "dup".into(), vec![]);
        q.enqueue(t);
        q.dequeue();
        let t2 = SwarmTask::new(TaskType::LogAnalysis, "dup".into(), vec![]);
        assert!(q.enqueue(t2)); // should succeed after dequeue cleared the key
    }
}
