use std::collections::HashMap;
use std::collections::VecDeque;
use std::task;

use crate::common::frame;
use crate::common::InvertedFutureResolver;
use crate::common::UniqueId;
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
    /**
     * When we send an Abort to our peer, this holds a UniqueId object alive
     * until the peer acknowledges the Abort (at which point it is removed from
     * here, ultimately dropped, and the TubeId can then be re-used).
     */
    pub abort_pending_id_reservation: Option<UniqueId>,
    pub pending_events: VecDeque<tube_event::TubeEvent>,
    pub sendacks: HashMap<u16, InvertedFutureResolver<()>>,
    pub completion_state: TubeCompletionState,
    pub waker: Option<task::Waker>,
}
impl TubeManager {
    pub fn new() -> Self {
        TubeManager {
            abort_pending_id_reservation: None,
            completion_state: TubeCompletionState::Open,
            pending_events: VecDeque::new(),
            sendacks: HashMap::new(),
            waker: None,
        }
    }
}
