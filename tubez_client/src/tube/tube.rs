use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct Tube {
  sender: Arc<Mutex<hyper::body::Sender>>,
  tube_id: u16,
}
impl Tube {
  pub(in crate) fn new(
    tube_id: u16,
    sender: Arc<Mutex<hyper::body::Sender>>,
  ) -> Self {
    Tube {
      sender,
      tube_id,
    }
  }

  pub fn get_id(&self) -> u16 {
    return self.tube_id;
  }
}
