use futures_util::future;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use hyper;
use hyper::body::HttpBody;

use crate::common::frame;
use crate::common::tube;
use crate::server::server_context::ServerContext;
use crate::server::server_event::ServerEvent;

async fn handle_frame(
    sender: &Arc<tokio::sync::Mutex<hyper::body::Sender>>,
    server_ctx: &Arc<Mutex<ServerContext>>,
    tube_store: &mut HashMap<u16, Arc<Mutex<tube::TubeManager>>>, 
    frame: &frame::Frame,
) {
    match frame {
        frame::Frame::ClientHasFinishedSending {
            tube_id,
        } => {
            // TODO
        },
        frame::Frame::Drain => {
            // TODO
        },
        frame::Frame::EstablishTube { 
            tube_id, 
            headers,
        } => {
            // TODO: First Authenticate here...

            let tube_mgr = Arc::new(Mutex::new(tube::TubeManager::new()));

            println!("      Storing TubeManager...");
            if let Err(e) = tube_store.try_insert(*tube_id, tube_mgr.clone()) {
                // TODO: Hmmm...should we respond to the client in some way with an error?
                eprintln!("Error storing TubeManager for new tube: {:?}", e);
                return;
            }

            println!("      Emitting tube...");
            let tube = tube::Tube::new_on_server(
                *tube_id,
                sender.clone(),
                tube_mgr,
            );
            let mut server_ctx = server_ctx.lock().unwrap();
            server_ctx.pending_events.push_back(ServerEvent::NewTube(tube));
            if let Some(waker) = server_ctx.waker.take() {
                waker.wake();
            }
        },
        frame::Frame::Payload {
            tube_id,
            ack_id,
            data,
        } => {
            let tube_mgr = match tube_store.get(tube_id) {
                Some(tube_ctx) => tube_ctx,
                None => {
                    // TODO: Hmm...send back some kind of an error to the client?
                    eprintln!(
                        "Received a Payload frame for Tube({:?}) that we aren't aware of!", 
                        tube_id
                    );
                    return;
                },
            };

            // If an ack was requested, send one...
            if let Some(ack_id) = ack_id {
                let frame_data = match frame::encode_payload_ack_frame(*tube_id, *ack_id) {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("Error encoding Ack frame: {:?}", e);
                        return;
                    },
                };
                let mut sender = sender.lock().await;
                match sender.send_data(frame_data.into()).await {
                    Ok(_) => (),
                    Err(e) => {
                        eprintln!("Error sending ack frame to client: {:?}", e);
                        return
                    },
                }
            }

            let mut tube_mgr = tube_mgr.lock().unwrap();
            tube_mgr.pending_events.push_back(tube::TubeEvent::Payload(data.to_vec()));
            if let Some(waker) = tube_mgr.waker.take() {
                waker.wake();
            }
        },
        frame::Frame::PayloadAck {
            tube_id,
            ack_id,
        } => {
            // TODO
        },
        frame::Frame::ServerHasFinishedSending {
            tube_id,
        } => {
            // TODO
        },
    }
}

pub(in crate) struct TubezHttpReq {
    server_ctx: Arc<Mutex<ServerContext>>,
}
impl TubezHttpReq {
    pub(in crate) fn new(server_ctx: Arc<Mutex<ServerContext>>) -> Self {
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
        // TODO: !!!!! 
        //       Use a tokio::sync::Mutex so that locks held while awaiting a data 
        //       send are safer/less prone to deadlocks as the send itself is .awaited
        //
        //       See:
        //       https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html
        //
        let body_sender = Arc::new(tokio::sync::Mutex::new(body_sender));
        let res = hyper::Response::new(body);

        println!("Http request received! Headers: {:?}", req.headers());

        let server_ctx = self.server_ctx.clone();
        let mut body = req.into_body();
        tokio::spawn(async move {
            let body_sender = body_sender.clone();
            let server_ctx = server_ctx.clone();
            let mut frame_decoder = frame::Decoder::new();
            let mut tube_store = HashMap::new();

            while let Some(data_result) = body.data().await {
                let raw_data = match data_result {
                    Ok(data) => data,
                    Err(e) => {
                        println!("  stream of data from client has errored: `{:?}`", e);
                        break;
                    },
                };

                println!("  decoding {:?} bytes of raw_data from client...", raw_data.len());
                let new_frames = match frame_decoder.decode(raw_data.to_vec()) {
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
                for frame in &new_frames {
                    println!("    Frame: {:?}", frame);
                    handle_frame(
                        &body_sender, 
                        &server_ctx, 
                        &mut tube_store, 
                        frame
                    ).await;
                }
            }
        });

        future::ok(res)
    }
}

pub(in crate) struct TubezMakeSvc {
    server_ctx: Arc<Mutex<ServerContext>>,
}
impl TubezMakeSvc {
    pub(in crate) fn new(server_ctx: Arc<Mutex<ServerContext>>) -> Self {
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
