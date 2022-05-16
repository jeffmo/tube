use std::collections::VecDeque;
use std::net::SocketAddr;

use hyper;

use crate::streamr::Streamr;

async fn streamr_request_handler(
    _req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, std::convert::Infallible> {
    let (mut body_sender, body) = hyper::Body::channel();
    let response: hyper::Response<hyper::Body> = hyper::Response::new(body);

    println!("Spawning second task...");
    tokio::spawn(async move {
        println!("Sending first chunk...");
        match body_sender.send_data("First chunk...\n".into()).await {
            Ok(()) => println!("First chunk sent!"),
            Err(err) => panic!("First chunk failed to send! {:?}", err)
        };
        println!("Waiting 3s before second chunk...");
        tokio::time::sleep(std::time::Duration::from_millis(3000)).await;

        println!("Sending second chunk...");
        match body_sender.send_data("...second chunk.\n".into()).await {
            Ok(()) => println!("Second chunk sent!"),
            Err(err) => panic!("Second chunk failed to send! {:?}", err),
        };
        println!("Second task complete");
    });
    println!("Returning response object...");
    Ok(response)
}

pub struct Server {
    test_streamrs: Option<VecDeque<Streamr>>,
}
impl Server {
    #[allow(dead_code)]
    pub async fn new(addr: &SocketAddr) -> Self {
        // TODO1: Start up an http server
        // Using example here: https://docs.rs/hyper/0.14.16/hyper/server/conn/index.html

        let streamr_makeservice = hyper::service::make_service_fn(
            |_conn: &hyper::server::conn::AddrStream| {
                async {
                    Ok::<_, std::convert::Infallible>(hyper::service::service_fn(
                        streamr_request_handler
                    ))
                }
            }
        );

        let hyper_server = 
            hyper::Server::bind(&addr)
                .http2_only(true)
                .serve(streamr_makeservice);

        if let Err(e) = hyper_server.await {
            eprintln!("server error: {}", e);
        }

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
        let mut server = Server::new(&addr).await;

        server.set_test_streamrs(vec![
            Streamr::new(),
            Streamr::new(),
        ]);

        let actual_streamrs = server.collect::<Vec<Streamr>>().await;
        assert_eq!(actual_streamrs.len(), 2);
    }
}
