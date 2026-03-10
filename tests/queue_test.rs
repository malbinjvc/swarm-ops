use swarm_ops::models::{SwarmTask, TaskType};
use swarm_ops::services::task_queue::TaskQueue;

#[test]
fn test_queue_enqueue_dequeue_basic() {
    let mut q = TaskQueue::new();
    let t = SwarmTask::new(TaskType::LogAnalysis, "data1".into(), vec![]);
    assert!(q.enqueue(t));
    assert_eq!(q.len(), 1);
    let out = q.dequeue().unwrap();
    assert_eq!(out.data, "data1");
    assert!(q.is_empty());
}

#[test]
fn test_queue_dedup_blocks_same_data_and_type() {
    let mut q = TaskQueue::new();
    let t1 = SwarmTask::new(TaskType::LogAnalysis, "same data".into(), vec![]);
    let t2 = SwarmTask::new(TaskType::LogAnalysis, "same data".into(), vec![]);
    assert!(q.enqueue(t1));
    assert!(!q.enqueue(t2));
    assert_eq!(q.len(), 1);
}

#[test]
fn test_queue_dedup_allows_different_types() {
    let mut q = TaskQueue::new();
    let t1 = SwarmTask::new(TaskType::LogAnalysis, "data".into(), vec![]);
    let t2 = SwarmTask::new(TaskType::HealthCheck, "data".into(), vec![]);
    assert!(q.enqueue(t1));
    assert!(q.enqueue(t2));
    assert_eq!(q.len(), 2);
}

#[test]
fn test_queue_fifo_order() {
    let mut q = TaskQueue::new();
    for i in 0..5 {
        let t = SwarmTask::new(TaskType::Incident, format!("item-{}", i), vec![]);
        q.enqueue(t);
    }
    for i in 0..5 {
        let out = q.dequeue().unwrap();
        assert_eq!(out.data, format!("item-{}", i));
    }
}

#[test]
fn test_queue_peek() {
    let mut q = TaskQueue::new();
    let t = SwarmTask::new(TaskType::Incident, "peek data".into(), vec![]);
    q.enqueue(t);
    assert_eq!(q.peek().unwrap().data, "peek data");
    assert_eq!(q.len(), 1); // peek doesn't remove
}

#[test]
fn test_queue_dequeue_empty() {
    let mut q = TaskQueue::new();
    assert!(q.dequeue().is_none());
}

#[test]
fn test_queue_re_enqueue_after_dequeue() {
    let mut q = TaskQueue::new();
    let t = SwarmTask::new(TaskType::LogAnalysis, "reinsert".into(), vec![]);
    q.enqueue(t);
    q.dequeue();
    let t2 = SwarmTask::new(TaskType::LogAnalysis, "reinsert".into(), vec![]);
    assert!(q.enqueue(t2)); // key was cleared on dequeue
}

#[test]
fn test_queue_default() {
    let q = TaskQueue::default();
    assert!(q.is_empty());
}
