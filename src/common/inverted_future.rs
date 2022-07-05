use std::future;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct InvertedFutureContext<T> {
    resolution: Option<T>,
    waker: Option<std::task::Waker>,
}

#[derive(Debug)]
pub struct InvertedFutureResolver<T> {
    ctx: Arc<Mutex<InvertedFutureContext<T>>>,
}
impl<T> InvertedFutureResolver<T> {
    pub fn resolve(&mut self, v: T) {
        let mut ctx = self.ctx.lock().unwrap();
        ctx.resolution = Some(v);
        if let Some(waker) = ctx.waker.take() {
            waker.wake();
        }
    }
}

#[derive(Clone,Debug)]
pub struct InvertedFuture<T> {
    ctx: Arc<Mutex<InvertedFutureContext<T>>>,
}
impl<T> InvertedFuture<T> {
    pub fn new() -> (Self, InvertedFutureResolver<T>) {
        let ctx = Arc::new(Mutex::new(InvertedFutureContext {
            resolution: None,
            waker: None,
        }));

        let resolver = InvertedFutureResolver { ctx: ctx.clone() };
        let future = InvertedFuture { ctx };

        (future, resolver)
    }
}
impl<T> future::Future for InvertedFuture<T> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>, 
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut ctx = self.ctx.lock().unwrap();
        ctx.waker = Some(cx.waker().clone());

        if let Some(res) = ctx.resolution.take() {
            std::task::Poll::Ready(res)
        } else {
            std::task::Poll::Pending
        }
    }
}

#[cfg(test)]
mod inverted_future_tests {
    use futures::FutureExt;

    use super::InvertedFuture;

    #[tokio::test]
    async fn unresolved_on_construction() {
        let (future, resolver) = InvertedFuture::<()>::new();
        assert_eq!(None, future.now_or_never());
    }

    #[tokio::test]
    async fn resolves_after_call_to_resolve() {
        let (future, mut resolver) = InvertedFuture::<u8>::new();
        resolver.resolve(42);
        assert_eq!(Some(42), future.now_or_never());
    }
}
