use serde_derive::Serialize;

use crate::judge::utils::{Memory, Time};

use super::spec::SubtaskSpec;

/// 测试点结果状态
#[derive(Serialize, Clone, Debug)]
pub enum TaskStatus {
  /// 测试点还在排队
  Pending,
  /// 测试点正在运行
  Running,
  /// 子任务中出现第一个非 Accepted 测试点之后，剩余的所有测试点都直接跳过
  Skipped,
  /// 程序通过该测试点
  Accepted,
  /// 程序正常运行结束，但是输出的结果与标准结果不匹配，或者无法通过 SPJ 的校验
  #[serde(rename = "Wrong Answer")]
  WrongAnswer,
  /// 程序无法在规定的时间限制内运行结束
  #[serde(rename = "Time Limit Exceeded")]
  TimeLimitExceeded,
  /// 程序运行过程中超过内存限制，或者因为超过运行内存而终止
  #[serde(rename = "Memory Limit Exceeded")]
  MemoryLimitExceeded,
  /// 程序运行过程中出现其他非正常终止状况，或者程序运行结束返回值不为 0
  #[serde(rename = "Runtime Error")]
  RuntimeError,
  /// 评测遇到非预期的情况
  #[serde(rename = "System Error")]
  SystemError,
}

/// 子任务结果状态
#[derive(Serialize, Clone, Debug)]
pub enum SubtaskStatus {
  /// 子任务还在排队（没有进行任何编译或者运行）
  Pending,
  /// 子任务中的其中一个测试点正在运行
  Running,
  /// 程序通过子任务中的全部测试点
  Accepted,
  /// 第一个非 Accepted 测试点的状态为 Wrong Answer
  #[serde(rename = "Wrong Answer")]
  WrongAnswer,
  /// 第一个非 Accepted 测试点的状态为 Time Limit Exceeded
  #[serde(rename = "Time Limit Exceeded")]
  TimeLimitExceeded,
  /// 第一个非 Accepted 测试点的状态为 Memory Limit Exceeded
  #[serde(rename = "Memory Limit Exceeded")]
  MemoryLimitExceeded,
  /// 第一个非 Accepted 测试点的状态为 Runtime Error
  #[serde(rename = "Runtime Error")]
  RuntimeError,
  /// 评测遇到非预期的情况
  #[serde(rename = "System Error")]
  SystemError,
}

/// 评测结果状态
#[derive(Serialize, Debug, Clone)]
pub enum JudgeStatus {
  /// 任务还在排队
  Pending,
  /// 任务已经被成功分配给了评测机
  Judging,
  /// 任务正在编译
  Compiling,
  /// 任务正在运行
  Running,
  /// 程序编译时遇到了错误
  #[serde(rename = "Compile Error")]
  CompileError,
  /// 程序通过全部子任务
  Accepted,
  /// 程序第一个非 Accepted 子任务的结果为 Wrong Answer
  #[serde(rename = "Wrong Answer")]
  WrongAnswer,
  /// 程序第一个非 Accepted 子任务的结果为 Time Limit Exceeded
  #[serde(rename = "Time Limit Exceeded")]
  TimeLimitExceeded,
  /// 程序第一个非 Accepted 子任务的结果为 Memory Limit Exceeded
  #[serde(rename = "Memory Limit Exceeded")]
  MemoryLimitExceeded,
  /// 程序第一个非 Accepted 子任务的结果为 Runtime Error
  #[serde(rename = "Runtime Error")]
  RuntimeError,
  /// 评测遇到非预期的情况
  #[serde(rename = "System Error")]
  SystemError,
}

#[derive(Serialize, Clone, Debug)]
pub struct TaskResult {
  pub message: String,
  pub status: TaskStatus,
  pub time: Option<Time>,
  pub memory: Option<Memory>,
}

impl TaskResult {
  pub fn from_status(status: TaskStatus) -> Self {
    Self {
      message: String::new(),
      status,
      time: None,
      memory: None,
    }
  }
}

#[derive(Serialize, Clone, Debug)]
pub struct SubtaskResult {
  pub message: String,
  pub status: SubtaskStatus,
  pub score: u32,
  pub tasks: Vec<TaskResult>,
}

impl SubtaskResult {
  pub fn from_spec(spec: &SubtaskSpec) -> Self {
    Self {
      message: String::new(),
      status: SubtaskStatus::Running,
      score: spec.score,
      tasks: Vec::new(),
    }
  }
}

#[derive(Serialize, Clone, Debug)]
pub struct JudgeResult {
  pub message: String,
  pub status: JudgeStatus,
  pub score: u32,
  pub subtasks: Vec<SubtaskResult>,
}

impl JudgeResult {
  pub fn from_status(status: JudgeStatus) -> Self {
    Self {
      status,
      message: String::new(),
      score: 0,
      subtasks: Vec::new(),
    }
  }

  pub fn from_status_message(status: JudgeStatus, message: String) -> Self {
    Self {
      status,
      message,
      score: 0,
      subtasks: Vec::new(),
    }
  }
}
