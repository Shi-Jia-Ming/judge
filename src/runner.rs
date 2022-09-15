use crate::wire::Message;
use crate::wire::{RecvMessage, SendMessage};
use crate::{conn, wire};
use serde_derive::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Semaphore;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct Task {
    #[serde(rename = "type")]
    task_type: String,
    answer: Option<String>,
    checkerType: String,
    interactive: Option<String>,
    mkdata: Option<String>,
    std: Option<String>,
    time: i32,
    memory: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubTask {}

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    cpus: i32,
    task_lock: Arc<Semaphore>,
    langs: Vec<String>,
    features: Vec<String>,
}

async fn connection_runner(conn: impl conn::JudgeClient) -> Result<(), Box<dyn Error>> {
    // Send hello message.
    loop {
        let msg = conn.recv().await?;
        match msg {
            RecvMessage::Ping => {
                conn.pong().await?;
            }
            RecvMessage::Task(ref k) => {}
            _ => {}
        }
    }
    Ok(())
}
