use crate::model::TaskId;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum NoticeKind {
    Question,
    Failed,
    Outward,
    Done,
    Limit,
}

impl NoticeKind {
    /// Question, Failed and Outward make a sound; Done and Limit are silent.
    pub fn is_loud(&self) -> bool {
        matches!(self, NoticeKind::Question | NoticeKind::Failed | NoticeKind::Outward)
    }
}

#[derive(Clone, Debug)]
pub struct Notice {
    pub kind: NoticeKind,
    pub task_id: TaskId,
    pub project: String,
    pub title: String,
    pub body: String,
    pub options: Vec<String>,
}

pub trait Notifier: Send + Sync {
    fn send(&self, n: &Notice);
}

pub struct Fanout(pub Vec<Arc<dyn Notifier>>);
impl Notifier for Fanout {
    fn send(&self, n: &Notice) {
        for x in &self.0 {
            x.send(n);
        }
    }
}

pub struct NullNotifier;
impl Notifier for NullNotifier {
    fn send(&self, _n: &Notice) {}
}

#[derive(Default)]
pub struct RecordingNotifier(pub Mutex<Vec<Notice>>);
impl Notifier for RecordingNotifier {
    fn send(&self, n: &Notice) {
        self.0.lock().unwrap().push(n.clone());
    }
}
