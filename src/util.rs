use async_trait::async_trait;
use log::{error, warn};
use std::{
    error::Error,
    fmt::{Debug, Display},
};
use thiserror::Error;
use tokio::sync::{
    mpsc,
    oneshot::{self, error::RecvError},
};

pub trait Logable {
    fn log(self);
}

impl<T, E> Logable for Result<T, E>
where
    E: Error,
{
    fn log(self) {
        if let Err(e) = self {
            warn!("{}", e);
        }
    }
}

pub type ReturnChannelReceive<Q: Send, R: Send> = mpsc::Receiver<(Q, oneshot::Sender<R>)>;
pub type ReturnChannelSend<Q: Send, R: Send> = mpsc::Sender<(Q, oneshot::Sender<R>)>;

#[derive(Error, Debug)]
pub enum ReturnChannelError {
    #[error("Failed to send.")]
    SendError,
    #[error("Failed to receive.")]
    ReceiveError,
}

#[async_trait]
pub trait ReturnChannel<Q: Send, R: Send> {
    async fn send_and_wait(&mut self, value: Q) -> Result<R, ReturnChannelError>;
}

#[async_trait]
impl<Q, R> ReturnChannel<Q, R> for mpsc::Sender<(Q, oneshot::Sender<R>)>
where
    Q: Send,
    R: Send,
{
    async fn send_and_wait(&mut self, value: Q) -> Result<R, ReturnChannelError> {
        let (send, recv) = oneshot::channel::<R>();
        //Send
        self.send((value, send))
            .await
            .map_err(|_| ReturnChannelError::SendError)?;
        Ok(recv.await.map_err(|_| ReturnChannelError::ReceiveError)?)
    }
}
