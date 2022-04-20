mod stream;
mod event;

pub use stream::ServerStream;

use std::net::SocketAddr;

pub struct Server {
  running: bool,
}
impl Server {
  fn new() -> Self {
    Server {
      running: false,
    }
  }

  pub async fn listen(&mut self, addr: &SocketAddr) -> Result<ServerStream, Box<dyn std::error::Error + Send + Sync>> {
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
    Ok(ServerStream::new())
  }
}

#[cfg(test)]
mod server_tests {
  use super::{
    Server,
    event::ServerEvent,
    event::ServerEvent_StreamError,
    event::ServerEventTag,
    // ServerStream,
  };
  use tokio;
  use futures::StreamExt;

  #[tokio::test]
  async fn constructs() {
    let mut server = Server::new();
    let addr = std::net::SocketAddr::from(([127,0,0,1], 3000));
    let mut server_stream = server.listen(&addr).await.unwrap();


    /////////////////////////////
    // !!!===PICK UP HERE==!!! //
    /////////////////////////////

    // TODO: Use "#[cfg(test)]" to implement a test-only function on the 
    //       ServerStream trait that allows this test to specify a series of 
    //       client-sent Streamr frames

    let events = 
      server_stream
        .collect::<Vec<ServerEvent>>().await;
    let events2 = events.clone();
    let event_tags: Vec<ServerEventTag> = 
      events.into_iter()
        .map(|event| ServerEventTag::from(&event))
        .collect();

    assert_eq!(event_tags, [
       ServerEventTag::AuthenticatedAndReady, 
       ServerEventTag::StreamError,
    ]);

    match &events2[1] {
      ServerEvent::StreamError(
        ServerEvent_StreamError::InvalidServerEventTransition(from, to)
      ) => {
        assert_eq!(*from, ServerEventTag::AuthenticatedAndReady);
        assert_eq!(*to, ServerEventTag::AuthenticatedAndReady);
      },
      _ => panic!(),
    };
  }
}
