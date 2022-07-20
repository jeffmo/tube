use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use hyper::body::HttpBody;

use crate::common::frame;
use crate::common::PeerType;
use crate::common::tube;
use crate::common::UniqueIdError;
use crate::common::UniqueIdManager;

#[derive(Debug)]
pub enum ChannelConnectError {
    InitError(hyper::Error),
}

#[derive(Debug)]
pub enum MakeTubeError {
    FrameEncodeError(frame::FrameEncodeError),
    InternalErrorDuplicateTubeId(u16),
    TubeIdsExhausted,
    UnknownTransportError,
}

pub struct Channel {
    body_sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>,
    tube_id_manager: UniqueIdManager,
    tube_managers: Arc<Mutex<HashMap<u16, Arc<Mutex<tube::TubeManager>>>>>,
}
impl Channel {
    pub(in crate::client) async fn new(
        hyper_client: &hyper::Client<hyper::client::HttpConnector>,
        _headers: HashMap<String, String>, // TODO
        server_uri: &hyper::Uri,
    ) -> Result<Self, ChannelConnectError> {
        let (body_sender, req_body) = hyper::Body::channel();
        let body_sender = Arc::new(tokio::sync::Mutex::new(body_sender));
        let req = hyper::Request::builder()
          .method(hyper::Method::POST)
          .uri(format!("{}", &server_uri))
          .body(req_body)
          .unwrap();

        log::trace!("Sending channel request to {}...", &server_uri);
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

                let raw_data = match data_result {
                    Ok(data) => data,
                    Err(e) => {
                        log::trace!("Stream of data from server has errored: `{:?}`", e);
                        break;
                    }
                };

                let mut new_frames = match frame_decoder.decode(raw_data.to_vec()) {
                    Ok(frames) => frames,
                    Err(e) => {
                        log::error!("Frame decode error: {:?}", e);
                        return;
                    },
                };

                while let Some(frame) = new_frames.pop_front() {
                    log::trace!("Processing frame: {:?}", frame);
                    match frame_handler.handle_frame(frame, &mut body_sender).await {
                        Ok(frame::FrameHandlerResult::NewTube(_tube)) => {
                            // TODO: Server-initiated tubes aren't supported yet.
                            log::error!(
                                "Received a NewTube frame from the server, but \
                                 server-initiated tubes aren't supported yet!"
                            );
                        },
                        Ok(frame::FrameHandlerResult::FullyHandled) => (),
                        Err(e) => log::error!("Error handling frame: {:?}", e),
                    }
                }
            }
        });

        Ok(Channel {
            body_sender: body_sender,
            tube_id_manager: UniqueIdManager::new_with_odd_ids(),
            tube_managers,
        })
    }

    pub async fn make_tube(
        &mut self, 
        headers: HashMap<String, String>,
    ) -> Result<tube::Tube, MakeTubeError> {
        let tube_id = match self.tube_id_manager.take_id() {
          Ok(id) => id,
          Err(UniqueIdError::NoIdsAvailable) => 
            return Err(MakeTubeError::TubeIdsExhausted),
        };
        let estab_tube_frame = match frame::encode_newtube_frame(tube_id.val(), headers) {
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

        log::trace!("Awaiting response NewTube frame...(TODO)");
        // TODO: Await the return of an NewTube frame from the server here 
        //       before creating a Tube and returning it.

        let tube_mgr = Arc::new(Mutex::new(tube::TubeManager::new()));
        let tube_id_val = tube_id.val();
        let tube = tube::Tube::new(
            PeerType::Client, 
            tube_id, 
            self.body_sender.clone(), 
            tube_mgr.clone(),
        );
        let mut tube_managers = self.tube_managers.lock().unwrap();
        match tube_managers.try_insert(tube_id_val, tube_mgr) {
            Ok(_) => Ok(tube),
            Err(_) => Err(MakeTubeError::InternalErrorDuplicateTubeId(tube_id_val)),
        }
    }
}

#[cfg(test)]
mod channel_tests {
    // TODO
}
