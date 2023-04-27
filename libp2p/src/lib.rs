use behaviour::{Behaviour, Event};
use event::handle_request_response;
use event::*;
use futures::prelude::*;
use libp2p::{
    identity::{Keypair, PeerId},
    multiaddr::Protocol,
    request_response::RequestId,
    swarm::{Swarm, SwarmBuilder, SwarmEvent, THandlerErr},
    Multiaddr,
};
use log::{debug, error, info};
use std::collections::HashMap;
use std::{error::Error, fs, path::Path};
use tokio::sync::mpsc::{self, UnboundedSender};
use traits::Signer;
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
    responses: HashMap<RequestId, UnboundedSender<SignResponse>>,
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
            Behaviour::new_behaviour_and_transport(&kp, local_peer_id, psk)?;

        let mut swarm =
            SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(Libp2pHost {
            identity: kp,
            local_peer_id,
            swarm,
            signer,
            responses: HashMap::default(),
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

    pub fn ping_peer(mut self, peer: String) -> Result<(), Box<dyn Error>> {
        let p = PeerId::from_bytes(peer.as_bytes())?;

        self.swarm.dial(p)?;

        Ok(())
    }

    pub fn run(mut self) -> Handler {
        let (tx, mut rx) = mpsc::unbounded_channel::<SignRequestForPeers>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    swarm_event = self.swarm.select_next_some() => self.handle_swarm_event(swarm_event).await.unwrap(),

                    request = rx.recv() => {
                        debug!("received request to send to peers: {:?}", request);
                        let peers = self.swarm.connected_peers().cloned().collect::<Vec<_>>();
                        if let Some(r) = request {
                            for peer in peers {
                                //TODO: may be generate the request id first so we can update the
                                // responses map BEFORE we send the message so we have zero chances that
                                // we receive a response before we updat the map
                                let request_id = self
                                    .swarm
                                    .behaviour_mut()
                                    .request_response
                                    .send_request(&peer, r.request.clone());
                                debug!("sent request with id: {:?} to peer {:?}", request_id, peer);
                                self.responses.insert(request_id, r.tx.clone());
                            }
                        }
                    },
                }
            }
        });

        Handler::new(tx)
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<Event, THandlerErr<Behaviour>>,
    ) -> Result<(), SignRequestResponseError> {
        match event {
            SwarmEvent::Behaviour(Event::RequestResponse(event)) => {
                handle_request_response(self, event).await?
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
}

#[derive(Debug)]
pub struct SignRequestForPeers {
    request: SignRequest,
    tx: UnboundedSender<SignResponse>,
}

#[derive(Debug, Clone)]
pub struct Handler {
    tx: UnboundedSender<SignRequestForPeers>,
}

impl Handler {
    fn new(tx: UnboundedSender<SignRequestForPeers>) -> Self {
        Self { tx }
    }

    pub async fn send(
        &self,
        request: SignRequest,
        min_sigs: usize,
    ) -> Result<Vec<SignResponse>, Box<dyn Error>> {
        let (tx, mut rx) = mpsc::unbounded_channel::<SignResponse>();

        let request = SignRequestForPeers { request, tx };

        self.tx.send(request)?;

        let mut responses: Vec<SignResponse> = Vec::default();
        loop {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out")))
                }
                response = rx.recv() => {
                    match response {
                        Some(response) => {
                            debug!("received response: {:?}", response);
                            responses.push(response);
                        }
                        None => {
                            debug!("no response was received");
                        }
                    };

                    if responses.len() >= min_sigs{
                        debug!("got enough responses");
                        break;
                    }
                }
            }
        }

        return Ok(responses);
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
