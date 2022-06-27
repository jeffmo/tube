use std::sync::Arc;
use std::sync::Mutex;

use super::tube_manager::TubeManager;

#[derive(Debug)]
pub struct Tube {
    available_ackids: VecDeque<u16>,
    ack_id_counter: u16,

    // TODO
    //sendacks: HashMap<u16, Arc<Mutex<SendAckFutureContext>>>,

    sender: Arc<Mutex<hyper::body::Sender>>,
    tube_id: u16,
    tube_manager: Arc<Mutex<TubeManager>>,
}
impl Tube {
    pub(in crate) fn new(
      tube_id: u16,
      sender: Arc<Mutex<hyper::body::Sender>>,
      tube_manager: Arc<Mutex<TubeManager>>,
    ) -> Self {
      Tube {
        ack_id_counter: 0,
        available_ackids: VecDeque::new(),
        sender,
        tube_id,
        tube_manager,
      }
    }

    pub fn get_id(&self) -> u16 {
      return self.tube_id;
    }

    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), SendError> {
      let ack_id = self.take_ackid();
      match frame::encode_payload_frame(self.tube_id, Some(ack_id), data) {
          Ok(frame_data) => {
              let sendack_ctx = Arc::new(Mutex::new(SendAckFutureContext {
                  ack_received: false,
              }));
              if let Err(_) = self.sendacks.try_insert(ack_id, sendack_ctx.clone()) {
                  return Err(SendError::AckIdAlreadyInUseInternalError)
              }

              {
                  let mut sender = self.sender.lock().await;
                  match sender.send_data(frame_data.into()).await {
                      Ok(_) => (),
                      Err(e) => {
                          self.sendacks.remove(&ack_id);
                          return Err(SendError::TransportError(e))
                      },
                  }
              };

              SendAckFuture {
                  ctx: sendack_ctx.clone(),
              }.await;
              self.sendacks.remove(&ack_id);

              // TODO: Await here for the return of an ACK from the client

              Ok(())
          },
          Err(e) => Err(SendError::FrameEncodeError(e)),
      }
    }
}
