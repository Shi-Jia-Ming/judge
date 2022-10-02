//! TODO: this limits does not work!

mod v2;

use cgroups_rs::{cgroup_builder::CgroupBuilder, Cgroup, CgroupPid};
use log::debug;
use std::{
  os::unix::process::CommandExt,
  process::{self, Command},
};
use uuid::Uuid;

use crate::utils::Memory;

use super::{Limit, LimitError};

/// Use cgroups v2 to limit resources
pub struct CgroupLimit {
  /// Cgroup limits
  cgroup: Cgroup,
}

impl CgroupLimit {
  pub fn new(memory: Memory) -> Self {
    debug!("Creating cgroup files");

    let heir = Box::new(v2::V2::from("/sys/fs/cgroup/judge"));
    let cg_name = format!("judge_{}", Uuid::new_v4());

    #[rustfmt::skip]
    let cgroup = CgroupBuilder::new(&cg_name)
      .memory()
        .kernel_memory_limit((memory.into_bytes() / 4) as i64)
        .memory_hard_limit(memory.into_bytes() as i64)
        .done()
      .cpu()
        // Currently, no multi-threaded/processed program is allowed for common OJ use.
        .cpus("1".to_owned())
        .done()
      .pid()
        .maximum_number_of_processes(cgroups_rs::MaxValue::Value(1000))
        .done()
      .build(heir);

    Self { cgroup }
  }
}

impl Limit for CgroupLimit {
  fn limit(&self, command: &mut Command) -> Result<(), LimitError> {
    let cgroup = self.cgroup.clone();
    unsafe {
      command.pre_exec(move || {
        cgroup
          .add_task(CgroupPid::from(process::id() as u64))
          .expect("failed to apply cgroup limit to task");
        Ok(())
      });
    }

    Ok(())
  }
}

impl Drop for CgroupLimit {
  fn drop(&mut self) {
    debug!("removing cgroup files");
    self.cgroup.delete().expect("failed to delete cgroup files");
  }
}
