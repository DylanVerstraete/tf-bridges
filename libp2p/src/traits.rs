use crate::types::{SignRequest, SignResponse};

#[derive(Debug, thiserror::Error, Clone, Copy)]
pub enum SignerError {
    #[error("invalid request")]
    InvalidRequest,
}

pub trait Signer: Send + Sync + 'static {
    fn sign(&self, message: &SignRequest) -> Result<SignResponse, SignerError>;
}
