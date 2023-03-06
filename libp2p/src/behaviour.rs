use either::Either;
use libp2p::identify::Event as IdentifyEvent;
use libp2p::ping::Event as PingEvent;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport, transport::upgrade::Version},
    identify,
    identity::Keypair,
    noise, ping,
    pnet::{PnetConfig, PreSharedKey},
    relay,
    swarm::NetworkBehaviour,
    tcp,
    yamux::YamuxConfig,
    PeerId, Transport,
};
use libp2p_relay::client::Event as RelayEvent;

use std::{str::FromStr, time::Duration};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event", event_process = false)]
pub struct Behaviour {
    relay: relay::client::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
}

impl Behaviour {
    pub fn new_behaviour_and_transport(
        kp: &Keypair,
        peer_id: PeerId,
        psk: Option<String>,
    ) -> Result<(Self, transport::Boxed<(PeerId, StreamMuxerBox)>), Box<dyn std::error::Error>>
    {
        let noise_config = noise::NoiseAuthenticated::xx(kp)?;
        let yamux_config = YamuxConfig::default();

        let base_transport = tcp::async_io::Transport::new(tcp::Config::default().nodelay(true));
        let (relay_transport, behaviour) = relay::client::new(peer_id);

        let transport = base_transport.or_transport(relay_transport).boxed();

        let maybe_encrypted = match psk {
            Some(psk) => {
                let pk = PreSharedKey::from_str(psk.as_str())?;
                Either::Left(
                    transport.and_then(move |socket, _| PnetConfig::new(pk).handshake(socket)),
                )
            }
            None => Either::Right(transport),
        };

        Ok((
            Behaviour {
                ping: ping::Behaviour::new(ping::Config::new()),
                relay: behaviour,
                // keep_alive: keep_alive::Behaviour,
                identify: identify::Behaviour::new(identify::Config::new(
                    "ipfs/0.1.0".to_string(),
                    kp.public(),
                )),
            },
            maybe_encrypted
                .upgrade(Version::V1)
                .authenticate(noise_config)
                .multiplex(yamux_config)
                .timeout(Duration::from_secs(20))
                .boxed(),
        ))
    }
}

#[derive(Debug)]
pub enum Event {
    Identify(IdentifyEvent),
    Relay(RelayEvent),
    Ping(PingEvent),
}

impl From<IdentifyEvent> for Event {
    fn from(event: IdentifyEvent) -> Self {
        Self::Identify(event)
    }
}

impl From<RelayEvent> for Event {
    fn from(event: RelayEvent) -> Self {
        Self::Relay(event)
    }
}

impl From<PingEvent> for Event {
    fn from(event: PingEvent) -> Self {
        Self::Ping(event)
    }
}
