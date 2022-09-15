use fs4::tokio::AsyncFileExt;
use log::{error, info, warn};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use thiserror::Error;
/// File operation module.
/// Requires file lock mechanism to work.
use tokio::fs;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use crate::util::{Logable, ReturnChannelError, ReturnChannelReceive, ReturnChannelSend};
use crate::wire::{RecvMessage, SendMessage, SyncRequest, SyncResponse};

#[derive(Error, Debug)]
pub enum FileError {
    #[error("Failed to (un)lock file.")]
    FlockError,
}

pub async fn resource_exists(uuid: &str) -> bool {
    false
}

/// sync_save_file()
/// Save data to path. `data` is Base64 encoded.
/// TODO: Release file lock on panic.
pub async fn save_file(data: Vec<u8>, path: String) -> Result<(), Box<dyn Error + Send + Sync>> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true) //No override operation in case of malfunction.
        .open(path)
        .await?;
    let file_unlocked =
        tokio::task::spawn_blocking(move || -> Result<File, Box<dyn Error + Send + Sync>> {
            file.lock_exclusive()?;
            Ok(file)
        })
        .await?;
    let mut file_unlocked = file_unlocked?;
    file_unlocked.write_all(data.as_ref()).await?;
    tokio::task::spawn_blocking(move || -> Result<(), tokio::io::Error> {
        file_unlocked.unlock()?;
        Ok(())
    });
    Ok(())
}

pub async fn save_opened_file(
    data: Vec<u8>,
    file: File,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let file_unlocked =
        tokio::task::spawn_blocking(move || -> Result<File, Box<dyn Error + Send + Sync>> {
            file.lock_exclusive()?;
            Ok(file)
        })
        .await?;
    let mut file_unlocked = file_unlocked?;
    file_unlocked.write_all(data.as_ref()).await?;
    tokio::task::spawn_blocking(move || -> Result<(), tokio::io::Error> {
        file_unlocked.unlock()?;
        Ok(())
    });
    Ok(())
}

pub async fn save_data_b64(data: String, path: String) -> Result<(), Box<dyn Error + Send + Sync>> {
    let data_bin = base64::decode(data)?;
    let path = format!("/var/judger/data/{}", path);
    save_file(data_bin, path).await
}

pub async fn load_data(id: &str) -> Result<String, Box<dyn Error>> {
    let full_path = format!("/var/judger/data/{}", id);
    read_file(full_path).await
}

pub async fn read_file(path: String) -> Result<String, Box<dyn Error>> {
    //Try to read such file.
    let file = fs::File::open(path).await?;
    let file_unlocked = tokio::task::spawn_blocking(move || -> Result<File, tokio::io::Error> {
        file.lock_shared()?;
        Ok(file)
    })
    .await?;
    let mut file_unlocked = file_unlocked?;
    let mut buf = String::new();
    file_unlocked.read_to_string(&mut buf).await?;
    tokio::task::spawn_blocking(move || -> Result<(), tokio::io::Error> {
        file_unlocked.unlock()?;
        Ok(())
    });
    Ok(buf)
}

pub async fn save_tmp_file(
    data: Vec<u8>,
    postfix: Option<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    save_file(data, "test".to_owned()).await
}

pub async fn read_tmp_file(name: &str) -> Result<String, Box<dyn Error>> {
    let full_path = format!("/tmp/judger/{}", name);
    read_file(full_path).await
}

pub enum FileSyncStatus {
    Ok, //Should the file already exists,we send Ok immediately.
    Error,
}

pub struct Syncer {
    cmd: ReturnChannelReceive<String, FileSyncStatus>,
    recv: mpsc::Receiver<SyncResponse>,
    send: mpsc::Sender<SendMessage>,
}

impl Syncer {
    pub fn new(
        cmd: ReturnChannelReceive<String, FileSyncStatus>,
        recv: mpsc::Receiver<SyncResponse>,
        send: mpsc::Sender<SendMessage>,
    ) -> Self {
        Self { cmd, recv, send }
    }
    pub async fn run_loop(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut waiting_file: HashMap<String, oneshot::Sender<FileSyncStatus>> = HashMap::new();
        loop {
            tokio::select! {
                Some((uuid,ret)) = self.cmd.recv() => {
                    //Check if file exists by simply opening this file.
                    let f = fs::File::open(format!("/var/judger/data/{}",uuid)).await;
                    //This is safe since we use file lock to prevent premature reads.
                    if let Ok(_) = f {
                        ret.send(FileSyncStatus::Ok).map_err(|_| ReturnChannelError::SendError).log();
                    } else {
                        //Send work.
                        match self.send.send(SendMessage::Sync(SyncRequest{uuid: uuid.clone()})).await {
                            Ok(()) => {
                                waiting_file.insert(uuid,ret);
                            }
                            k => {
                                k.log();
                            }
                        }
                    }

                },
                Some(recv) = self.recv.recv() => {
                    //If sync response refers to a non-existent file?
                    if let Some(ret) = waiting_file.remove(&recv.uuid.to_owned()) {
                        //Save content.
                        //TODO: split create file and write content to avoid blocking.
                        let status = match save_data_b64(recv.data,recv.uuid).await {
                            Ok(_) => FileSyncStatus::Ok,
                            Err(e) => {warn!("syncer: Failed to save file,reason {}",e);FileSyncStatus::Error}
                        };

                        //Callback.
                        ret.send(status).map_err(|_| ReturnChannelError::SendError).log();

                    } else {
                        warn!("syncer: server sends suprious sync data.");
                    }
                },
                else => {
                    info!("syncer: all channels has closed.");
                    break;
                }
            }
        }
        Ok(())
    }
}
