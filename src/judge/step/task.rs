use log::debug;
use std::{collections::HashMap, path::PathBuf};
use thiserror::Error;

use async_trait::async_trait;

use crate::{
  builder,
  communicate::{
    result::{TaskResult, TaskStatus},
    spec::DefaultTaskSpec,
  },
  judge::{
    checker::Checker,
    job::{executable::Executable, limit::rlimit::RLimit, status::ExitType, Job},
    tmpdir::TmpDir,
    utils::{Memory, Time},
  },
};

use super::{Handle, HandleContext};

/// things required to run a task
pub struct TaskRunnerContext<'a> {
  /// stores all the files
  files: &'a HashMap<String, PathBuf>,
  /// tmpdir to put stdout file
  tmpdir: &'a TmpDir,
  /// checker to check stdout and answer
  checker: &'a Checker,
  /// executable file to generate job
  executable: &'a Executable,
}

#[derive(Default)]
pub struct TaskRunnerContextBuilder<'a> {
  files: Option<&'a HashMap<String, PathBuf>>,
  tmpdir: Option<&'a TmpDir>,
  checker: Option<&'a Checker>,
  executable: Option<&'a Executable>,
}

impl<'a> TaskRunnerContextBuilder<'a> {
  builder!(files, &'a HashMap<String, PathBuf>);
  builder!(tmpdir, &'a TmpDir);
  builder!(executable, &'a Executable);
  builder!(checker, &'a Checker);

  pub fn build(self) -> TaskRunnerContext<'a> {
    TaskRunnerContext {
      files: self.files.unwrap(),
      tmpdir: self.tmpdir.unwrap(),
      executable: self.executable.unwrap(),
      checker: self.checker.unwrap(),
    }
  }
}

#[derive(Debug, Error)]
pub enum TaskError {
  #[error("file {0} not found")]
  FileNotFound(String),
}

pub struct TaskHandler<'a> {
  task: DefaultTaskSpec,
  runner: &'a TaskRunnerContext<'a>,
  time_limit: Option<Time>,
  memory_limit: Option<Memory>,
}

impl<'a> TaskHandler<'a> {
  pub fn new(task: DefaultTaskSpec, runner: &'a TaskRunnerContext) -> Self {
    Self {
      task,
      runner,
      time_limit: None,
      memory_limit: None,
    }
  }

  builder!(time_limit, Time);
  builder!(memory_limit, Memory);
}

#[async_trait]
impl Handle<TaskResult> for TaskHandler<'_> {
  async fn handle(self, _context: &HandleContext) -> anyhow::Result<TaskResult> {
    debug!("Running default input/output task");

    // prepare input file
    let input = self
      .runner
      .files
      .get(&self.task.input)
      .ok_or_else(|| TaskError::FileNotFound(self.task.input.clone()))?;

    // prepare asnwer file
    let answer = self
      .runner
      .files
      .get(&self.task.output)
      .ok_or_else(|| TaskError::FileNotFound(self.task.output.clone()))?;

    // prepare output file
    let output = &self.runner.tmpdir.random_file();

    let time_limit = match self.task.time {
      Some(x) => Time::from_microseconds(x),
      None => self.time_limit.unwrap(),
    };
    let memory_limit = match self.task.memory {
      Some(x) => Memory::from_bytes(x),
      None => self.memory_limit.unwrap(),
    };

    // run executable
    let wait = Job::from_executable(&self.runner.executable)
      .stdin(input.clone())
      .stdout(output.clone())
      // FIXME enable cgroup limit
      // .cgroup(CgroupLimit::new(Memory::from_bytes(memory_limit)))
      .rlimit(RLimit::new(time_limit, memory_limit))
      .status()
      .await?;

    // get task status and message
    let (status, message) = match wait.exit_type() {
      ExitType::TimeLimitExceed => (TaskStatus::TimeLimitExceeded, String::new()),
      ExitType::MemoryLimitExceed => (TaskStatus::MemoryLimitExceeded, String::new()),
      ExitType::RuntimeError => (
        TaskStatus::RuntimeError,
        format!(
          "signal {:?}, exit code {:?}",
          wait.signal_code(),
          wait.exit_code()
        ),
      ),
      ExitType::Ok => {
        // read answer file from output
        let result = self.runner.checker.check_file(output, answer).await?;
        (
          match result.r#match {
            true => TaskStatus::Accepted,
            false => TaskStatus::WrongAnswer,
          },
          result.message,
        )
      }
    };

    Ok(TaskResult {
      status,
      message,
      time: Some(wait.cputime()),
      memory: Some(wait.memory()),
    })
  }
}
