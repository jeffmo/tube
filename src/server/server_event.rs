use crate::common::tube::Tube;
use super::channel::Channel;

#[derive(Debug)]
pub enum ServerEvent {
  NewChannel(Channel),
  //NewTube(Tube),  // TODO: Re-implement when Client::new_tube() is built
}

