use behaviour::{Behaviour, Event};
use event::*;
use futures::{channel::mpsc::UnboundedSender, prelude::*};
use libp2p::{
    core::{muxing::StreamMuxerBox, transport, PeerId},
    identity::Keypair,
    multiaddr::Protocol,
    request_response::{Event as RequestResponseEvent, Message as RequestResponseMessage},
    swarm::{Swarm, SwarmBuilder, SwarmEvent, THandlerErr},
    Multiaddr,
};
use log::{debug, error, info};
use std::collections::HashSet;
use std::{error::Error, fs, path::Path};
use tokio::sync::mpsc;
use traits::{Collector, Signer};
use types::{SignRequest, SignResponse};

pub mod behaviour;
pub mod event;
pub mod traits;
pub mod types;

pub struct Libp2pHost<S> {
    pub identity: Keypair,
    pub local_peer_id: PeerId,
    pub swarm: Swarm<Behaviour>,
    pub signer: S,
    pending_requests: (
        UnboundedReceiver<SignResponse>,
        UnboundedSender<SignRequest>,
    ),
    responses: HashSet<RequestId, SignResponse>,
}

impl<S> Libp2pHost<S>
where
    S: Signer,
{
    pub async fn new(
        keypair: Option<Keypair>,
        psk: Option<String>,
        signer: S,
    ) -> Result<Self, Box<dyn Error>> {
        let kp = match keypair {
            Some(kp) => kp,
            None => Keypair::generate_ed25519(),
        };

        let local_peer_id = PeerId::from(kp.public());
        let (behaviour, transport) =
            Behaviour::new_behaviour_and_transport(local_peer_id, psk, &kp)?;

        let mut swarm =
            SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        let (tx, mut rx) = mpsc::unbounded_channel();

        Ok(Libp2pHost {
            identity: keypair,
            local_peer_id,
            swarm,
            signer,
            pending_requests: (tx, rx),
            responses: HashSet::new(),
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

                request = self.pending_requests.recv() => self.push_request_to_swarm(request).await?,
            }
        }
    }

    fn push_request_to_swarm(&mut self, request: SignRequest) -> Result<RequestId, ()> {
        self.swarm
            .behaviour_mut()
            .request_response
            .send_request(&request, request.clone())?;
    }

    pub fn push_to_pending_requests(&mut self, request: SignRequest) -> Result<(), ()> {
        self.pending_requests.send(request)?
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

    pub async fn handle_request_response(
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

                    let response = self
                        .signer
                        .sign(&request)
                        .map_err(|_| SignRequestResponseError::Unknown)?;

                    self.swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, response)
                        .map_err(|_| SignRequestResponseError::Unknown)?;
                }
                RequestResponseMessage::Response {
                    request_id,
                    response,
                } => {
                    info!(
                        "Request response 'Message::Response': {} {:?}",
                        request_id, response
                    );
                    // Gather responses
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
