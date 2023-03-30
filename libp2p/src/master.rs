use crate::{behaviour::Behaviour, SignRequest};

use event::{handle_request_response, MessageRequest, MessageResponse};
use libp2p::{core::PeerId, request_response::RequestId};
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

    pub async fn run(mut self) -> Result<(), ()> {
        loop {
            tokio::select! {
                swarm_event = self.swarm.select_next_some() => self.handle_swarm_event(swarm_event).await?,
            }
        }
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<Event, THandlerErr<Behaviour>>,
    ) -> Result<(), ()> {
        match event {
            SwarmEvent::Behaviour(Event::RequestResponse(event)) => {
                handle_request_response(self, event)
                    .await
                    .map_err(|err| error!("error while handling request response: {}", err))?;
            }
            ev => {
                debug!("other event: {:?}", ev);
            }
        }
        Ok(())
    }
}
