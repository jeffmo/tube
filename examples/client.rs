use std::collections::HashMap;

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
    println!("Channel created! Creating tube...");

    let tube1_headers = HashMap::new();
    let mut tube1 = match channel.make_tube(tube1_headers).await {
        Ok(tube) => tube,
        Err(e) => {
            println!("Error creating tube: {:?}", e);
            return
        },
    };
    println!("tube1 created! Sending some data...");
    tube1.send("tube1 data!".into()).await.unwrap();
    println!("received ack for data sent on tube1! Creating tube2...");

    println!("Waiting a bit before 2nd tube...");
    // TODO: Deleting this kills the transport... Probably need to gracefully 
    //       kill/end/await all the Channels in a destructor or something?
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let tube2_headers = HashMap::new();
    let _tube2 = match channel.make_tube(tube2_headers).await {
        Ok(tube) => tube,
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
