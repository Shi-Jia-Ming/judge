use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use log::{debug, error};
use tokio::{
  net::TcpStream,
  sync::{mpsc, Mutex},
};
use tokio_tungstenite::{
  tungstenite::{self, Message},
  WebSocketStream,
};

use crate::{
  communicate::message::{Hello, RecvMessage, SendMessage},
  judge::step::request::handle_request,
};

use super::cache::CacheDir;

pub struct Dispatch {
  pub stream: WebSocketStream<TcpStream>,
}

impl Dispatch {
  pub async fn from(stream: WebSocketStream<TcpStream>) -> Self {
    Self { stream }
  }
}

impl Dispatch {
  /// Send message to WebSocket
  pub async fn send_message(
    &mut self,
    send_message: SendMessage,
  ) -> Result<(), tungstenite::Error> {
    debug!("Send: {:?}", send_message);
    let message = serde_json::to_string(&send_message).unwrap();
    let message = Message::Text(message);
    self.stream.send(message).await
  }

  /// Start the event loop.
  pub async fn run(&mut self) {
    let (sender, mut recver) = mpsc::channel::<SendMessage>(1024);
    let cache = Arc::new(Mutex::new(
      CacheDir::new(sender.clone())
        .await
        .expect("failed to create cache dir"),
    ));

    self
      .send_message(SendMessage::Hello(Hello {
        version: "v0".to_string(),
        cpus: 1,
        langs: vec!["c".to_string(), "cpp".to_string(), "py".to_string()],
        ext_features: vec![],
      }))
      .await
      .expect("failed to send hello message");

    loop {
      tokio::select! {
        message = self.stream.next() => {
          match message {
            Some(msg) => match msg {
              Ok(msg) if msg.is_text() || msg.is_binary() => {
                let message = msg.into_text().unwrap();
                match serde_json::from_str::<RecvMessage>(message.as_str()) {
                  Ok(recv) => {
                    debug!("Received: {:?}", recv);

                    match recv {
                      RecvMessage::Ping => {
                        sender.send(SendMessage::Pong).await.unwrap();
                        debug!("Ping? Pong!");
                      }
                      RecvMessage::Task(request) => {
                        let id = request.id;

                        // FIXME: I cannot reject requests!
                        // sender.send(SendMessage::Reject { id }).await.unwrap();
                        // debug!("Rejected request {}: buzy", id);

                        sender.send(SendMessage::Accept { id }).await.unwrap();
                        debug!("Accepted request {}", id);

                        let sender = sender.clone();
                        let cache = cache.clone();
                        tokio::spawn(async move {
                          handle_request(request, sender, cache).await.unwrap();
                        });
                      }
                      RecvMessage::Sync(sync) => {
                        let cache = cache.clone();
                        tokio::spawn(async move {
                          let contents = base64::decode(sync.data).unwrap();
                          cache.lock().await.save(&sync.uuid, &contents).await.expect("failed to save sync file");
                        });
                      }
                    }
                  },
                  Err(err) => error!("Unrecognized message: {err}"),
                }
              }
              Ok(msg) if msg.is_close() => break,
              Ok(msg) => debug!("Received unknown message: {msg}"),
              Err(err) => {
                error!("{}", err);
                break;
              }
            },
            None => break,
          }
        }
        message = recver.recv() => {
          if let Some(message) = message {
            self.send_message(message).await.expect("failed to send message");
          }
        }
      };
    }
  }
}
