use std::collections::VecDeque;
use std::task;

use super::tube_event;

#[derive(Debug)]
pub(in crate) struct TubeManager {
    pub(in crate) state_machine: tube_event::StateMachine,
    pub(in crate) pending_events: VecDeque<tube_event::TubeEvent>,
    pub(in crate) terminated: bool,
    pub(in crate) waker: Option<task::Waker>,
}
impl TubeManager {
    pub(in crate) fn new() -> Self {
        TubeManager {
            state_machine: tube_event::StateMachine::new(),
            pending_events: VecDeque::new(),
            terminated: false,
            waker: None,
        }
    }
}
