use std::sync::Arc;
use std::sync::Mutex;

use crate::tube::Tube;

#[derive(Debug)]
pub enum ServerEvent {
  NewTube(Tube),
  Err(hyper::Error),
}

