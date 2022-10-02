use std::path::PathBuf;

use cgroups_rs::{
  blkio::BlkIoController, cpu::CpuController, cpuset::CpuSetController, freezer::FreezerController,
  hugetlb::HugeTlbController, memory::MemController, pid::PidController, Cgroup, Hierarchy,
  Subsystem,
};

#[derive(Debug, Clone)]
pub struct V2 {
  root: String,
}

impl From<&str> for V2 {
  fn from(path: &str) -> Self {
    Self {
      root: path.to_string(),
    }
  }
}

impl Hierarchy for V2 {
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
