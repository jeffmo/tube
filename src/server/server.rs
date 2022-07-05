use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;

use crate::common::tube::Tube;
use super::hyper_tubez_service::TubezMakeSvc;
use super::server_context::ServerContext;
use super::server_error::ServerError;
use super::server_event::ServerEvent;

pub struct Server {
    server_ctx: Arc<Mutex<ServerContext>>,
}
impl Server {
    pub async fn new(addr: &SocketAddr) -> Self {
        let server_ctx = Arc::new(Mutex::new(ServerContext {
            is_complete: false,
            pending_events: VecDeque::new(),
            waker: None,
        }));

        let hyper_server = 
            hyper::Server::bind(&addr)
                .http2_only(true)
                .serve(TubezMakeSvc::new(server_ctx.clone()));

        let tubez_server = Server {
            server_ctx: server_ctx.clone(),
        };

        tokio::spawn(async move {
            if let Err(e) = hyper_server.await {
                let mut server_ctx = server_ctx.lock().unwrap();
                let error_msg = format!("Server error: {}", e);
                eprintln!("{}", error_msg);
                server_ctx.pending_events.push_back(Err(ServerError::Err(format!("{:?}", e))));
                // TODO: Need to iterate all tubes and error them here as well.
                if let Some(waker) = server_ctx.waker.take() {
                    waker.wake();
                };
            } else {
                // TODO: Indicate that the http request has EOM'd? Not sure...
                // 
                //         "completes when the server has been shutdown"
                //         https://docs.rs/hyper/latest/hyper/server/struct.Server.html
            }
        });

        tubez_server
    }

    pub async fn new_tube() /*TODO: -> Tube*/ {
        // TODO: This is just a boilerplate mitigator...
        //       Make a channel internal to Server{} and basically hide that 
        //       channel so that users only have to care about Server emitting 
        //       ServerEvent::NewTube() events...
    }
}
impl futures::stream::Stream for Server {
    type Item = Result<ServerEvent, ServerError>;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        let mut server_ctx = self.server_ctx.lock().unwrap();
        server_ctx.waker = Some(cx.waker().clone());
        if let Some(event) = server_ctx.pending_events.pop_front() {
            futures::task::Poll::Ready(Some(event))
        } else {
            if server_ctx.is_complete {
                futures::task::Poll::Ready(None)
            } else {
                futures::task::Poll::Pending
            }
        }
    }
}
