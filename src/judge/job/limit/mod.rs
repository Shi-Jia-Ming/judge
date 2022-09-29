use thiserror::Error;
use tokio::process::Command;

pub mod cgroup;
pub mod rlimit;

#[derive(Debug, Error)]
pub enum LimitError {}

pub trait Limit {
  fn apply_to(&self, command: &mut Command) -> Result<(), LimitError>;
}
