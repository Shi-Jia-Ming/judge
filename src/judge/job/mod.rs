pub mod executable;
pub mod limit;
pub mod wait;

use log::debug;
use std::{path::PathBuf, process::Output};
use thiserror::Error;
use tokio::{fs::File, process::Command};

use crate::builder;

use self::{
  executable::Executable,
  limit::{cgroup::CgroupLimit, rlimit::RLimit, Limit, LimitError},
  wait::Wait,
};

/// Run the program with limitations
pub struct Job {
  command: Command,

  stdin: Option<PathBuf>,
  stdout: Option<PathBuf>,
  stderr: Option<PathBuf>,

  rlimit: Option<RLimit>,
  cgroup: Option<CgroupLimit>,
}

#[derive(Debug, Error)]
pub enum JobError {
  #[error("failed to execute command: {0}")]
  IoError(#[from] std::io::Error),
  #[error("wait failed: {0}")]
  WaitError(#[from] self::wait::WaitError),
  #[error("failed to apply limit to job: {0}")]
  LimitError(#[from] LimitError),
}

impl Job {
  /// create a new job without and limitations
  pub fn from_unlimited(command: Command) -> Self {
    Self {
      command,
      stdin: None,
      stdout: None,
      stderr: None,
      rlimit: None,
      cgroup: None,
    }
  }

  /// create a new job with executable file
  pub fn from_executable(executable: &Executable) -> Self {
    let command = match executable {
      Executable::Elf(path) => Command::new(path),
      Executable::Command { name, args } => {
        let mut command = Command::new(name);
        command.args(args);
        command
      }
    };
    Self::from_unlimited(command)
  }

  // builders

  builder!(stdin, PathBuf);
  builder!(stdout, PathBuf);
  builder!(stderr, PathBuf);

  builder!(rlimit, RLimit);
  builder!(cgroup, CgroupLimit);
}

impl Job {
  /// Apply I/O source and limits to Command
  async fn command(&mut self) -> Result<&mut Command, JobError> {
    // apply limits
    if let Some(rlimit) = &self.rlimit {
      rlimit.apply_to(&mut self.command)?;
    }
    if let Some(cgroup) = &self.cgroup {
      cgroup.apply_to(&mut self.command)?;
    }

    // apply io
    if let Some(stdin) = &self.stdin {
      let stdin = File::open(stdin).await?.into_std().await;
      self.command.stdin(stdin);
    }
    if let Some(stdout) = &self.stdout {
      let stdout = File::create(stdout).await?.into_std().await;
      self.command.stdout(stdout);
    }
    if let Some(stderr) = &self.stderr {
      let stderr = File::create(stderr).await?.into_std().await;
      self.command.stderr(stderr);
    }

    Ok(&mut self.command)
  }

  /// Execute the job and get resource usage and exit_type
  pub async fn status(mut self) -> Result<Wait, JobError> {
    debug!("applying limits and io...");
    let command = self.command().await?;
    debug!("start spawn command...");
    let child = command.spawn()?;
    debug!("start wait child process...");
    let wait = Wait::wait(child.id().expect("failed to get child pid") as i32).await?;
    debug!("child process finished, return status {wait:?}");
    Ok(wait)
  }

  /// Execute the job and get stdout and stderr
  pub async fn output(mut self) -> Result<Output, JobError> {
    debug!("applying limits and io...");
    let command = self.command().await?;
    debug!("start run program...");
    let output = command.output().await?;
    debug!("job finished");
    Ok(output)
  }
}
