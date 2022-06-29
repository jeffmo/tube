use futures_util::future;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use hyper::body::HttpBody;

use crate::common::frame;
use crate::common::PeerType;
use super::server_context::ServerContext;
use super::server_event::ServerEvent;

pub(in crate::server) struct TubezHttpReq {
    server_ctx: Arc<Mutex<ServerContext>>,
}
impl TubezHttpReq {
    fn new(server_ctx: Arc<Mutex<ServerContext>>) -> Self {
        TubezHttpReq {
            server_ctx,
        }
    }
}
impl hyper::service::Service<hyper::Request<hyper::Body>> for TubezHttpReq {
    type Response = hyper::Response<hyper::Body>;
    type Error = hyper::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self, 
        _cx: &mut futures::task::Context<'_>,
    ) -> futures::task::Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, req: hyper::Request<hyper::Body>) -> Self::Future {
        let (body_sender, body) = hyper::Body::channel();
        let body_sender = Arc::new(tokio::sync::Mutex::new(body_sender));
        let res = hyper::Response::new(body);

        println!("Http request received! Headers: {:?}", req.headers());

        let server_ctx = self.server_ctx.clone();
        let mut body = req.into_body();
        tokio::spawn(async move {
            let mut body_sender = body_sender.clone();
            let server_ctx = server_ctx.clone();
            let mut frame_decoder = frame::Decoder::new();
            let mut tube_store = Arc::new(Mutex::new(HashMap::new()));
            let mut frame_handler = frame::FrameHandler::new(
                PeerType::Server,
                &mut tube_store,
            );

            while let Some(data_result) = body.data().await {
                let raw_data = match data_result {
                    Ok(data) => data,
                    Err(e) => {
                        println!("  stream of data from client has errored: `{:?}`", e);
                        break;
                    },
                };

                println!("  decoding {:?} bytes of raw_data from client...", raw_data.len());
                let mut new_frames = match frame_decoder.decode(raw_data.to_vec()) {
                    Ok(frames) => frames,
                    Err(e) => {
                        // TODO: What happens if we get weird data from the client? Should we 
                        //       log and dump it? Trash the request (sec implications of that?)?
                        // 
                        //       For now just log and ignore to avoid some kind of hand-wavy 
                        //       DDOS situation
                        println!("frame decode error: {:?}", e);
                        return;
                    },
                };

                println!("  decoded {:?} new frames. Handling in order...", new_frames.len());
                while let Some(frame) = new_frames.pop_front() {
                    println!("    Frame: {:?}", frame);
                    match frame_handler.handle_frame(frame, &mut body_sender).await {
                        Ok(frame::FrameHandlerResult::NewTube(tube)) => {
                            let mut server_ctx = server_ctx.lock().unwrap();
                            server_ctx.pending_events.push_back(
                                ServerEvent::NewTube(tube)
                            );
                            if let Some(waker) = server_ctx.waker.take() {
                                waker.wake();
                            }
                        },
                        Ok(frame::FrameHandlerResult::FullyHandled) => (),
                        Err(e) => eprintln!("      Error handling frame: {:?}", e),
                    }
                }
            }
            println!("Stream of httprequest data from client has ended!");
        });

        future::ok(res)
    }
}

pub(in crate::server) struct TubezMakeSvc {
    server_ctx: Arc<Mutex<ServerContext>>,
}
impl TubezMakeSvc {
    pub fn new(server_ctx: Arc<Mutex<ServerContext>>) -> Self {
        TubezMakeSvc {
            server_ctx,
        }
    }
}
impl<T> hyper::service::Service<T> for TubezMakeSvc {
    type Response = TubezHttpReq;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self, 
        _cx: &mut futures::task::Context<'_>,
    ) -> futures::task::Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _: T) -> Self::Future {
        future::ok(TubezHttpReq::new(self.server_ctx.clone()))
    }
}
