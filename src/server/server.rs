use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;

use hyper;

use crate::server::hyper_tubez_service::TubezMakeSvc;
use crate::server::server_context::ServerContext;
pub use crate::server::server_event::ServerEvent;
use crate::server::tube::Tube;

#[derive(Debug)]
pub enum ServerError {
    // TODO: Actually enumerate errors...
    Err(String)
}

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
                server_ctx.pending_events.push_back(ServerEvent::Err(e));
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
}
impl futures::stream::Stream for Server {
    type Item = Result<Tube, ServerError>;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        let mut server_ctx = self.server_ctx.lock().unwrap();
        server_ctx.waker = Some(cx.waker().clone());
        match server_ctx.pending_events.pop_front() {
          Some(ServerEvent::NewTube(tube)) => futures::task::Poll::Ready(Some(Ok(tube))),
          Some(ServerEvent::Err(e)) => futures::task::Poll::Ready(Some(Err(
            ServerError::Err(format!("{}", e))
          ))),
          None => 
            if server_ctx.is_complete {
              futures::task::Poll::Ready(None)
            } else {
              futures::task::Poll::Pending
            }
        }
    }
}
