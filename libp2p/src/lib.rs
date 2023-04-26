use behaviour::{Behaviour, Event};
use futures::prelude::*;
use libp2p::{
    identity::Keypair,
    multiaddr::Protocol,
    swarm::{Swarm, SwarmEvent, THandlerErr, SwarmBuilder},
    Multiaddr,
    request_response::{
        Event as RequestResponseEvent, Message as RequestResponseMessage,
    },
    core::{muxing::StreamMuxerBox, transport, PeerId},
};
use log::{debug, error, info};
use std::{error::Error, fs, path::Path};

use traits::{Signer, Master};
use types::{SignRequest, SignResponse};
use event::*;

pub mod behaviour;
pub mod event;
pub mod master;
pub mod signer;
pub mod traits;
pub mod types;

pub struct Libp2pHost<S, M> {
    pub identity: Keypair,
    pub local_peer_id: PeerId,
    pub swarm: Swarm<Behaviour>,
    pub signer: S,
    pub collector: M,
}

impl<S, M> Libp2pHost<S, M> 
    where S: Signer, M: Master 
{
    pub async fn new(
        behaviour: Behaviour,
        transport: transport::Boxed<(PeerId, StreamMuxerBox)>,
        keypair: Keypair,
        signer: S,
        collector: M,
    ) -> Result<Self, Box<dyn Error>> {
        let local_peer_id = PeerId::from_public_key(&keypair.public());

        let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(Libp2pHost {
            identity: keypair,
            local_peer_id,
            swarm,
            signer,
            collector
        })
    }

    pub async fn connect_to_relay(&mut self, relay: String) -> Result<(), Box<dyn Error>> {
        info!("connecting to relay: {}", relay);
        let relay_addr: Multiaddr = relay.parse()?;

        let dest_relay_addr = relay_addr.clone().with(Protocol::P2pCircuit);

        // Listen on the destination relay address so other peers can find us
        self.swarm.listen_on(dest_relay_addr)?;

        Ok(())
    }

    pub async fn run(mut self) -> Result<(), ()> {
        loop {
            tokio::select! {
                swarm_event = self.swarm.select_next_some() => self.handle_swarm_event(swarm_event).await?,
            }
        }
    }

    pub fn ping_peer(mut self, peer: String) -> Result<(), Box<dyn Error>> {
        let p = PeerId::from_bytes(peer.as_bytes())?;

        self.swarm.dial(p)?;

        Ok(())
    }

    pub fn ping(mut self, addr: String) -> Result<(), Box<dyn Error>> {
        let remote: Multiaddr = addr.parse()?;
        self.swarm.dial(remote)?;
        Ok(())
    }

    pub fn echo(mut self, addr: String) -> Result<(), Box<dyn Error>> {
        let remote: Multiaddr = addr.parse()?;

        self.swarm.dial(remote)?;
        Ok(())
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<Event, THandlerErr<Behaviour>>,
    ) -> Result<(), ()> {
        match event {
            SwarmEvent::Behaviour(Event::RequestResponse(event)) => {
                self.handle_request_response(event)
                    .await
                    .map_err(|err| error!("error while handling request response: {}", err))?;
            }
            SwarmEvent::Behaviour(Event::Identify(event)) => {
                info!("found identify event: {:?}", event);
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                let peer_id = self.swarm.local_peer_id().to_string();
                info!("Listening on {:?}, {:?}", address, peer_id);
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!("Connection established {:?} on {:?}", peer_id, endpoint);
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: maybe_peer_id,
                error,
                ..
            } => {
                error!(
                    "Outgoing connection error: {:?} to peer: {:?}",
                    error, maybe_peer_id
                );
            }
            SwarmEvent::Behaviour(Event::Relay(e)) => info!("{:?}", e),
            SwarmEvent::Behaviour(Event::Ping(_)) => {
                info!("pong")
            }
            ev => {
                debug!("other event: {:?}", ev);
            }
        }
        Ok(())
    }

    pub async fn handle_request_response (
        &mut self,
        event: RequestResponseEvent<SignRequest, SignResponse>,
    ) -> Result<(), SignRequestResponseError> {
        match event {
            RequestResponseEvent::Message { message, .. } => match message {
                RequestResponseMessage::Request {
                    request,
                    channel,
                    request_id: _req_id,
                } => {
                    info!("Request response 'Message::Request' for {:?}", request);
    
                        let response = 
                            self.signer
                            .sign(&request)
                            .map_err(|_| SignRequestResponseError::Unknown)?;
        
                        self.swarm
                            .behaviour_mut()
                            .request_response
                            .send_response(channel, response).map_err(|_| SignRequestResponseError::Unknown)?;
                },
                RequestResponseMessage::Response {
                    request_id,
                    response,
                } => {
                    info!(
                        "Request response 'Message::Response': {} {:?}",
                        request_id, response
                    );
                    self.collector.collect(&response).map_err(|_| SignRequestResponseError::Unknown)?
                }
            },
            RequestResponseEvent::OutboundFailure {
                request_id, error, ..
            } => {
                error!(
                    "Request {} response outbound failure {:?}",
                    request_id, error
                );
            }
            RequestResponseEvent::InboundFailure { error, .. } => {
                error!("Request response inbound failure {:?}", error);
            }
            RequestResponseEvent::ResponseSent { .. } => (),
        }
        Ok(())
    }
    
}

/// Read the pre shared key file from the given ipfs directory
pub fn get_psk(path: &Path) -> std::io::Result<Option<String>> {
    let swarm_key_file = path.join("swarm.key");
    match fs::read_to_string(swarm_key_file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
