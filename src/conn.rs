// Connection between judger and backend.

use crate::config::GlobalConfig;
use crate::file::{self, *};
use crate::job::{Job, JobRunner, LocalRunnerClient};
use crate::util::{Logable, ReturnChannel, ReturnChannelSend};
use crate::wire;
use crate::wire::Message as WireMsg;
use crate::wire::Task;
use crate::wire::{
    Hello, JobGenerated, RecvMessage, SendMessage, SyncRequest, SyncResponse, TaskSpec,
};
use async_trait::async_trait;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{future, SinkExt, StreamExt, TryStreamExt};
use log::{error, info, trace, warn};
use std::borrow::Borrow;
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::join;
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::{
    accept_async, connect_async, tungstenite::protocol::Message, WebSocketStream,
};

#[derive(Debug, Error)]
pub enum ConnError {
    #[error("Function not supported.")]
    NotSupported,
}

/// Note: Callers are to reorder the messages on send/recv calls.
#[async_trait]
pub trait JudgeClient {
    #[allow(unused_variables)]
    async fn raw_send(&self, msg: &[u8]) -> Result<(), Box<dyn Error>> {
        Err(Box::new(ConnError::NotSupported))
    }

    async fn raw_recv(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        Err(Box::new(ConnError::NotSupported))
    }

    async fn send(&self, msg: &SendMessage) -> Result<(), Box<dyn Error>>;

    /// Send a message,and receives a matching reply.
    /// If nothing
    async fn send_then_recv(&self, msg: &SendMessage) -> Result<RecvMessage, Box<dyn Error>>;
    async fn recv(&self) -> Result<RecvMessage, Box<dyn Error>>;

    async fn hello(
        &self,
        cpus: i32,
        langs: &[&str],
        features: &[&str],
    ) -> Result<(), Box<dyn Error>>;
    async fn get_file(&self, uuid: &str) -> Result<Vec<u8>, Box<dyn Error>>;
    async fn send_status(&self) -> Result<(), Box<dyn Error>>;
    async fn send_progress(&self) -> Result<(), Box<dyn Error>>;
    async fn finish(&self) -> Result<(), Box<dyn Error>>;
    async fn pong(&self) -> Result<(), Box<dyn Error>>;
    async fn accept(&self, id: i32) -> Result<(), Box<dyn Error>>;
    async fn reject(&self, id: i32) -> Result<(), Box<dyn Error>>;

    async fn get_file_path(&self, uuid: &str) -> Result<PathBuf, Box<dyn Error>>;
}

/// WsClient - Connect to main website (TODO)
pub struct WsClient {}

/// WsServer - Accept *one* WebSocket connection.
pub struct WsServer {
    cmd_push: Option<mpsc::Sender<SendMessage>>,
    response_queue: Option<mpsc::Receiver<RecvMessage>>,
}

pub struct WsServerRunner {
    listener: Option<TcpListener>,
    //connection : Option<WebSocketStream<TcpStream>>,
    tx: Option<SplitSink<WebSocketStream<TcpStream>, Message>>,
    rx: Option<SplitStream<WebSocketStream<TcpStream>>>,
    command_queue: Option<mpsc::Receiver<SendMessage>>,
    command_push: Option<mpsc::Sender<SendMessage>>,
    response_queue: Option<mpsc::Sender<RecvMessage>>,
    job_push: Option<mpsc::Sender<(i32, Vec<Job>)>>,
}

pub struct WsServerRecvRunner {
    listener: Option<TcpListener>,
    rx: Option<SplitStream<WebSocketStream<TcpStream>>>,
    command_push: Option<mpsc::Sender<SendMessage>>,
    job_push: Option<mpsc::Sender<JobGenerated>>,
    response_queue: Option<mpsc::Sender<RecvMessage>>,
    syncer_queue: ReturnChannelSend<String, FileSyncStatus>,
    syncer_push: mpsc::Sender<SyncResponse>,
}

impl WsServerRecvRunner {
    pub async fn recv_loop(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Receiver func
        let rx = self.rx.as_mut().unwrap();
        while let Some(Ok(msg)) = rx.next().await {
            match msg {
                Message::Text(k) => {
                    let q = serde_json::from_str::<RecvMessage>(k.as_ref());
                    match q {
                        Ok(k) => {
                            info!("msg : {:?}", k);
                            match k {
                                RecvMessage::Ping => {
                                    //self.command_queue.as_mut().unwrap().recv();
                                    match self
                                        .command_push
                                        .as_mut()
                                        .unwrap()
                                        .send(SendMessage::Pong)
                                        .await
                                    {
                                        Ok(_) => {}
                                        Err(e) => {
                                            error!("error pushing queue: {}", e);
                                        }
                                    }
                                }
                                RecvMessage::Task(k) => {
                                    let queue = self.command_push.as_ref().unwrap().clone();
                                    let mut sync_queue = self.syncer_queue.clone();
                                    let job_push = self.job_push.clone().unwrap();
                                    let k_clone = k.clone();
                                    tokio::spawn(async move {
                                        //Load all files.
                                        for (_filename, file_uuid) in k.files.iter() {
                                            //Put file into Map.
                                            let sync_result = sync_queue
                                                .send_and_wait(file_uuid.clone())
                                                .await
                                                .unwrap();
                                            //Wait for sync ok.
                                            if !matches!(sync_result, FileSyncStatus::Ok) {
                                                error!(
                                                    "syncer: Failed to sync file `{}`",
                                                    file_uuid
                                                );
                                                return;
                                            }
                                        }

                                        //if let Some(config_uuid) = k.files.get("config.json")
                                        let file_id = k
                                            .files
                                            .get("config.json")
                                            .expect("No config.json provided!");
                                        //Read config file
                                        let config_file = load_data(file_id).await.unwrap();
                                        let config: TaskSpec =
                                            serde_json::from_str(config_file.as_ref()).unwrap();
                                        let jobs = config.task_to_job_spec(&k).await.unwrap();

                                        //Push job to job runner.
                                        let send_msg = match job_push.try_send(jobs) {
                                            Ok(_) => SendMessage::Accept { id: k.id },
                                            Err(_) => SendMessage::Reject { id: k.id },
                                        };
                                        queue.send(send_msg).await.log();
                                    });
                                }
                                RecvMessage::Sync(k) => {
                                    self.syncer_push.send(k).await.log();
                                }
                                _ => {
                                    warn!("recv: Message is invalid.");
                                }
                            }
                        }
                        Err(e) => {
                            error!("{}", e);
                            continue;
                        }
                    }
                }
                Message::Binary(_) => {}
                Message::Close(_) => {}
                Message::Ping(_) => {}
                Message::Pong(_) => {}
                _ => {}
            }
        }
        Ok(())
    }
}

pub struct WsServerSendRunner {
    tx: Option<SplitSink<WebSocketStream<TcpStream>, Message>>,
    command_queue: Option<mpsc::Receiver<SendMessage>>,
}

impl WsServerSendRunner {
    pub async fn send_loop(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = self.tx.as_mut().unwrap();
        let queue = self.command_queue.as_mut().unwrap();
        let hello = SendMessage::Hello(Hello {
            version: "v0".to_owned(),
            cpus: num_cpus::get() as i32,
            langs: vec!["c".to_owned(), "cpp".to_owned()],
            ext_features: vec![],
        });
        tx.send(Message::Text(serde_json::to_string(&hello).unwrap()))
            .await
            .unwrap();
        while let Some(msg) = queue.recv().await {
            match serde_json::to_string(&msg) {
                Ok(msg) => match tx.send(Message::Text(msg)).await {
                    Ok(_) => continue,
                    Err(WsError::ConnectionClosed) => {
                        info!("send: Connection closed.");
                        break;
                    }
                    Err(k) => {
                        error!("send: Error in sending: {}", k);
                    }
                },
                Err(err) => {
                    warn!("send: error to receive from queue,info {}", err);
                }
            }
        }
        info!("send: connection closed");
        //Send out message.
        Ok(())
    }
}

impl WsServer {
    pub async fn listen_wait_one(
        addr: &str,
    ) -> Result<
        (
            WsServerRecvRunner,
            WsServerSendRunner,
            LocalRunnerClient,
            Syncer,
        ),
        Box<dyn Error>,
    > {
        let listener = TcpListener::bind(&addr).await?;
        let (stream, _) = listener.accept().await?;
        let ws = accept_async(stream).await?;
        let (tx, rx) = ws.split();
        let (cmd_tx, cmd_rx) = mpsc::channel::<SendMessage>(64 as usize);
        let cmd_tx_recv = cmd_tx.clone();
        let cmd_tx_job_runner = cmd_tx.clone();
        let cmd_tx_syncer = cmd_tx.clone();
        let (job_tx, job_rx) = mpsc::channel::<JobGenerated>(64 as usize);
        let (sync_tx, sync_rx) = mpsc::channel(128 as usize);
        let (sync_recv_tx, sync_recv_rx) = mpsc::channel(128_usize);
        Ok((
            WsServerRecvRunner {
                listener: Some(listener),
                command_push: Some(cmd_tx_recv),
                rx: Some(rx),
                response_queue: None,
                job_push: Some(job_tx),
                syncer_queue: sync_tx,
                syncer_push: sync_recv_tx,
            },
            WsServerSendRunner {
                command_queue: Some(cmd_rx),
                tx: Some(tx),
            },
            LocalRunnerClient::new(job_rx, cmd_tx_job_runner, crate::job::LocalRunner {}),
            Syncer::new(sync_rx, sync_recv_rx, cmd_tx_syncer),
        ))
    }
}

#[async_trait]
impl JudgeClient for WsServer {
    async fn send(&self, msg: &SendMessage) -> Result<(), Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn send_then_recv(&self, msg: &SendMessage) -> Result<RecvMessage, Box<dyn Error>> {
        self.send(msg).await?;
        //Read a message and REORDER INTO queue.
        Err(ConnError::NotSupported.into())
    }

    async fn recv(&self) -> Result<RecvMessage, Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn hello(
        &self,
        cpus: i32,
        langs: &[&str],
        features: &[&str],
    ) -> Result<(), Box<dyn Error>> {
        self.send(&SendMessage::Hello(wire::Hello {
            version: "v1".to_owned(),
            langs: vec!["c".to_owned(), "cpp".to_owned(), "py".to_owned()],
            cpus: 16,
            ext_features: vec![],
        }))
        .await?;
        Ok(())
    }

    async fn get_file(&self, uuid: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn send_status(&self) -> Result<(), Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn send_progress(&self) -> Result<(), Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn finish(&self) -> Result<(), Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn pong(&self) -> Result<(), Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn accept(&self, id: i32) -> Result<(), Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn reject(&self, id: i32) -> Result<(), Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }

    async fn get_file_path(&self, uuid: &str) -> Result<PathBuf, Box<dyn Error>> {
        Err(ConnError::NotSupported.into())
    }
}
