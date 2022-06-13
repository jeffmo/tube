use hyper;

pub enum ClientConnectError {
}

pub struct Client;
impl Client {
  pub async fn new() -> Result<Self, ClientConnectError> {
    let hyper_client: hyper::Client<hyper::client::HttpConnector> = 
      hyper::Client::builder()
        .http2_only(true)
        .build_http();

    let (mut body_sender, body) = hyper::Body::channel();
    let req = hyper::Request::builder()
      .method(hyper::Method::POST)
      .uri("http://127.0.0.1:3000/")
      .body(body)
      .unwrap();

    println!("Sending request...");
    match hyper_client.request(req).await {
      Ok(response) => {
        println!("Request sent! Sending more stuff...");
        body_sender.try_send_data("test".into());
      },
      Err(e) => {
        println!("Err! {:?}", e);
      },
    };

    Ok(Client {
    })
  }
}

#[cfg(test)]
mod client_tests {
}
