use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct Time(u32);

impl Time {
  pub fn from_microseconds(microseconds: u32) -> Self {
    Time(microseconds)
  }

  pub fn from_seconds(seconds: u32) -> Self {
    Time(seconds * 1000)
  }
}

impl Time {
  pub fn into_seconds(self) -> u32 {
    self.0 / 1000
  }

  pub fn into_microseconds(self) -> u32 {
    self.0
  }
}

impl Serialize for Time {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_u32(self.into_microseconds())
  }
}

#[derive(Debug, Clone, Copy)]
pub struct Memory(u64);

impl Memory {
  pub fn from_bytes(bytes: u64) -> Self {
    Memory(bytes)
  }

  pub fn from_kilobytes(kilobytes: u64) -> Self {
    Memory(kilobytes * 1024)
  }

  pub fn from_megabytes(megabytes: u64) -> Self {
    Memory(megabytes * 1024 * 1024)
  }
}

impl Memory {
  pub fn into_bytes(self) -> u64 {
    self.0
  }

  pub fn into_kilobytes(self) -> u64 {
    self.0 / 1024
  }

  pub fn into_megabytes(self) -> u64 {
    self.0 / 1024 / 1024
  }
}

impl Serialize for Memory {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_u64(self.into_bytes())
  }
}
