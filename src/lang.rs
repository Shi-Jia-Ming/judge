use serde_derive::{Deserialize, Serialize};
use std::error::Error;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Lang {
    pub lang: String,
    pub file_extension: String,
    pub compile: Option<String>,
    pub run: Option<String>,
}
