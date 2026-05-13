use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::mpsc;

#[derive(Debug)]
pub enum EngineCommand {
    Cancel,
}

#[derive(Debug, Clone, Default)]
pub struct TurnCancel {
    state: Arc<StdMutex<TurnCancelState>>,
}

#[derive(Debug, Default)]
struct TurnCancelState {
    senders: Vec<mpsc::UnboundedSender<EngineCommand>>,
    cancelled: bool,
}

impl TurnCancel {
    pub fn new(root: mpsc::UnboundedSender<EngineCommand>) -> Self {
        let group = Self::default();
        group.register(root);
        group
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn register(&self, sender: mpsc::UnboundedSender<EngineCommand>) {
        if let Ok(mut state) = self.state.lock() {
            if state.cancelled {
                let _ = sender.send(EngineCommand::Cancel);
            }
            state.senders.push(sender);
        }
    }

    pub fn cancel_all(&self) -> bool {
        let senders = self
            .state
            .lock()
            .map(|mut state| {
                state.cancelled = true;
                state.senders.clone()
            })
            .unwrap_or_default();
        let mut sent = false;
        for sender in senders {
            sent |= sender.send(EngineCommand::Cancel).is_ok();
        }
        sent
    }
}
