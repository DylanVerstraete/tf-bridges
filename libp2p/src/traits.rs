use crate::types::{SignRequest, SignResponse};

#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    #[error("invalid request")]
    InvalidRequest,
}

pub trait Signer {
    fn sign(&self, message: &SignRequest) -> Result<SignResponse, SignerError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CollectError {
    #[error("invalid response")]
    InvalidResponse,
}

pub trait Collector {
    fn collect(&mut self, response: &SignResponse) -> Result<(), CollectError>;
}
