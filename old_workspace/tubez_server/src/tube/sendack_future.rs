use std::future;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct SendAckFutureContext {
    pub ack_received: bool,
}

pub struct SendAckFuture {
    pub ctx: Arc<Mutex<SendAckFutureContext>>,
}
impl future::Future for SendAckFuture {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>, 
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let ctx = self.ctx.lock().unwrap();
        if ctx.ack_received {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    }
}
