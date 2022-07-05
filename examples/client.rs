use std::collections::HashMap;

use futures::StreamExt;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    println!("Creating client...");
    let mut client = tubez::Client::new();
    println!("Creating channel...");
    let channel_headers = HashMap::new();
    let mut channel = match client.make_tube_channel(channel_headers).await {
        Ok(channel) => channel,
        Err(e) => {
            println!("channel creation error: {:?}", e);
            return
        },
    };
    println!("Channel created!");

    println!("Waiting 5secs");
    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;

    println!("Creating tube1...");
    let tube1_headers = HashMap::new();
    let mut tube1 = match channel.make_tube(tube1_headers).await {
        Ok(tube) => tube,
        Err(e) => {
            println!("Error creating tube: {:?}", e);
            return
        },
    };
    println!("tube1 created!");

    /*
    println!("Creating tube2...");
    let tube2_headers = HashMap::new();
    let mut tube2 = match channel.make_tube(tube2_headers).await {
        Ok(tube) => tube,
        Err(e) => {
            println!("Error creating tube: {:?}", e);
            return
        },
    };
    println!("tube2 created!");
    */
    println!("Waiting 5secs");
    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;

    println!("Sending some data...");
    tube1.send("tube1 data!".into()).await.unwrap();
    println!("received ack for data sent on tube1!");
    println!("client has finished...");
    tube1.has_finished_sending().await;

    println!("Waiting 3 secs before creating 2nd tube...");
    // TODO: Deleting this kills the transport... Probably need to gracefully 
    //       kill/end/await all the Channels in a destructor or something?
    tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;

    let tube2_headers = HashMap::new();
    let tube2 = match channel.make_tube(tube2_headers).await {
        Ok(tube) => tube,
        Err(e) => {
            println!("Error creating tube: {:?}", e);
            return
        },
    };

    println!("Waiting on tube events...");
    while let Some(tube_event) = tube1.next().await {
      println!("Tubeevent: {:?}", tube_event);
    }
    std::mem::drop(tube1);
    std::mem::drop(tube2);
    println!("No more tube events! Dropping channel in 3 seconds...");
    tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
    std::mem::drop(channel);
    println!("Channel now dropped!");

    println!("Waiting a bit before exiting...");
    // TODO: Deleting this kills the transport... Probably need to gracefully 
    //       kill/end/await all the Channels in a destructor or something?
    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
}
