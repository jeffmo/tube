use std::collections::VecDeque;
use std::net::SocketAddr;

use crate::streamr::Streamr;

pub struct Server {
    test_streamrs: Option<VecDeque<Streamr>>,
}
impl Server {
  #[allow(dead_code)]
  pub fn new(_addr: &SocketAddr) -> Self {
    // TODO1: Start up an http server
    // Using example here: https://docs.rs/hyper/0.14.16/hyper/server/conn/index.html

    // TODO2: Wire the http server into self.stream_handler

    Server {
        test_streamrs: None,
    }
  }
}
impl futures::stream::Stream for Server {
    type Item = Streamr;

    fn poll_next(
        mut self: core::pin::Pin<&mut Self>,
        _cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        let next_streamr = match self.test_streamrs.as_mut() {
            Some(test_streamrs) => test_streamrs.pop_front(),

            // TODO
            None => panic!("No test streamrs found and http has not been integrated yet!"),
        };

        futures::task::Poll::Ready(next_streamr)
    }
}

#[cfg(test)]
impl Server {
    pub fn set_test_streamrs(&mut self, streamrs: Vec<Streamr>) {
        self.test_streamrs = Some(VecDeque::from(streamrs))
    }
}

#[cfg(test)]
mod server_tests {
    use futures::StreamExt;
    use tokio;

    use super::Server;
    use crate::streamr::Streamr;

    // TODO: This test is silly and basically just tests the set_test_streamrs 
    //       mechanics. Delete it when there's something more useful to write a 
    //       test around.
    #[tokio::test]
    async fn server_listens() {
        let addr = std::net::SocketAddr::from(([127,0,0,1], 3000));
        let mut server = Server::new(&addr);

        server.set_test_streamrs(vec![
            Streamr::new(),
            Streamr::new(),
        ]);

        let actual_streamrs = server.collect::<Vec<Streamr>>().await;
        assert_eq!(actual_streamrs.len(), 2);
    }
}
