use behaviour::{Behaviour, Event};
use futures::prelude::*;
use libp2p::core::PeerId;
use libp2p::{
    multiaddr::Protocol,
    swarm::{Swarm, SwarmEvent, THandlerErr},
    Multiaddr,
};
use log::{debug, error, info};
use std::{error::Error, fs, path::Path};
use tokio::sync::mpsc;
use traits::Signer;
use types::SignRequest;

use event::{handle_request_response, MessageRequest, MessageResponse};
pub use libp2p::identity::ed25519;
pub use libp2p::identity::Keypair;

pub mod behaviour;
pub mod event;
pub mod master;
pub mod traits;
pub mod types;
pub struct Libp2pHost {
    pub identity: Keypair,
    pub local_peer_id: PeerId,
    pub swarm: Swarm<Behaviour>,
    pub signer: Box<dyn Signer>,
}

impl Libp2pHost {
    pub async fn new(
        keypair: Option<Keypair>,
        psk: Option<String>,
        signer: Box<dyn Signer>,
    ) -> Result<Self, Box<dyn Error>> {
        let kp = match keypair {
            Some(kp) => kp,
            None => Keypair::generate_ed25519(),
        };

        let local_peer_id = PeerId::from(kp.public());
        let (behaviour, transport) =
            Behaviour::new_behaviour_and_transport(&kp, local_peer_id, psk)?;

        let mut swarm = Swarm::with_tokio_executor(transport, behaviour, local_peer_id);

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(Libp2pHost {
            identity: kp,
            local_peer_id,
            swarm,
            signer,
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
                handle_request_response(self, event)
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
