use futures::StreamExt;

use tubez::server::ChannelEvent;
use tubez::server::ServerEvent;
use tubez::tube::Tube;
use tubez::tube::TubeEvent;

fn spawn_tube_handler(mut tube: Tube) {
  tokio::spawn(async move {
      let tube_id = tube.get_id();
      while let Some(tube_event) = tube.next().await {
          println!("TubeLoop: Tube({}) event: {:?}", tube_id, tube_event);
          match tube_event {
              TubeEvent::ClientHasFinishedSending => {
                  println!("TubeLoop:  responding with ServerHasFinishedSending...");
                  tube.has_finished_sending().await.unwrap();
                  println!("TubeLoop:    sent!");
              },
              _ => (),
          }
      }
      println!("TubeLoop: Tube has finished receiving data! Dropping..");
  });
}

fn spawn_channel_handler(mut channel: tubez::server::Channel) {
    tokio::spawn(async move {
        while let Some(channel_event) = channel.next().await {
            match channel_event {
                ChannelEvent::NewTube(tube) => {
                    println!("ChannelLoop: Tube({}) arrived!", tube.get_id());
                    spawn_tube_handler(tube);

                    // Only expect 1 Tube
                    break;
                }
            }
        }
        println!("ChannelLoop: Dropping channel!");
    });
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let addr = std::net::SocketAddr::from(([127,0,0,1], 3000));
    println!("Starting server...");
    let mut server = tubez::Server::new(&addr).await;
    println!("Server started.\n");

    println!("Waiting on Tubes...");
    while let Some(server_event) = server.next().await {
      match server_event {
        /*
        Ok(ServerEvent::NewTube(mut tube)) => {
          println!("Tube has arrived! Spawning handler.");
          tokio::spawn(async move {
            while let Some(tube_event) = tube.next().await {
              println!("TubeEvent: {:?}", tube_event);
              match tube_event {
                tubez::tube::TubeEvent::ClientHasFinishedSending => {
                  println!("  responding with ServerHasFinishedSending...");
                  tube.has_finished_sending().await.unwrap();
                  println!("    sent!");
                },
                _ => (),
              }
            }
            println!("No more tube events!");
          });
        },
        */

        Ok(ServerEvent::NewChannel(channel)) => {
            println!("New channel has arrived!");
            spawn_channel_handler(channel);
        },

        Err(e) => {
          println!("Server error: {:?}", e);
        },
      }
    }
}
