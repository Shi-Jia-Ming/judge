use judge::dispatch::Dispatch;
use log::info;
use std::error::Error;
use tokio::net::TcpListener;

mod communicate;
mod judge;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  env_logger::init();

  let addr = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "127.0.0.1:1145".to_string());

  // Create the event loop and TCP listener we'll accept connections on.
  let listener = TcpListener::bind(&addr).await?;
  info!("Listening on: {}", addr);

  while let Ok((stream, _)) = listener.accept().await {
    let addr = stream
      .peer_addr()
      .expect("connected streams should have a peer address");

    info!("New WebSocket connection: {}", addr);

    let stream = tokio_tungstenite::accept_async(stream)
      .await
      .expect("Error during the websocket handshake occurred");

    tokio::spawn(async move {
      Dispatch::from(stream).await.run().await;
      info!("Connection closed: {}", addr);
    });
  }

  Ok(())
}
