use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use hyper;

use super::channel::Channel;
use super::channel::ChannelConnectError;

pub enum MakeChannelError {
  ChannelConnectError(ChannelConnectError)
}

pub struct Client {
  channels: Vec<Arc<Mutex<Channel>>>,
  hyper_client: hyper::Client<hyper::client::HttpConnector>,
}
impl Client {
  pub fn new() -> Self {
    let hyper_client: hyper::Client<hyper::client::HttpConnector> = 
      hyper::Client::builder()
        .http2_only(true)
        .build_http();

    Client {
      channels: vec![],
      hyper_client,
    }
  }

  pub async fn make_tube_channel(
    &mut self,
    headers: HashMap<String, String>,
  ) -> Result<Channel, ChannelConnectError> {
    Channel::new(&self.hyper_client).await
  }
}
