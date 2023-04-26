use crate::{SignRequest, SignResponse, traits::{SignerError, Signer}};

pub struct SignerService {

}

impl SignerService {
    pub fn new() -> Self {
        Self {}
    }

    pub fn sign(&self, _message: &SignRequest) -> Result<SignResponse, SignerError> {
        Ok(SignResponse::new())
    }
}

impl Signer for SignerService {
    fn sign(&self, _message: &SignRequest) -> Result<SignResponse, SignerError> {
        Ok(SignResponse::new())
    }
}