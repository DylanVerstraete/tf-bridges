use std::error::Error;

use crate::types::{SignRequest, SignResponse};

#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    #[error("invalid request")]
    InvalidRequest,
}

pub trait Signer {
    fn sign(&self, message: &SignRequest) -> Result<SignResponse, SignerError>;
}
