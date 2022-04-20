mod streamr;
mod event;

pub use streamr::Streamr;

use std::net::SocketAddr;

pub struct Server {
}
impl Server {
  #[allow(dead_code)]
  fn new(_addr: &SocketAddr) -> Self {
    // TODO1: Start up an http server
    // Using example here: https://docs.rs/hyper/0.14.16/hyper/server/conn/index.html

    // TODO2: Wire the http server into self.stream_handler

    Server {}
  }
}

#[cfg(test)]
mod server_tests {
  use tokio;

  use super::Server;

  #[tokio::test]
  async fn server_listens() {
    let addr = std::net::SocketAddr::from(([127,0,0,1], 3000));
    let _server = Server::new(&addr);

    // TODO: This test is mostly for running a server manually.
    //       Need to think about how best to write a test for listen()...
  }
}
