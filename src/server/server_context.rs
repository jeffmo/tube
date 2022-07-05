use std::collections::VecDeque;
use std::task;

use super::server_error::ServerError;
use super::server_event::ServerEvent;

pub(in crate::server) struct ServerContext {
    pub(in crate::server) is_complete: bool,
    pub(in crate::server) pending_events: VecDeque<Result<ServerEvent, ServerError>>,
    pub(in crate::server) waker: Option<task::Waker>,
}
