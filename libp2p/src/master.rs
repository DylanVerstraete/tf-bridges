use crate::{behaviour::Behaviour, SignRequest, types::SignResponse, traits::{CollectError, Master}};
use libp2p::{core::PeerId, request_response::RequestId};
use log::debug;
use std::error::Error;

pub struct SignerMaster<'a> {
    pub behaviour: &'a mut Behaviour,
    pub sign_responses: Vec<SignResponse>,
}

impl<'a> SignerMaster<'a> {
    pub fn new(behaviour: &'a mut Behaviour) -> Self {
        Self { behaviour, sign_responses: vec![] }
    }

    pub async fn send_signing_request(
        self,
        signing_request: SignRequest,
        peer: &PeerId,
    ) -> Result<RequestId, Box<dyn Error>> {
        debug!("sending request: {:?}", signing_request.clone());
        Ok(self
            .behaviour
            .request_response
            .send_request(&peer, signing_request))
    }
}

impl <'a>Master for SignerMaster<'a> {
    fn collect(&mut self, response: &SignResponse) -> Result<(), CollectError> {
        Ok(self.sign_responses.push(response.to_vec()))
    }
}
