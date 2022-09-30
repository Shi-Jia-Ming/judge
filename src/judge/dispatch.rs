use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use log::{debug, error};
use tokio::{
  net::TcpStream,
  sync::{mpsc, Mutex},
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::{
  communicate::{
    message::{Hello, Progress, RecvMessage, SendMessage},
    result::{JudgeResult, JudgeStatus},
  },
  judge::step::{Handle, HandleContext},
};

use super::cache::CacheDir;

pub struct Dispatch {
  pub stream: WebSocketStream<TcpStream>,
  pub recv: mpsc::Receiver<SendMessage>,
  pub context: HandleContext,
}

impl Dispatch {
  pub async fn from(stream: WebSocketStream<TcpStream>) -> Self {
    let (sender, recv) = mpsc::channel::<SendMessage>(1024);
    let cache = Arc::new(Mutex::new(
      CacheDir::new(sender.clone())
        .await
        .expect("failed to create cache dir"),
    ));
    let context = HandleContext { sender, cache };
    Self {
      stream,
      context,
      recv,
    }
  }
}

impl Dispatch {
  /// Start the event loop.
  pub async fn run(&mut self) {
    let context = self.context.clone();
    tokio::spawn(async move {
      context
        .sender
        .send(SendMessage::Hello(Hello {
          version: "v0".to_string(),
          cpus: 1,
          langs: vec!["c".to_string(), "cpp".to_string(), "py".to_string()],
          ext_features: vec![],
        }))
        .await
        .expect("failed to send hello message");
    });

    macro_rules! send {
      ($message:expr) => {
        self.context.sender.send($message).await.unwrap()
      };
    }

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
                        debug!("Ping? Pong!");
                        send!(SendMessage::Pong);
                      }
                      RecvMessage::Task(request) => {
                        let id = request.id;

                        // FIXME: I cannot reject requests!
                        // sender.send(SendMessage::Reject { id }).await.unwrap();
                        // debug!("Rejected request {}: buzy", id);

                        debug!("Accepted request {}", id);
                        send!(SendMessage::Accept { id });

                        let context = self.context.clone();
                        tokio::spawn(async move {
                          let result = request.handle(&context).await;
                          debug!("task finished: {result:?}");
                          let finish = match result {
                            Ok(result) => result,
                            Err(error) => {
                              JudgeResult::from_status_message(JudgeStatus::SystemError, format!("{error}"))
                            }
                          };
                          context.sender.send(
                            SendMessage::Finish(Progress { id, result: finish })
                          ).await.unwrap();
                        });
                      }
                      RecvMessage::Sync(sync) => {
                        let context = self.context.clone();
                        tokio::spawn(async move {
                          let contents = base64::decode(sync.data).unwrap();
                          context.cache.lock().await.save(&sync.uuid, &contents).await.expect("failed to save sync file");
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
                error!("{err}");
                break;
              }
            },
            None => break,
          }
        }
        message = self.recv.recv() => {
          if let Some(message) = message {
            debug!("Send: {message:?}");
            let message = serde_json::to_string(&message).unwrap();
            let message = Message::Text(message);
            self.stream.send(message).await.expect("failed to send message")
          }
        }
      };
    }
  }
}
