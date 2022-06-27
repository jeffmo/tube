use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use hyper::body::HttpBody;

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
    ) -> Result<Self, ChannelConnectError> {
        let (body_sender, req_body) = hyper::Body::channel();
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
        tokio::spawn(async move {
            while let Some(data_result) = res_body.data().await {
                println!("Server data received!");
                match data_result {
                    Ok(data) => println!("  server_data: `{:?}`", data),
                    Err(e) => {
                        println!("  response body has errored: `{:?}`", e);
                        break;
                    }
                }
            }
        });

        Ok(Channel {
            body_sender: Arc::new(tokio::sync::Mutex::new(body_sender)),
            tube_id_counter: 0,
            tube_managers: Arc::new(Mutex::new(HashMap::new())),
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
          Err(e) => Err(MakeTubeError::InternalErrorDuplicateTubeId(tube_id)),
        }
    }

    fn new_tube_id(&mut self) -> u16 {
        let new_id = self.tube_id_counter;
        self.tube_id_counter += 2;
        new_id
    }
}
