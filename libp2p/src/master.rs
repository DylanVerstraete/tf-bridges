use crate::{behaviour::Behaviour, SignRequest};
use libp2p::{identity::PeerId, request_response::RequestId};
use log::debug;
use std::error::Error;

pub struct SignerMaster<'a> {
    pub behaviour: &'a mut Behaviour,
}

impl<'a> SignerMaster<'a> {
    pub fn new(behaviour: &'a mut Behaviour) -> Self {
        Self { behaviour }
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
