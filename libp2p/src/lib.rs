use either::Either;
use futures::prelude::*;

use libp2p::{
    core::{muxing::StreamMuxerBox, transport, transport::upgrade::Version},
    identify,
    identity::Keypair,
    multiaddr::Protocol,
    noise, ping,
    pnet::{PnetConfig, PreSharedKey},
    relay,
    swarm::{keep_alive, NetworkBehaviour, SwarmEvent},
    yamux::YamuxConfig,
    Multiaddr, PeerId, Swarm, Transport,
};

use std::{error::Error, fs, path::Path, str::FromStr, time::Duration};

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    keep_alive: keep_alive::Behaviour,
    relay: relay::client::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
}

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

        // Build the relay transport and behaviour
        let (relay_transport, behaviour) = relay::client::new(local_peer_id);

        let transport = match psk {
            Some(_) => build_transport(relay_transport, &kp, psk)?,
            None => libp2p::development_transport(kp.clone()).await?,
        };

        let swarm = Swarm::with_async_std_executor(
            transport,
            Behaviour {
                ping: ping::Behaviour::new(ping::Config::new()),
                relay: behaviour,
                keep_alive: keep_alive::Behaviour,
                identify: identify::Behaviour::new(identify::Config::new(
                    "ipfs/0.1.0".to_string(),
                    kp.public(),
                )),
            },
            local_peer_id,
        );

        Ok(Libp2pHost {
            identity: kp,
            local_peer_id,
            swarm,
        })
    }

    pub async fn start(mut self, relay: String, peer_id: &str) -> Result<(), Box<dyn Error>> {
        println!("parsing relay addr");
        let relay_peer_id = PeerId::from_str(peer_id)?;
        println!("parsing relay peer id");
        let relay_addr: Multiaddr = relay.parse()?;

        let dest_relay_addr = relay_addr.clone().with(Protocol::P2p(relay_peer_id.into()));

        // let mut client_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
        let client_addr = relay_addr
            .clone()
            .with(Protocol::P2p(relay_peer_id.into()))
            .with(Protocol::P2pCircuit);

        let client_addr_with_id = client_addr
            .clone()
            .with(Protocol::P2p(self.local_peer_id.into()));

        println!("listening on {:?}", client_addr.clone());
        self.swarm.listen_on(client_addr)?;

        println!("dest relay addr: {:?}", dest_relay_addr);
        self.swarm.dial(dest_relay_addr.clone())?;

        println!("making reservation...");
        wait_for_reservation(&mut self, client_addr_with_id, relay_peer_id, false).await;

        // Do processing
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

/// Builds the transport that serves as a common ground for all connections.
/// If a psk is given, the transport will be secured
fn build_transport(
    transport: libp2p_relay::client::Transport,
    kp: &Keypair,
    psk: Option<String>,
) -> Result<transport::Boxed<(PeerId, StreamMuxerBox)>, Box<dyn Error>> {
    let noise_config = noise::NoiseAuthenticated::xx(kp)?;
    let yamux_config = YamuxConfig::default();

    let maybe_encrypted = match psk {
        Some(psk) => {
            let pk = PreSharedKey::from_str(psk.as_str())?;
            Either::Left(transport.and_then(move |socket, _| PnetConfig::new(pk).handshake(socket)))
        }
        None => Either::Right(transport),
    };

    Ok(maybe_encrypted
        .upgrade(Version::V1)
        .authenticate(noise_config)
        .multiplex(yamux_config)
        .timeout(Duration::from_secs(20))
        .boxed())
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
            SwarmEvent::Behaviour(BehaviourEvent::Relay(
                relay::client::Event::ReservationReqAccepted {
                    relay_peer_id: peer_id,
                    renewal,
                    ..
                },
            )) if relay_peer_id == peer_id && renewal == is_renewal => {
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
            SwarmEvent::Behaviour(BehaviourEvent::Ping(_)) => {}
            e => panic!("{e:?}"),
        }
    }
}

async fn wait_for_dial(client: &mut Libp2pHost, remote: PeerId) -> bool {
    loop {
        match client.swarm.select_next_some().await {
            SwarmEvent::Dialing(peer_id) if peer_id == remote => {}
            SwarmEvent::ConnectionEstablished { peer_id, .. } if peer_id == remote => return true,
            SwarmEvent::OutgoingConnectionError { peer_id, .. } if peer_id == Some(remote) => {
                return false
            }
            SwarmEvent::Behaviour(BehaviourEvent::Ping(_)) => {}
            e => panic!("{e:?}"),
        }
    }
}
