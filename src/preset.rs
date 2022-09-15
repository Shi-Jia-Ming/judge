use serde_derive::*;
use std::borrow::Cow;
use thiserror::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Preset {
    ulimit_files: i64,
    stack: i64,
    cpus: i64,
    memory: i64,
    time: i32,
    scmp: bool,
}

#[derive(Error, Debug)]
pub enum PresetError {
    #[error("Failed to set memory")]
    Memory,
    #[error("Failed to set file limits")]
    File,
    #[error("Failed to set subprocess limits")]
    Process,
    #[error("Failed to set stack size")]
    Stack,
    #[error("Failed to apply seccomp")]
    Seccomp,
}

impl Preset {
    fn new(time: i64, memory: i64, files: i64, stack: i64, cpus: i64, scmp: bool) -> Self {
        Preset {
            ulimit_files: files,
            stack: stack,
            cpus: cpus,
            memory: 128 * 1024 * 1024,
            time: 1000,
            scmp: scmp,
        }
    }

    /// Apply a preset on current process.
    ///
    fn apply(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
