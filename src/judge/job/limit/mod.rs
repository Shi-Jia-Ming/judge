use std::process::Command;
use thiserror::Error;

pub mod cgroup;
pub mod rlimit;

#[derive(Debug, Error)]
pub enum LimitError {}

/// Limit command's resources
///
/// to clean the limits, impl the [`Drop`] trait
pub trait Limit {
  /// apply this limit to the command
  fn limit(&self, command: &mut Command) -> Result<(), LimitError>;
}
