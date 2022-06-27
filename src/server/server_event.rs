use crate::common::tube::Tube;

#[derive(Debug)]
pub enum ServerEvent {
  NewTube(Tube),
  Err(hyper::Error),
}

