use std::collections::HashMap;
use std::collections::VecDeque;
use std::task;

use crate::common::frame;
use crate::common::InvertedFutureResolver;
use super::tube_event;

#[derive(Clone,Debug,PartialEq)]
pub(in crate) enum TubeCompletionState {
    Open,
    ClientHasFinishedSending,
    ServerHasFinishedSending,
    Closed,
    AbortedFromLocal(frame::AbortReason),
    AbortedFromRemote(frame::AbortReason),
}

#[derive(Debug)]
pub(in crate) struct TubeManager {
    pub(in crate) pending_events: VecDeque<tube_event::TubeEvent>,
    pub(in crate) sendacks: HashMap<u16, InvertedFutureResolver<()>>,
    pub(in crate) terminated: bool,
    pub(in crate) completion_state: TubeCompletionState,
    pub(in crate) waker: Option<task::Waker>,
}
impl TubeManager {
    pub(in crate) fn new() -> Self {
        TubeManager {
            sendacks: HashMap::new(),
            pending_events: VecDeque::new(),
            terminated: false,
            completion_state: TubeCompletionState::Open,
            waker: None,
        }
    }
}
