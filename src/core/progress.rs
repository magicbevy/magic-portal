use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::error::{PortalError, PortalResult};

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Step {
        message: String,
    },
    Download {
        label: String,
        received: u64,
        total: Option<u64>,
    },
    Log {
        line: String,
    },
}

impl ProgressEvent {
    pub fn step(message: impl Into<String>) -> Self {
        Self::Step {
            message: message.into(),
        }
    }

    pub fn log(line: impl Into<String>) -> Self {
        Self::Log { line: line.into() }
    }
}

pub trait Reporter: Send {
    fn report(&mut self, event: ProgressEvent);
}

impl<F> Reporter for F
where
    F: FnMut(ProgressEvent) + Send,
{
    fn report(&mut self, event: ProgressEvent) {
        self(event);
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancelFlag {
    cancelled: Arc<AtomicBool>,
}

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> PortalResult<()> {
        if self.is_cancelled() {
            Err(PortalError::Cancelled)
        } else {
            Ok(())
        }
    }
}
