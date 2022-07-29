use std::collections::HashMap;
use std::time::Duration;

use clap::Parser;
use simple_logger::SimpleLogger;

use futures::StreamExt;

fn uri_cli_parser(s: &str) -> Result<hyper::Uri, String> {
    let uri: hyper::Uri = match s.parse() {
        Ok(uri) => uri,
        Err(e) => return Err(format!("Error parsing URI: {:?}", e)),
    };

    let mut uri_parts = uri.into_parts();
    if let None = uri_parts.scheme {
        uri_parts.scheme = Some(hyper::http::uri::Scheme::HTTPS);
    }
    if let None = uri_parts.path_and_query {
        uri_parts.path_and_query = 
          Some(hyper::http::uri::PathAndQuery::from_static("/"));
    }

    let uri = hyper::Uri::from_parts(uri_parts).unwrap();
    Ok(uri.into())
}

#[derive(Parser)]
struct CLIArgs {
    #[clap(value_parser = uri_cli_parser)]
    server_uri: hyper::Uri,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    SimpleLogger::new()
      .init()
      .expect("Error initializing logger");

    let cli_args = CLIArgs::parse();

    println!("Creating client...");
    let mut client = tube::Client::new(cli_args.server_uri);
    println!("Creating channel...");
    let channel_headers = HashMap::new();
    let mut channel = client.make_tube_channel(channel_headers).await.expect(
        "Channel creation error"
    );
    println!("Channel created!");

    println!("Waiting 3secs");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Creating tube1...");
    let tube1_headers = HashMap::new();
    let mut tube1 = channel.make_tube(tube1_headers).await.expect(
      "Tube1 creation error"
    );
    println!("tube1 created!");

    println!("Waiting 3secs");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Sending some data...");
    tube1.send("tube1 data!".into(), Duration::from_secs(3)).await.unwrap();
    println!("received ack for data sent on tube1!");
    println!("client has finished...");
    tube1.has_finished_sending().await.expect("Tube1 failed sending ClientHasFinished");

    println!("Waiting 3 secs before creating 2nd tube...");
    // TODO: Deleting this kills the transport... Probably need to gracefully 
    //       kill/end/await all the Channels in a destructor or something?
    tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;

    let tube2_headers = HashMap::new();
    let tube2 = channel.make_tube(tube2_headers).await.expect(
        "Tube2 creation error"
    );

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
