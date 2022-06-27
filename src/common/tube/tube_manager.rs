use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::task;

use super::sendack_future::SendAckFutureContext;
use super::tube_event;

#[derive(Debug, PartialEq)]
pub(in crate) enum TubeCompletionState {
    Open,
    ClientHasFinishedSending,
    ServerHasFinishedSending,
    Closed,
}

#[derive(Debug)]
pub(in crate) struct TubeManager {
    pub(in crate) pending_events: VecDeque<tube_event::TubeEvent>,
    pub(in crate) sendacks: HashMap<u16, Arc<Mutex<SendAckFutureContext>>>,
    pub(in crate) state_machine: tube_event::StateMachine,
    pub(in crate) terminated: bool,
    pub(in crate) completion_state: TubeCompletionState,
    pub(in crate) waker: Option<task::Waker>,
}
impl TubeManager {
    pub(in crate) fn new() -> Self {
        TubeManager {
            sendacks: HashMap::new(),
            state_machine: tube_event::StateMachine::new(),
            pending_events: VecDeque::new(),
            terminated: false,
            completion_state: TubeCompletionState::Open,
            waker: None,
        }
    }
}
