use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

use super::result::JudgeResult;

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum RecvMessage {
  /// ping 消息，收到立即回复一个 pong
  Ping,
  /// 分配任务
  Task(TaskRequest),
  /// 请求同步文件的响应消息
  Sync(SyncResponse),
}

#[derive(Deserialize, Clone, Debug)]
pub struct TaskRequest {
  pub id: u32,
  pub code: String,
  pub language: String,
  pub files: HashMap<String, String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SyncResponse {
  pub uuid: String,
  pub data: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum SendMessage {
  /// pong 消息
  Pong,
  /// 握手消息
  Hello(Hello),
  /// 同步评测机状态
  Status(Status),
  /// 同步文件请求
  Sync(SyncRequest),
  /// 任务进度反馈
  Progress(Progress),
  /// 任务结束反馈
  Finish(Progress),
  /// 接受任务
  Accept { id: u32 },
  /// 拒绝任务
  Reject { id: u32 },
}

#[derive(Serialize, Clone, Debug)]
pub struct Hello {
  pub version: String,
  pub cpus: u32,
  pub langs: Vec<String>,
  #[deprecated]
  pub ext_features: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Status {
  pub cpus: u32,
  pub occupied: u32,
  pub queue: u32,
}

#[derive(Serialize, Clone, Debug)]
pub struct Progress {
  pub id: u32,
  pub result: JudgeResult,
}

#[derive(Serialize, Clone, Debug)]
pub struct SyncRequest {
  pub uuid: String,
}
