use std::collections::HashMap;

use crate::tube;
use super::channel;

pub enum ServerMakeTubeError {
    ChannelConnectError(channel::ChannelConnectError),
    MakeTubeError(channel::MakeTubeError),
}

pub struct Client {
  hyper_client: hyper::Client<hyper::client::HttpConnector>,
  implicit_channel: Option<channel::Channel>,
}
impl Client {
  pub fn new() -> Self {
    let hyper_client: hyper::Client<hyper::client::HttpConnector> = 
      hyper::Client::builder()
        .http2_only(true)
        .build_http();

    Client {
      hyper_client,
      implicit_channel: None,
    }
  }

  pub async fn make_tube_channel(
    &mut self,
    headers: HashMap<String, String>,
  ) -> Result<channel::Channel, channel::ChannelConnectError> {
    channel::Channel::new(&self.hyper_client, headers).await
  }

  pub async fn new_tube(
      &mut self,
      headers: HashMap<String, String>,
  ) -> Result<tube::Tube, ServerMakeTubeError> {
      let channel = match self.implicit_channel.as_mut() {
          Some(channel) => channel,
          None => match self.make_tube_channel(HashMap::new()).await {
              Ok(channel) => {
                  self.implicit_channel = Some(channel);
                  self.implicit_channel.as_mut().unwrap()
              },
              Err(e) => return Err(ServerMakeTubeError::ChannelConnectError(e)),
          },
      };

      match channel.make_tube(headers).await {
          Ok(tube) => Ok(tube),
          Err(e) => Err(ServerMakeTubeError::MakeTubeError(e)),
      }
  }
}
