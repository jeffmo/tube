use futures::StreamExt;

use tubez_client;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    println!("Creating client...");
    let client = tubez_client::Client::new().await;
}
