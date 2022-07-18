use futures_util::future;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use hyper::body::HttpBody;

use crate::common::frame;
use crate::common::PeerType;
use super::channel::Channel;
use super::channel::ChannelContext;
use super::channel::ChannelEvent;
use super::server_context::ServerContext;
use super::server_event::ServerEvent;

pub(in crate::server) struct TubezHttpReq {
    channel_ctx: Weak<Mutex<ChannelContext>>,
    server_ctx: Arc<Mutex<ServerContext>>,
}
impl TubezHttpReq {
    fn new(
        server_ctx: Arc<Mutex<ServerContext>>,
        channel_ctx: Weak<Mutex<ChannelContext>>,
    ) -> Self {
        TubezHttpReq {
            channel_ctx,
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
        let mut body_sender = Arc::new(tokio::sync::Mutex::new(body_sender));
        let res = hyper::Response::new(body);

        log::trace!("Http request received. Headers: {:?}", req.headers());

        let channel_ctx = self.channel_ctx.clone();
        let server_ctx = self.server_ctx.clone();
        let mut body = req.into_body();
        tokio::spawn(async move {
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
                        log::error!("Stream of data from client has errored: `{:?}`", e);
                        break;
                    },
                };

                let mut new_frames = match frame_decoder.decode(raw_data.to_vec()) {
                    Ok(frames) => frames,
                    Err(e) => {
                        // TODO: What happens if we get weird data from the client? Should we 
                        //       log and dump it? Trash the request (sec implications of that?)?
                        // 
                        //       For now just log and ignore to avoid some kind of hand-wavy 
                        //       DDOS situation
                        log::error!("frame decode error: {:?}", e);
                        return;
                    },
                };

                while let Some(frame) = new_frames.pop_front() {
                    log::trace!("New frame received: {:?}", frame);
                    match frame_handler.handle_frame(frame, &mut body_sender).await {
                        Ok(frame::FrameHandlerResult::NewTube(mut tube)) => {
                            if let Some(channel_ctx) = Weak::upgrade(&channel_ctx) {
                                let mut channel_ctx = channel_ctx.lock().unwrap();
                                channel_ctx.pending_events.push_back(
                                    ChannelEvent::NewTube(tube)
                                );
                                if let Some(waker) = channel_ctx.waker.take() {
                                    waker.wake();
                                }
                            } else {
                                log::error!("Received a new Tube from the client on a channel that has been dropped!");
                                match tube.abort_internal(
                                    frame::AbortReason::ApplicationError
                                ).await {
                                    Ok(()) => (),
                                    Err(e) => 
                                        log::error!("Error aborting tube: `{:?}`", e),
                                }
                            }
                        },
                        Ok(frame::FrameHandlerResult::FullyHandled) => (),
                        Err(e) => log::error!("Error handling frame: {:?}", e),
                    }
                }
            }
            log::trace!("Stream of httprequest data from client has ended.");
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

    fn publish_channel(&mut self, channel: Channel) {
        let mut server_ctx = self.server_ctx.lock().unwrap();
        server_ctx.pending_events.push_back(
            Ok(ServerEvent::NewChannel(channel))
        );
        if let Some(waker) = server_ctx.waker.take() {
            waker.wake();
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
        let channel_ctx = Arc::new(Mutex::new(ChannelContext::new()));
        let weak_channel = Arc::downgrade(&channel_ctx);
        let channel = Channel::new(channel_ctx);
        self.publish_channel(channel);
        future::ok(TubezHttpReq::new(
            self.server_ctx.clone(),
            weak_channel,
        ))
    }
}
