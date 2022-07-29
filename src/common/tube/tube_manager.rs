use std::collections::HashMap;
use std::collections::VecDeque;
use std::task;

use crate::common::frame;
use crate::common::InvertedFutureResolver;
use super::tube_event;

#[derive(Clone,Debug,PartialEq)]
pub enum TubeCompletionState {
    Open,
    ClientHasFinishedSending,
    ServerHasFinishedSending,
    Closed,
    AbortedFromLocal(frame::AbortReason),
    AbortedFromRemote(frame::AbortReason),
}

#[derive(Debug)]
pub struct TubeManager {
    pub pending_events: VecDeque<tube_event::TubeEvent>,
    pub sendacks: HashMap<u16, InvertedFutureResolver<()>>,
    pub terminated: bool,
    pub completion_state: TubeCompletionState,
    pub waker: Option<task::Waker>,
}
impl TubeManager {
    pub fn new() -> Self {
        TubeManager {
            sendacks: HashMap::new(),
            pending_events: VecDeque::new(),
            terminated: false,
            completion_state: TubeCompletionState::Open,
            waker: None,
        }
    }
}
