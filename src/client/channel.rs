use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use hyper::body::HttpBody;

use crate::common::PeerType;
use crate::common::frame;
use crate::common::tube;

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

        let body_sender_weak = Arc::downgrade(&body_sender);
        let tube_mgrs2 = tube_managers.clone();
        tokio::spawn(async move {
            let mut tube_mgrs = tube_mgrs2;
            let mut frame_decoder = frame::Decoder::new();
            let mut frame_handler = frame::FrameHandler::new(
                PeerType::Client,
                &mut tube_mgrs,
            );

            while let Some(data_result) = res_body.data().await {
                // This seems hacky...but it works.
                //
                // When the sender is dropped, res_body.data().await yields 
                // Some(Buf{}) (an empty Buf)...presumably to indicate EOM? 
                // Weird...but I guess it works?
                //
                // A better solution might be to wrap res_body.data() inside some
                // stream that ends when EITHER .data() returns None OR 
                // body_sender is dropped. That way the async loop 
                // /intentionally/ polls and stops iterating when all tubes + 
                // channels have been dropped.
                let mut body_sender = match body_sender_weak.upgrade() {
                    Some(body_sender) => body_sender,
                    None => break,
                };

                println!("Server data received!");
                let raw_data = match data_result {
                    Ok(data) => data,
                    Err(e) => {
                        println!("  stream of data from server has errored: `{:?}`", e);
                        break;
                    }
                };

                println!("  decoding {:?} bytes of raw_data from server...", raw_data.len());
                let mut new_frames = match frame_decoder.decode(raw_data.to_vec()) {
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
                while let Some(frame) = new_frames.pop_front() {
                    println!("    Frame: {:?}", frame);
                    match frame_handler.handle_frame(frame, &mut body_sender).await {
                        Ok(frame::FrameHandlerResult::NewTube(tube)) => {
                            // TODO: Server-initiated tubes aren't supported yet.
                        },
                        Ok(frame::FrameHandlerResult::FullyHandled) => (),
                        Err(e) => eprintln!("      Error handling frame: {:?}", e),
                    }
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
        let tube = tube::Tube::new(
            PeerType::Client, 
            tube_id, 
            self.body_sender.clone(), 
            tube_mgr.clone(),
        );
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
