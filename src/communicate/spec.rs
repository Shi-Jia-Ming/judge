//! 配置文件的解析
//!
//! 目前仅支持 default 模式的评测配置

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum TaskSpec {
  Default {
    #[serde(flatten)]
    common: CommonTaskSpec,
  },
  Interactive {
    #[serde(flatten)]
    common: CommonTaskSpec,
    interactive: String,
  },
  Dynamic {
    #[serde(flatten)]
    common: CommonTaskSpec,
    mkdata: String,
    std: String,
  },
  SubmitAnswer {
    answer: String,
  },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommonTaskSpec {
  pub time: Option<u32>,
  pub memory: Option<u64>,
  pub subtasks: Vec<SubtaskSpec>,
  #[serde(flatten)]
  pub checker: Option<CheckerSpec>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CheckerSpec {
  /// SPJ 类型，目前仅支持 testlib
  #[serde(rename = "checkerType")]
  pub checker_type: String,
  /// SPJ 文件名称
  #[serde(rename = "checkerName")]
  pub checker_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubtaskSpec {
  pub score: u32,
  pub time: Option<u32>,
  pub memory: Option<u64>,
  pub cases: Vec<DefaultTaskSpec>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DefaultTaskSpec {
  pub input: String,
  pub output: String,
  pub time: Option<u32>,
  pub memory: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DynamicTaskSpec {
  pub args: Vec<String>,
  pub time: Option<u32>,
  pub memory: Option<u64>,
}
