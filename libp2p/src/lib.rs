use either::Either;
use futures::prelude::*;

use libp2p::{
    core::{
        muxing::StreamMuxerBox, transport, transport::upgrade::Version, transport::OrTransport,
    },
    identify,
    identity::Keypair,
    multiaddr::Protocol,
    noise, ping,
    pnet::{PnetConfig, PreSharedKey},
    relay,
    swarm::{keep_alive, NetworkBehaviour, Swarm, SwarmEvent},
    tcp,
    yamux::YamuxConfig,
    Multiaddr, PeerId, Transport,
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
        let (relay_transport, behaviour) = relay::client::new(local_peer_id);

        let transport = match psk {
            Some(_) => build_transport(relay_transport, &kp, psk)?,
            None => libp2p::development_transport(kp.clone()).await?,
        };

        // let (relay_transport, behaviour) = relay::client::new(local_peer_id);
        // let transport = OrTransport::new(relay_transport, transport);

        let swarm = Swarm::with_tokio_executor(
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

    pub async fn start_relay_client(
        mut self,
        relay: String,
        port: i32,
        peer_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let relay_peer_id = PeerId::from_str(peer_id)?;
        let relay_addr: Multiaddr = relay.parse()?;

        let dest_relay_addr = relay_addr
            .clone()
            .with(Protocol::P2p(relay_peer_id.into()))
            .with(Protocol::P2pCircuit);

        println!("dest relay addr: {:?}", dest_relay_addr.clone());

        self.swarm.listen_on(
            dest_relay_addr
                .clone()
                .with(Protocol::P2p(self.local_peer_id.into())),
        )?;

        self.swarm.dial(dest_relay_addr.clone())?;

        // println!("making reservation...");
        // wait_for_reservation(&mut self, client_addr_with_id, relay_peer_id, false).await;

        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr {
                    listener_id,
                    address,
                } => {
                    println!("{:?} listening on {}", listener_id, address)
                }
                SwarmEvent::Dialing(peer_id) => {
                    println!("dialing peer: {}", peer_id);
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } if peer_id == relay_peer_id => {
                    println!("connected to relay");
                }
                SwarmEvent::Behaviour(BehaviourEvent::Ping(_)) => {
                    println!("pong")
                }
                SwarmEvent::OutgoingConnectionError { peer_id, .. }
                    if peer_id == Some(relay_peer_id) =>
                {
                    println!("outgoing conn err")
                }
                e => panic!("{e:?}"),
            }
        }
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
    base_transport: libp2p_relay::client::Transport,
    kp: &Keypair,
    psk: Option<String>,
) -> Result<transport::Boxed<(PeerId, StreamMuxerBox)>, Box<dyn Error>> {
    let noise_config = noise::NoiseAuthenticated::xx(kp)?;
    let yamux_config = YamuxConfig::default();

    // let base_transport = tcp::async_io::Transport::new(tcp::Config::default().nodelay(true));

    let maybe_encrypted = match psk {
        Some(psk) => {
            let pk = PreSharedKey::from_str(psk.as_str())?;
            Either::Left(
                base_transport.and_then(move |socket, _| PnetConfig::new(pk).handshake(socket)),
            )
        }
        None => Either::Right(base_transport),
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

// async fn wait_for_reservation(
//     client: &mut Libp2pHost,
//     client_addr: Multiaddr,
//     relay_peer_id: PeerId,
//     is_renewal: bool,
// ) {
//     let mut new_listen_addr = false;
//     let mut reservation_req_accepted = false;
//     let client_addr = client_addr.with(Protocol::P2p(client.local_peer_id.into()));
//     loop {
//         match client.swarm.select_next_some().await {
//             SwarmEvent::Behaviour(BehaviourEvent::Relay(
//                 relay::client::Event::ReservationReqAccepted {
//                     relay_peer_id: peer_id,
//                     renewal,
//                     ..
//                 },
//             )) if relay_peer_id == peer_id && renewal == is_renewal => {
//                 reservation_req_accepted = true;
//                 if new_listen_addr {
//                     break;
//                 }
//             }
//             SwarmEvent::NewListenAddr { address, .. } if address == client_addr => {
//                 new_listen_addr = true;
//                 if reservation_req_accepted {
//                     break;
//                 }
//             }
//             SwarmEvent::Behaviour(BehaviourEvent::Ping(_)) => {}
//             e => panic!("{e:?}"),
//         }
//     }
// }

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
