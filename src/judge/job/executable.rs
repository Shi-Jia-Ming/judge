use std::path::PathBuf;

pub enum Executable {
  /// Compiled into a executable file
  Elf(PathBuf),
  /// Scripting language which needs a runtime
  Command { name: String, args: Vec<String> },
}
