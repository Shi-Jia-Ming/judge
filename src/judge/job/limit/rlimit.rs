use rlimit::{setrlimit, Resource, INFINITY};

use crate::judge::utils::{Memory, Time};

use super::{Limit, LimitError};

/// Use `setrlimit` to limit resources
pub struct RLimit {
  /// time limit
  time: Time,
  /// memory limit
  memory: Memory,
}

impl RLimit {
  pub fn new(time: Time, memory: Memory) -> Self {
    Self { time, memory }
  }
}

impl Limit for RLimit {
  fn apply_to(&self, command: &mut tokio::process::Command) -> Result<(), LimitError> {
    let time = self.time.into_seconds() as u64;
    let memory = self.memory.into_bytes();
    unsafe {
      command.pre_exec(move || {
        // Put resource limits
        // TODO: Apply setting for different presets.
        setrlimit(Resource::NOFILE, 5000, 5000)?;
        setrlimit(Resource::CPU, time, time)?;
        setrlimit(Resource::STACK, INFINITY, INFINITY)?;
        setrlimit(Resource::CORE, 0, 0)?;
        // No core dump needed
        // TODO: Use pids.max in cgroups to achieve process limits.
        // NPROC is per-user limit,not per-process limit.
        // setrlimit(Resource::NPROC, INFINITY, INFINITY)?;
        setrlimit(Resource::DATA, memory, memory)?;
        Ok(())
      });
    }
    Ok(())
  }
}
