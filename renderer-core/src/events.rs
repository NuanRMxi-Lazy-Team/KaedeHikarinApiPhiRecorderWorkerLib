use std::sync::Arc;

pub type JobId = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum JobEventKind {
    Started,
    Loading,
    Mixing,
    MixingSfx {
        completed: u64,
        total: u64,
    },
    Rendering {
        completed: u64,
        total: u64,
        fps: f64,
        estimated_seconds: f64,
    },
    Paused,
    Resumed,
    Done {
        duration_seconds: f64,
    },
    Canceled,
    Failed {
        message: Arc<str>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobEvent {
    pub job_id: JobId,
    pub kind: JobEventKind,
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: JobEvent);
}

#[derive(Debug, Default)]
pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: JobEvent) {}
}
