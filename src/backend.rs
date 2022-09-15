use crate::job::*;
use async_trait::async_trait;
use std::path::*;

pub trait Backend {
    fn run() -> Result<JobResult, ()>;
    fn compile() -> Result<JobResult, ()>;
    fn judge() -> Result<JobResult, ()>;
}
