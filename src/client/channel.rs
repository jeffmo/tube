use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use hyper::body::HttpBody;

use crate::common::frame;
use crate::common::tube;

async fn handle_frame(
    sender: &Arc<tokio::sync::Mutex<hyper::body::Sender>>,
    tube_managers: &Arc<Mutex<HashMap<u16, Arc<Mutex<tube::TubeManager>>>>>, 
    frame: &frame::Frame,
) {
    match frame {
        frame::Frame::ClientHasFinishedSending {
            tube_id,
        } => {
            eprintln!("Received a ClientHasFinishedSending frame from the server...uhhh...");
            return;
        },
        frame::Frame::Drain => {
            // TODO
        },
        frame::Frame::EstablishTube { 
            tube_id, 
            headers,
        } => {
            // TODO
        },
        frame::Frame::Payload {
            tube_id,
            ack_id,
            data,
        } => {
            let tube_mgr = {
                let tube_mgrs = tube_managers.lock().unwrap();
                match tube_mgrs.get(tube_id) {
                    Some(tube_ctx) => tube_ctx.clone(),
                    None => {
                        // TODO: Hmm...send back some kind of an error to the client?
                        eprintln!(
                            "Received a Payload frame for Tube({:?}) that we aren't aware of!", 
                            tube_id
                        );
                        return;
                    },
                }
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
            let tube_mgr = {
              let tube_mgrs = tube_managers.lock().unwrap();
              match tube_mgrs.get(tube_id) {
                    Some(tube_mgr) => tube_mgr.clone(),
                    None => {
                        eprintln!("Received a PayloadAck frame from the client for tube_id={:?}, which is not a Tube the server is tracking!", tube_id);
                        return;
                    }
              }
            };

            let sendack_ctx = {
                let tube_mgr = tube_mgr.lock().unwrap();
                match tube_mgr.sendacks.get(ack_id) {
                    Some(ctx) => ctx.clone(),
                    None => {
                        eprintln!("Received a PayloadAck(id={:?}) for Tube(id={:?}), but this is not an ack_id this tube is tracking!", ack_id, tube_id);
                        return;
                    }
                }
            };

            {
                let mut sendack_ctx = sendack_ctx.lock().unwrap();
                sendack_ctx.ack_received = true;
                if let Some(waker) = sendack_ctx.waker.take() {
                  waker.wake();
                }
            }
        },
        frame::Frame::ServerHasFinishedSending {
            tube_id,
        } => {
            let tube_mgr = {
                let tube_mgrs = tube_managers.lock().unwrap();
                match tube_mgrs.get(tube_id) {
                    Some(tube_ctx) => tube_ctx.clone(),
                    None => {
                        // TODO: Hmm...send back some kind of an error to the client?
                        eprintln!(
                            "Received a ServerHasFinishedSending frame for Tube({:?}) that we aren't aware of!", 
                            tube_id
                        );
                        return;
                    },
                }
            };
            let mut tube_mgr = tube_mgr.lock().unwrap();
            let new_state = {
                use tube::TubeCompletionState::*;
                match tube_mgr.completion_state {
                    Open => ServerHasFinishedSending,
                    ClientHasFinishedSending => Closed,
                    ServerHasFinishedSending => {
                        eprintln!("Server sent ServerHasFinishedSending frame twice!");
                        ServerHasFinishedSending
                    },
                    Closed => {
                        eprintln!("Server sent ServerHasFinishedSending frame twice!");
                        Closed
                    },
                }
            };
            if tube_mgr.completion_state != new_state {
                tube_mgr.completion_state = new_state;
                if tube_mgr.completion_state == tube::TubeCompletionState::ServerHasFinishedSending {
                    tube_mgr.pending_events.push_back(tube::TubeEvent::ServerHasFinishedSending);
                }
                if let Some(waker) = tube_mgr.waker.take() {
                  waker.wake();
                }
            }
        },
    }
}

#[derive(Debug)]
pub enum ChannelConnectError {
    InitError(hyper::Error),
}

#[derive(Debug)]
pub enum MakeTubeError {
    FrameEncodeError(frame::FrameEncodeError),
    InternalErrorDuplicateTubeId(u16),
    UnknownTransportError,
}

pub struct Channel {
    body_sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>,
    tube_id_counter: u16,
    tube_managers: Arc<Mutex<HashMap<u16, Arc<Mutex<tube::TubeManager>>>>>,
}
impl Channel {
    pub(in crate::client) async fn new(
        hyper_client: &hyper::Client<hyper::client::HttpConnector>,
        _headers: HashMap<String, String>, // TODO
    ) -> Result<Self, ChannelConnectError> {
        let (body_sender, req_body) = hyper::Body::channel();
        let body_sender = Arc::new(tokio::sync::Mutex::new(body_sender));
        let req = hyper::Request::builder()
          .method(hyper::Method::POST)
          .uri("http://127.0.0.1:3000/")
          .body(req_body)
          .unwrap();

        println!("Sending channel request...");
        let response = match hyper_client.request(req).await {
            Ok(response) => response,
            Err(e) => return Err(ChannelConnectError::InitError(e)),
        };
        let mut res_body = response.into_body();
        let tube_managers = Arc::new(Mutex::new(HashMap::new()));

        let body_sender2 = body_sender.clone();
        let tube_mgrs2 = tube_managers.clone();
        tokio::spawn(async move {
            let tube_mgrs = tube_mgrs2;
            let body_sender = body_sender2;
            let mut frame_decoder = frame::Decoder::new();

            while let Some(data_result) = res_body.data().await {
                println!("Server data received!");
                let raw_data = match data_result {
                    Ok(data) => data,
                    Err(e) => {
                        println!("  stream of data from server has errored: `{:?}`", e);
                        break;
                    }
                };

                println!("  decoding {:?} bytes of raw_data from server...", raw_data.len());
                let new_frames = match frame_decoder.decode(raw_data.to_vec()) {
                    Ok(frames) => frames,
                    Err(e) => {
                        // TODO: What happens if we get weird data from the server? Should we 
                        //       log and dump it? Trash the request (sec implications of that?)?
                        // 
                        //       For now just log and ignore to avoid some kind of hand-wavy 
                        //       DDOS situation
                        eprintln!("frame decode error: {:?}", e);
                        return;
                    },
                };

                println!("  decoded {:?} new frames. Handling in order...", new_frames.len());
                for frame in &new_frames {
                    println!("    Frame: {:?}", frame);
                    handle_frame(
                        &body_sender,
                        &tube_mgrs,
                        frame,
                    ).await;
                }
            }
        });

        Ok(Channel {
            body_sender: body_sender,
            tube_id_counter: 0,
            tube_managers,
        })
    }

    pub async fn make_tube(
        &mut self, 
        headers: HashMap<String, String>,
    ) -> Result<tube::Tube, MakeTubeError> {
        let tube_id = self.new_tube_id();
        let estab_tube_frame = match frame::encode_establish_tube_frame(tube_id, headers) {
            Ok(data) => data,
            Err(e) => return Err(MakeTubeError::FrameEncodeError(e)),
        };

        {
            let mut body_sender = self.body_sender.lock().await;
            match body_sender.send_data(estab_tube_frame.into()).await {
                Ok(_) => (),
                Err(_bytes) => return Err(MakeTubeError::UnknownTransportError),
            };
        };

        println!("Awaiting response EstablishTube frame...(TODO)");
        // TODO: Await the return of an EstablishTube frame from the server here 
        //       before creating a Tube and returning it.

        let tube_mgr = Arc::new(Mutex::new(tube::TubeManager::new()));
        let tube = tube::Tube::new_on_client(tube_id, self.body_sender.clone(), tube_mgr.clone());
        let mut tube_managers = self.tube_managers.lock().unwrap();
        match tube_managers.try_insert(tube_id, tube_mgr) {
          Ok(_) => Ok(tube),
          Err(_) => Err(MakeTubeError::InternalErrorDuplicateTubeId(tube_id)),
        }
    }

    fn new_tube_id(&mut self) -> u16 {
        let new_id = self.tube_id_counter;
        self.tube_id_counter += 2;
        new_id
    }
}
