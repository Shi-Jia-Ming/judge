use cgroups_rs::{
  blkio::BlkIoController, cgroup_builder::CgroupBuilder, cpu::CpuController,
  cpuset::CpuSetController, freezer::FreezerController, hugetlb::HugeTlbController,
  memory::MemController, pid::PidController, Cgroup, CgroupPid, Hierarchy, Subsystem,
};
use std::{path::PathBuf, process};
use tokio::process::Command;
use uuid::Uuid;

use crate::judge::utils::Memory;

use super::{Limit, LimitError};

/// use cgroups v2 to limit resources
pub struct CgroupLimit {
  /// Cgroup limits
  cgroup: Cgroup,
}

impl CgroupLimit {
  pub fn new(memory: Memory) -> Self {
    let heir = Box::new(V2Path::from("/sys/fs/cgroup/judger/"));
    let cg_name = format!("judger_{}", Uuid::new_v4());

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
  fn apply_to(&self, command: &mut Command) -> Result<(), LimitError> {
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
    self.cgroup.delete().expect("failed to delete cgroup files");
  }
}

#[derive(Debug, Clone)]
pub struct V2Path {
  root: String,
}

impl From<&str> for V2Path {
  fn from(path: &str) -> Self {
    Self {
      root: path.to_string(),
    }
  }
}

impl Hierarchy for V2Path {
  fn v2(&self) -> bool {
    true
  }

  fn subsystems(&self) -> Vec<Subsystem> {
    let p = format!("{}/{}", self.root, "cgroup.controllers");
    let ret = std::fs::read_to_string(p.as_str());
    if ret.is_err() {
      return vec![];
    }

    let mut subs = vec![];

    let controllers = ret.unwrap().trim().to_string();
    let mut controller_list: Vec<&str> = controllers.split(' ').collect();

    // The freezer functionality is present in V2, but not as a controller,
    // but apparently as a core functionality. FreezerController supports
    // that, but we must explicitly fake the controller here.
    controller_list.push("freezer");

    for s in controller_list {
      match s {
        "cpu" => {
          subs.push(Subsystem::Cpu(CpuController::new(self.root(), true)));
        }
        "io" => {
          subs.push(Subsystem::BlkIo(BlkIoController::new(self.root(), true)));
        }
        "cpuset" => {
          subs.push(Subsystem::CpuSet(CpuSetController::new(self.root(), true)));
        }
        "memory" => {
          subs.push(Subsystem::Mem(MemController::new(self.root(), true)));
        }
        "pids" => {
          subs.push(Subsystem::Pid(PidController::new(self.root(), true)));
        }
        "freezer" => {
          subs.push(Subsystem::Freezer(FreezerController::new(
            self.root(),
            true,
          )));
        }
        "hugetlb" => {
          subs.push(Subsystem::HugeTlb(HugeTlbController::new(
            self.root(),
            true,
          )));
        }
        _ => {}
      }
    }

    subs
  }

  fn root_control_group(&self) -> Cgroup {
    Cgroup::load(Box::new(self.clone()), "".to_string())
  }

  fn root(&self) -> PathBuf {
    PathBuf::from(self.root.clone())
  }
}
