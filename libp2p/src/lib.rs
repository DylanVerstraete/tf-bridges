use futures::prelude::*;
use libp2p::{
    identity::Keypair,
    multiaddr::Protocol,
    relay,
    swarm::{Swarm, SwarmEvent},
    Multiaddr, PeerId,
};
use log::{debug, info};
pub mod behaviour;
use behaviour::{Behaviour, Event};
use std::{error::Error, fs, path::Path, str::FromStr};

pub struct Libp2pHost {
    pub identity: Keypair,
    pub local_peer_id: PeerId,
    pub swarm: Swarm<Behaviour>,
}

impl Libp2pHost {
    pub async fn new(
        keypair: Option<Keypair>,
        psk: Option<String>,
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
        })
    }

    pub async fn connect_to_relay(
        mut self,
        relay: String,
        peer_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let relay_peer_id = PeerId::from_str(peer_id)?;
        let relay_addr: Multiaddr = relay.parse()?;

        let dest_relay_addr = relay_addr
            .clone()
            .with(Protocol::P2p(relay_peer_id.into()))
            .with(Protocol::P2pCircuit);

        debug!("dest relay addr: {:?}", dest_relay_addr.clone());

        self.swarm.listen_on(dest_relay_addr.clone())?;
        wait_for_dial(&mut self, relay_peer_id).await;

        info!("making reservation...");
        wait_for_reservation(&mut self, dest_relay_addr.clone(), relay_peer_id, false).await;
        self.swarm.dial(dest_relay_addr.clone())?;

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

async fn wait_for_reservation(
    client: &mut Libp2pHost,
    client_addr: Multiaddr,
    relay_peer_id: PeerId,
    is_renewal: bool,
) {
    let mut new_listen_addr = false;
    let mut reservation_req_accepted = false;
    let client_addr = client_addr.with(Protocol::P2p(client.local_peer_id.into()));
    loop {
        match client.swarm.select_next_some().await {
            SwarmEvent::Behaviour(Event::Relay(relay::client::Event::ReservationReqAccepted {
                relay_peer_id: peer_id,
                renewal,
                ..
            })) if relay_peer_id == peer_id && renewal == is_renewal => {
                reservation_req_accepted = true;
                if new_listen_addr {
                    break;
                }
            }
            SwarmEvent::NewListenAddr { address, .. } if address == client_addr => {
                new_listen_addr = true;
                if reservation_req_accepted {
                    break;
                }
            }
            SwarmEvent::Behaviour(Event::Ping(_)) => {}
            SwarmEvent::ListenerClosed {
                addresses, reason, ..
            } => {
                debug!("listener {:?} closed because of: {:?}", addresses, reason);
            }
            ev => {
                debug!("other event: {:?}", ev);
            }
        }
    }
}

async fn wait_for_dial(client: &mut Libp2pHost, remote: PeerId) -> bool {
    loop {
        match client.swarm.select_next_some().await {
            SwarmEvent::Dialing(peer_id) if peer_id == remote => {}
            SwarmEvent::ConnectionEstablished { peer_id, .. } if peer_id == remote => {
                info!("connection to {} established", peer_id);
                return true;
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error } if peer_id == Some(remote) => {
                debug!("connection error: {:?}", error);
                return false;
            }
            SwarmEvent::Behaviour(Event::Ping(_)) => {}
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("listening on {}", address);
            }
            ev => {
                debug!("other event: {:?}", ev);
            }
        }
    }
}
