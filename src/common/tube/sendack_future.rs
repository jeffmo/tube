use std::future;
use std::sync::Arc;
use std::sync::Mutex;
use std::task;

#[derive(Debug)]
pub(in crate) struct SendAckFutureContext {
    pub ack_received: bool,
    pub waker: Option<task::Waker>,
}

pub(in crate) struct SendAckFuture {
    ctx: Arc<Mutex<SendAckFutureContext>>,
}
impl SendAckFuture {
    pub fn new(ctx: Arc<Mutex<SendAckFutureContext>>) -> Self {
        SendAckFuture {
            ctx,
        }
    }
}
impl future::Future for SendAckFuture {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>, 
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut ctx = self.ctx.lock().unwrap();
        ctx.waker = Some(cx.waker().clone());

        if ctx.ack_received {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    }
}
