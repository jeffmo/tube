use std::collections::HashMap;
use std::collections::VecDeque;
use std::task;

use crate::tube::event;

#[derive(Debug)]
pub(in crate) struct TubeManager {
    pub(in crate) state_machine: event::StateMachine,
    pub(in crate) pending_events: VecDeque<event::TubeEvent>,
    pub(in crate) terminated: bool,
    pub(in crate) waker: Option<task::Waker>,
}
impl TubeManager {
    pub(in crate) fn new() -> Self {
        TubeManager {
            state_machine: event::StateMachine::new(),
            pending_events: VecDeque::new(),
            terminated: false,
            waker: None,
        }
    }
}
