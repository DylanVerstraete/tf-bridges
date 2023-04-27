use crate::types::{SignRequest, SignResponse};
use async_trait::async_trait;
use either::Either;
use futures::AsyncReadExt;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport, transport::upgrade::Version},
    futures::{AsyncRead, AsyncWrite, AsyncWriteExt},
    identify,
    identify::Event as IdentifyEvent,
    identity::{Keypair, PeerId},
    noise, ping,
    ping::Event as PingEvent,
    pnet::{PnetConfig, PreSharedKey},
    relay,
    relay::client::{Behaviour as RelayBehaviour, Event as RelayEvent},
    request_response::{
        Behaviour as RequestResponseBehaviour, Codec as RequestResponseCodec,
        Config as RequestResponseConfig, Event as RequestResponseEvent, ProtocolName,
        ProtocolSupport,
    },
    tcp,
    yamux::YamuxConfig,
    Transport,
};
use libp2p_swarm_derive::NetworkBehaviour;

use std::io;
use std::iter::once;

use std::{str::FromStr, time::Duration};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
pub struct Behaviour {
    pub relay: RelayBehaviour,
    pub ping: ping::Behaviour,
    pub identify: identify::Behaviour,
    pub request_response: RequestResponseBehaviour<ExchangeCodec>,
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

        let request_response = RequestResponseBehaviour::new(
            ExchangeCodec,
            once((ExchangeProtocol, ProtocolSupport::Full)),
            RequestResponseConfig::default(),
        );

        Ok((
            Behaviour {
                ping: ping::Behaviour::new(ping::Config::new()),
                relay: behaviour,
                // keep_alive: keep_alive::Behaviour,
                identify: identify::Behaviour::new(identify::Config::new(
                    "ipfs/0.1.0".to_string(),
                    kp.public(),
                )),
                request_response,
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
    RequestResponse(RequestResponseEvent<SignRequest, SignResponse>),
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

impl From<RequestResponseEvent<SignRequest, SignResponse>> for Event {
    fn from(event: RequestResponseEvent<SignRequest, SignResponse>) -> Self {
        Self::RequestResponse(event)
    }
}

#[derive(Debug, Clone)]
pub struct ExchangeProtocol;

impl ProtocolName for ExchangeProtocol {
    fn protocol_name(&self) -> &[u8] {
        b"/stellar/1"
    }
}

#[derive(Clone)]
pub struct ExchangeCodec;

#[async_trait]
impl RequestResponseCodec for ExchangeCodec {
    type Protocol = ExchangeProtocol;
    type Request = SignRequest;
    type Response = SignResponse;

    async fn read_request<T: Send + Unpin + AsyncRead>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request> {
        let mut buff = vec![];
        io.read_to_end(&mut buff).await?;

        let req =
            SignRequest::try_from(buff.as_slice()).map_err(|_| io::ErrorKind::InvalidInput)?;
        Ok(req)
    }

    async fn read_response<T: Send + Unpin + AsyncRead>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response> {
        let mut buff = vec![];
        io.read_to_end(&mut buff).await?;

        Ok(buff)
    }

    async fn write_request<T: Send + Unpin + AsyncWrite>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()> {
        let b: Vec<u8> = req.try_into().map_err(|_| io::ErrorKind::InvalidInput)?;
        io.write_all(&b).await?;
        io.close().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        resp: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        io.write_all(&resp).await?;
        io.close().await?;

        Ok(())
    }
}
