use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
};

use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub enum EngineCommand {
    Cancel,
}

#[derive(Debug)]
pub enum QuestionReply {
    Answer {
        answers: Vec<Vec<String>>,
        stop: bool,
    },
    Rejected,
}

#[derive(Debug, Clone, Default)]
pub struct TurnCancel {
    state: Arc<StdMutex<TurnCancelState>>,
}

#[derive(Debug, Default)]
struct TurnCancelState {
    senders: Vec<mpsc::UnboundedSender<EngineCommand>>,
    questions: HashMap<String, oneshot::Sender<QuestionReply>>,
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
        let (senders, questions) = self
            .state
            .lock()
            .map(|mut state| {
                state.cancelled = true;
                let questions = state
                    .questions
                    .drain()
                    .map(|(_, sender)| sender)
                    .collect::<Vec<_>>();
                (state.senders.clone(), questions)
            })
            .unwrap_or_default();
        let mut sent = false;
        for question in questions {
            sent |= question.send(QuestionReply::Rejected).is_ok();
        }
        for sender in senders {
            sent |= sender.send(EngineCommand::Cancel).is_ok();
        }
        sent
    }

    pub fn register_question(
        &self,
        tool_call_id: impl Into<String>,
    ) -> Result<oneshot::Receiver<QuestionReply>, ()> {
        let (sender, receiver) = oneshot::channel();
        let previous = match self.state.lock() {
            Ok(mut state) => {
                if state.cancelled {
                    return Err(());
                }
                state.questions.insert(tool_call_id.into(), sender)
            }
            Err(_) => return Err(()),
        };
        if let Some(previous) = previous {
            let _ = previous.send(QuestionReply::Rejected);
        }
        Ok(receiver)
    }

    pub fn unregister_question(&self, tool_call_id: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.questions.remove(tool_call_id);
        }
    }

    pub fn answer_question(
        &self,
        tool_call_id: &str,
        answers: Vec<Vec<String>>,
        stop: bool,
    ) -> bool {
        let sender = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.questions.remove(tool_call_id));
        sender
            .map(|sender| sender.send(QuestionReply::Answer { answers, stop }).is_ok())
            .unwrap_or(false)
    }

    pub fn reject_question(&self, tool_call_id: &str) -> bool {
        let sender = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.questions.remove(tool_call_id));
        sender
            .map(|sender| sender.send(QuestionReply::Rejected).is_ok())
            .unwrap_or(false)
    }
}
