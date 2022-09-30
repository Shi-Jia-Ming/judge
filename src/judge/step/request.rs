use std::{collections::HashMap, sync::Arc};

use crate::{
  communicate::{
    message::{Progress, SendMessage, TaskRequest},
    result::{JudgeResult, JudgeStatus, SubtaskResult, SubtaskStatus, TaskResult, TaskStatus},
    spec::TaskSpec,
  },
  judge::{
    cache::CacheDir,
    checker::Checker,
    job::{
      limit::{cgroup::CgroupLimit, rlimit::RLimit},
      wait::WaitStatus,
      Job,
    },
    step::compile::{Compile, CompileResult, Compiler},
    tmpdir::TmpDir,
    JudgeError,
  },
};
use log::debug;

use tokio::{
  fs::File,
  sync::{mpsc, Mutex},
};

pub async fn handle_request(
  request: TaskRequest,
  sender: &mpsc::Sender<SendMessage>,
  cache: &Arc<Mutex<CacheDir>>,
) -> Result<JudgeResult, JudgeError> {
  debug!("Working on task {}", request.id);

  debug!("Start sync files...");
  let mut filemap = HashMap::new();
  for (filename, uuid) in request.files.into_iter() {
    // drop cache.lock() before await
    let recv = {
      let mut cache = cache.lock().await;
      cache.sync(&uuid).await
    };
    let filepath = recv.await?;
    filemap.insert(filename, filepath);
  }

  debug!("Parsing config.json...");
  let config = filemap
    .get("config.json")
    .ok_or(JudgeError::ConfigNotFound)?;
  let config = tokio::fs::read(config).await?;
  let config = serde_json::from_str::<TaskSpec>(&String::from_utf8_lossy(config.as_slice()))?;

  // Send progress package to web
  macro_rules! progress {
    ($result:expr) => {
      sender
        .send(SendMessage::Progress(Progress {
          id: request.id,
          result: $result,
        }))
        .await
        .unwrap()
    };
  }

  match config {
    // default task
    TaskSpec::Default { common } => {
      debug!("Start judging default task...");
      let mut result = JudgeResult::from_status(JudgeStatus::Compiling);
      progress!(result.clone());

      // compile step
      let compile_dirname = TmpDir::new().await?;
      let compile: anyhow::Result<(CompileResult, Checker)> = try {
        debug!("Compiling source code...");
        let compile = Compiler::new(request.code.into_bytes(), request.language)
          .compile(&compile_dirname)
          .await?;

        debug!("Preparing checker...");
        let checker = Checker::new(&common.checker).await?;

        (compile, checker)
      };
      let (compile, checker) = match compile {
        Ok((compile, checker)) => (compile, checker),
        Err(error) => {
          return Ok(JudgeResult::from_status_message(
            JudgeStatus::CompileError,
            format!("{error:?}"),
          ));
        }
      };
      result.message += format!("{}\n{}", compile.stdout, compile.stderr).trim();

      debug!("Check input/output file exists...");
      for subtask in common.subtasks.iter() {
        for case in subtask.cases.iter() {
          filemap
            .get(&case.input)
            .ok_or(JudgeError::FileNotFound(case.input.clone()))?;
          filemap
            .get(&case.output)
            .ok_or(JudgeError::FileNotFound(case.output.clone()))?;
        }
      }

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
          let input = filemap.get(&task.input).unwrap();
          let answer = filemap.get(&task.output).unwrap();
          let output = &tmpdir.random_file();

          // run executable
          let wait = Job::from_executable(&compile.executable)
            .stdin(File::open(input).await?.into_std().await)
            .stdout(File::create(output).await?.into_std().await)
            .cgroup(CgroupLimit::new(memory_limit))
            .rlimit(RLimit::new(time_limit, memory_limit))
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
          task_result.time = Some(wait.cputime() as u32);
          task_result.memory = Some(wait.memory() as u64);

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
      if request.code.trim() == answer.trim() {
        result.status = JudgeStatus::Accepted;
        result.score = 100;
      }

      Ok(result)
    }
  }
}
