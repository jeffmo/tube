use std::collections::VecDeque;
use std::task;

use crate::server_event::ServerEvent;

pub(in crate) struct ServerContext {
    pub(in crate) is_complete: bool,
    pub(in crate) pending_events: VecDeque<ServerEvent>,
    pub(in crate) waker: Option<task::Waker>,
}
