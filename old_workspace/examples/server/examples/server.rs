use futures::StreamExt;

extern crate tubez_server;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let addr = std::net::SocketAddr::from(([127,0,0,1], 3000));
    println!("Starting server...");
    let mut server = tubez_server::Server::new(&addr).await;
    println!("Server started.\n");

    println!("Waiting on Tubes...");
    while let Some(server_event) = server.next().await {
      match server_event {
        Ok(mut tube) => {
          println!("Tube has arrived! Spawning handler.");
          tokio::spawn(async move {
            while let Some(tube_event) = tube.next().await {
              println!("TubeEvent: {:?}", tube_event);
            }
            println!("No more tube events!");
          });
        },

        Err(e) => {
          println!("Server error: {:?}", e);
        },
      }
    }
}
