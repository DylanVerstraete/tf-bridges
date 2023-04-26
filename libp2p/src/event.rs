use crate::{
    types::{SignRequest, SignResponse},
};
pub use libp2p::request_response::RequestId;
use libp2p::request_response::{
    ResponseChannel,
};
use log::error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignRequestResponseError {
    #[error("transaction not found")]
    NotFound(String),
    #[error("unknown error")]
    Unknown,
}

pub struct MessageRequest {
    pub request_id: RequestId,
    pub reply_channel: ResponseChannel<Vec<u8>>,
    pub request: SignRequest,
}

pub struct MessageResponse {
    pub request_id: RequestId,
    pub response: SignResponse,
}

