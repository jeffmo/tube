mod streamr;
mod event;

pub use streamr::Streamr;

use std::net::SocketAddr;

pub struct Server {
  running: bool,
}
impl Server {
  #[allow(dead_code)]
  fn new() -> Self {
    Server {
      running: false,
    }
  }

  pub async fn listen(&mut self, _addr: &SocketAddr) 
    -> Result<Streamr, Box<dyn std::error::Error + Send + Sync>> {
    if self.running {
      // TODO: Log an error here?
      return Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::AddrInUse,
        "Server already running!",
      )));
    }

    // TODO1: Start up an http server
    // Using example here: https://docs.rs/hyper/0.14.16/hyper/server/conn/index.html

    // TODO2: Wire the http server into self.stream_handler

    self.running = true;
    println!("Now running!");
    Ok(Streamr::new())
  }
}

#[cfg(test)]
mod server_tests {
  use tokio;

  use super::Server;

  #[tokio::test]
  async fn server_listens() {
    let mut server = Server::new();
    let addr = std::net::SocketAddr::from(([127,0,0,1], 3000));
    server.listen(&addr).await.unwrap();

    // TODO: This test is mostly for running a server manually.
    //       Need to think about how best to write a test for listen()...
  }
}
