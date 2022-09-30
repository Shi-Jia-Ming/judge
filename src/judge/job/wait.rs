use thiserror::Error;
use tokio::task::JoinError;

#[cfg(target_family = "unix")]
#[derive(Debug)]
/// The Wait struct
///
/// The struct hold the exit status and resource usage of a sub-process
///
/// Used for determine the exit type of the program and get resource usage
pub struct Wait {
  rusage: Rusage,
  code: i32,
}

pub enum WaitStatus {
  Ok,
  TimeLimitExceed,
  MemoryLimitExceed,
  RuntimeError,
}

#[derive(Error, Debug)]
pub enum WaitError {
  #[error("tokio::task::spawn_blocking error: `{0}`")]
  JoinError(#[from] JoinError),
  #[error("libc::wait4 failed to get child usage")]
  Wait4Failed,
}

#[cfg(target_family = "unix")]
impl Wait {
  pub async fn wait(pid: i32) -> Result<Self, WaitError> {
    // This probably should be unsafe!
    let mut status: i32 = 0;
    let mut rusage = std::mem::MaybeUninit::uninit();

    let code = tokio::task::spawn_blocking(move || unsafe {
      libc::wait4(pid, &mut status, 0, rusage.as_mut_ptr())
    })
    .await?;

    if code < 0 {
      Err(WaitError::Wait4Failed)
    } else {
      let rusage = unsafe { rusage.assume_init() };
      Ok(Self {
        rusage: Rusage::from(rusage),
        code: status,
      })
    }
  }

  pub async fn wait_all() -> Result<Self, WaitError> {
    Self::wait(-1).await
  }
}

#[cfg(target_family = "unix")]
impl Wait {
  pub fn is_signal(&self) -> bool {
    libc::WIFSIGNALED(self.code)
  }

  pub fn signal_code(&self) -> Option<i32> {
    if self.is_signal() {
      Some(libc::WTERMSIG(self.code))
    } else {
      None
    }
  }

  pub fn is_exited(&self) -> bool {
    libc::WIFEXITED(self.code)
  }

  pub fn exit_code(&self) -> Option<i32> {
    match self.is_exited() {
      true => Some(libc::WEXITSTATUS(self.code)),
      false => None,
    }
  }

  pub fn is_ok(&self) -> bool {
    self.is_exited() && self.exit_code().unwrap() == 0
  }

  pub fn exit_type(&self) -> WaitStatus {
    if self.is_ok() {
      WaitStatus::Ok
    } else if self.is_signal() {
      match self.signal_code().unwrap() {
        libc::SIGXCPU => WaitStatus::TimeLimitExceed,
        libc::SIGKILL => WaitStatus::RuntimeError,
        libc::SIGSEGV => WaitStatus::RuntimeError,
        libc::SIGXFSZ => WaitStatus::MemoryLimitExceed,
        _ => WaitStatus::RuntimeError,
      }
    } else {
      WaitStatus::RuntimeError
    }
  }

  pub fn cputime(&self) -> i64 {
    self.rusage.cputime()
  }

  pub fn memory(&self) -> i64 {
    self.rusage.memory()
  }
}

#[cfg(target_family = "unix")]
#[derive(Debug)]
pub struct Rusage(libc::rusage);

#[cfg(target_family = "unix")]
impl Rusage {
  /// get cpu time in microseconds
  pub fn cputime(&self) -> i64 {
    self.0.ru_stime.tv_sec * 1000 + self.0.ru_stime.tv_usec / 1000
  }

  /// get max memory usage in bytes
  pub fn memory(&self) -> i64 {
    self.0.ru_maxrss * 1024
  }
}

impl From<libc::rusage> for Rusage {
  fn from(a: libc::rusage) -> Self {
    Rusage(a)
  }
}
