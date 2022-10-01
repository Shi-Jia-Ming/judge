use async_trait::async_trait;
use log::debug;
use std::{collections::HashMap, path::PathBuf};
use thiserror::Error;

use crate::{
  communicate::{
    message::{Progress, SendMessage},
    result::{JudgeResult, JudgeStatus, SubtaskResult, SubtaskStatus, TaskResult, TaskStatus},
    spec::TaskSpec,
  },
  judge::{
    checker::Checker,
    job::{limit::rlimit::RLimit, wait::WaitStatus, Job},
    step::compile::CompileStep,
    tmpdir::TmpDir,
    utils::{Memory, Time},
  },
  judge_error,
};

use super::{Handle, HandleContext};

pub struct TaskStep {
  task: TaskSpec,
  files: HashMap<String, PathBuf>,
}

impl TaskStep {
  pub fn new(task: TaskSpec, files: HashMap<String, PathBuf>) -> Self {
    Self { task, files }
  }
}

#[derive(Debug, Error)]
pub enum TaskError {
  #[error("file {0} not found")]
  FileNotFound(String),
}

#[async_trait]
impl Handle<JudgeResult> for TaskStep {
  async fn handle(self, context: &HandleContext) -> anyhow::Result<JudgeResult> {
    /// Send progress package to web
    macro_rules! progress {
      ($result:expr) => {
        context
          .sender
          .send(SendMessage::Progress(Progress {
            id: context.request.id,
            result: $result,
          }))
          .await
          .unwrap()
      };
    }

    match self.task {
      // default task
      TaskSpec::Default { common } => {
        debug!("Start judging default task...");
        let mut result = JudgeResult::from_status(JudgeStatus::Compiling);
        progress!(result.clone());

        // compile step
        let compile = CompileStep::new(
          context.request.code.clone().into_bytes(),
          context.request.language.clone(),
        )
        .compile()
        .await;
        let compile = match compile {
          Ok(compile) => compile,
          Err(error) => judge_error!(JudgeStatus::CompileError, "{error}"),
        };
        result.message += format!("{}\n{}", compile.stdout, compile.stderr).trim();

        debug!("Preparing checker...");
        let checker = Checker::new(&common.checker).await?;

        debug!("Start judging...");
        result.status = JudgeStatus::Running;
        progress!(result.clone());

        let time_limit = common.time.unwrap_or(1000);
        let memory_limit = common.memory.unwrap_or(128 * 1024 * 1024);
        let tmpdir = TmpDir::new().await?;

        // TODO: run subtask parallel
        for subtask in common.subtasks.iter() {
          result.subtasks.push(SubtaskResult::from_spec(&subtask));
          let subtask_result = result.subtasks.last_mut().unwrap();
          let time_limit = subtask.time.unwrap_or(time_limit);
          let memory_limit = subtask.memory.unwrap_or(memory_limit);

          for task in subtask.cases.iter() {
            subtask_result
              .tasks
              .push(TaskResult::from_status(TaskStatus::Running));
            let task_result = subtask_result.tasks.last_mut().unwrap();
            let time_limit = task.time.unwrap_or(time_limit);
            let memory_limit = task.memory.unwrap_or(memory_limit);

            // prepare input/output files
            let input = self
              .files
              .get(&task.input)
              .ok_or_else(|| TaskError::FileNotFound(task.input.clone()))?;
            let answer = self
              .files
              .get(&task.output)
              .ok_or_else(|| TaskError::FileNotFound(task.output.clone()))?;
            let output = &tmpdir.random_file();

            // run executable
            let wait = Job::from_executable(&compile.executable)
              .stdin(input.clone())
              .stdout(output.clone())
              // FIXME enable cgroup limit
              // .cgroup(CgroupLimit::new(Memory::from_bytes(memory_limit)))
              .rlimit(RLimit::new(
                Time::from_microseconds(time_limit),
                Memory::from_bytes(memory_limit),
              ))
              .status()
              .await?;

            // get task status and message
            let (task_status, message) = match wait.exit_type() {
              WaitStatus::TimeLimitExceed => (TaskStatus::TimeLimitExceeded, String::new()),
              WaitStatus::MemoryLimitExceed => (TaskStatus::MemoryLimitExceeded, String::new()),
              WaitStatus::RuntimeError => (TaskStatus::RuntimeError, String::new()),
              WaitStatus::Ok => {
                // read answer file from output
                let result = checker.check_file(output, answer).await?;
                (
                  match result.r#match {
                    true => TaskStatus::Accepted,
                    false => TaskStatus::WrongAnswer,
                  },
                  result.message,
                )
              }
            };

            task_result.status = task_status;
            task_result.message = message;
            task_result.time = Some(wait.cputime());
            task_result.memory = Some(wait.memory());

            // skip remain test cases if not accepted
            match task_result.status {
              TaskStatus::Accepted => continue,
              _ => break,
            }
          }

          subtask_result.status = subtask_result
            .tasks
            .last()
            .and_then(|task| match task.status {
              TaskStatus::Pending => None,
              TaskStatus::Running => None,
              TaskStatus::Skipped => None,
              TaskStatus::Accepted => Some(SubtaskStatus::Accepted),
              TaskStatus::WrongAnswer => Some(SubtaskStatus::WrongAnswer),
              TaskStatus::TimeLimitExceeded => Some(SubtaskStatus::TimeLimitExceeded),
              TaskStatus::MemoryLimitExceeded => Some(SubtaskStatus::MemoryLimitExceeded),
              TaskStatus::RuntimeError => Some(SubtaskStatus::RuntimeError),
              TaskStatus::SystemError => Some(SubtaskStatus::SystemError),
            })
            .unwrap_or(SubtaskStatus::SystemError);
          subtask_result.message = format!(
            "{}/{} passed",
            subtask_result
              .tasks
              .iter()
              .filter(|task| match task.status {
                TaskStatus::Accepted => true,
                _ => false,
              })
              .count(),
            subtask.cases.len()
          );

          // calculate result score according to subtask status
          result.score += match subtask_result.status {
            SubtaskStatus::Accepted => subtask_result.score,
            _ => 0,
          };

          progress!(result.clone());
        }

        // calculate judge status according to subtask status
        result.status = JudgeStatus::Accepted;
        for subtask in result.subtasks.iter() {
          let status = match subtask.status {
            SubtaskStatus::Pending => Some(JudgeStatus::SystemError),
            SubtaskStatus::Running => Some(JudgeStatus::SystemError),
            SubtaskStatus::Accepted => None,
            SubtaskStatus::WrongAnswer => Some(JudgeStatus::WrongAnswer),
            SubtaskStatus::TimeLimitExceeded => Some(JudgeStatus::TimeLimitExceeded),
            SubtaskStatus::MemoryLimitExceeded => Some(JudgeStatus::MemoryLimitExceeded),
            SubtaskStatus::RuntimeError => Some(JudgeStatus::RuntimeError),
            SubtaskStatus::SystemError => Some(JudgeStatus::SystemError),
          };

          if let Some(status) = status {
            result.status = status;
            break;
          }
        }

        Ok(result)
      }

      TaskSpec::Interactive { .. } => todo!(),
      TaskSpec::Dynamic { .. } => todo!(),

      // Submit answer problem, we ignore the language
      TaskSpec::SubmitAnswer { answer } => {
        debug!("Start judging submit_answer task...");
        let mut result = JudgeResult::from_status(JudgeStatus::WrongAnswer);

        // TODO multi-line judge
        if context.request.code.trim() == answer.trim() {
          result.status = JudgeStatus::Accepted;
          result.score = 100;
        }

        Ok(result)
      }
    }
  }
}
