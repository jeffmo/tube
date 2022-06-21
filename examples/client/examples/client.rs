use futures::StreamExt;
use std::collections::HashMap;

extern crate tubez_client;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    println!("Creating client...");
    let mut client = tubez_client::Client::new();
    println!("Creating channel...");
    let channel_headers = HashMap::new();
    let channel_mutex = match client.make_tube_channel(channel_headers).await {
        Ok(channel) => channel,
        Err(e) => {
            println!("channel creation error: {:?}", e);
            return
        },
    };
    println!("Channel created! Creating tube...");
    let mut channel = channel_mutex.lock().unwrap();

    let tube1_headers = HashMap::new();
    let tube1_mutex = match channel.make_tube(tube1_headers).await {
        Ok(tube_mutex) => tube_mutex,
        Err(e) => {
            println!("Error creating tube: {:?}", e);
            return
        },
    };

    let tube2_headers = HashMap::new();
    let tube2_mutex = match channel.make_tube(tube2_headers).await {
        Ok(tube_mutex) => tube_mutex,
        Err(e) => {
            println!("Error creating tube: {:?}", e);
            return
        },
    };

    println!("Waiting a bit before 3rd tube...");
    // TODO: Deleting this kills the transport... Probably need to gracefully 
    //       kill/end/await all the Channels in a destructor or something?
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let tube3_headers = HashMap::new();
    let tube3_mutex = match channel.make_tube(tube3_headers).await {
        Ok(tube_mutex) => tube_mutex,
        Err(e) => {
            println!("Error creating tube: {:?}", e);
            return
        },
    };

    println!("Waiting a bit before exiting...");
    // TODO: Deleting this kills the transport... Probably need to gracefully 
    //       kill/end/await all the Channels in a destructor or something?
    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
}
