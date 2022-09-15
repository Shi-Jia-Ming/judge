use std::{fs, io::Read, path::Path};

use serde_derive::{Deserialize, Serialize};
/// Global Config
///
/// Configuration of judger daemon.
///
/// Search path:
/// - ./judger.conf (cwd of judger)
/// - /etc/judger.conf
/// - Default (when no config file available)
///
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GlobalConfig {
    /// WebSocket Listen Address
    pub addr: String,
    /// Server name
    pub name: String,
    /// Sync Data Directory
    pub data_dir: String,
    /// Temporary file directory
    pub tmp_dir: Option<String>, // Default to /tmp
}

impl GlobalConfig {
    fn read() -> Self {
        let dirs = ["./judger_conf.toml", "/etc/judger_conf.toml"];
        for i in dirs {
            match fs::File::open(i) {
                Ok(mut k) => {
                    let mut s = String::new();
                    k.read_to_string(&mut s).unwrap();
                    let conf: Self = toml::from_str(s.as_ref()).unwrap();
                    return conf;
                }
                Err(e) => {
                    continue;
                }
            }
        }
        Self::default()
    }
    fn read_from(path: String) -> Option<Self> {
        match fs::File::open(path) {
            Ok(mut k) => {
                let mut s = String::new();
                k.read_to_string(&mut s).unwrap();
                let conf: Self = toml::from_str(s.as_ref()).unwrap();
                return Some(conf);
            }
            Err(e) => None,
        }
    }
}

impl TryFrom<&str> for GlobalConfig {
    type Error = serde_json::Error;
    /// Convert a JSON string into GlobalConfig
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(value)
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:41234".to_owned(),
            name: "name".to_owned(),
            data_dir: "/var/data".to_owned(),
            tmp_dir: None,
        }
    }
}
