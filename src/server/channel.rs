use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use crate::common::tube::Tube;

#[derive(Debug)]
pub enum ChannelEvent {
    NewTube(Tube),
}

#[derive(Debug)]
pub(in crate::server) struct ChannelContext {
    pub(in crate::server) pending_events: VecDeque<ChannelEvent>,
    pub(in crate::server) waker: Option<std::task::Waker>,
}
impl ChannelContext {
    pub fn new() -> Self {
        ChannelContext {
            pending_events: VecDeque::new(),
            waker: None,
        }
    }
}

#[derive(Debug)]
pub struct Channel {
    ctx: Arc<Mutex<ChannelContext>>,
}
impl Channel {
    pub(in crate::server) fn new(
        ctx: Arc<Mutex<ChannelContext>>,
    ) -> Self {
        Channel {
            ctx,
        }
    }
}
impl futures::stream::Stream for Channel {
    type Item = ChannelEvent;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        let mut ctx = self.ctx.lock().unwrap();
        ctx.waker = Some(cx.waker().clone());

        match ctx.pending_events.pop_front() {
            Some(channel_event) => futures::task::Poll::Ready(Some(channel_event)),
            None => futures::task::Poll::Pending,
        }
    }
}
