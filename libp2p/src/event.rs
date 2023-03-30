use crate::{
    traits::Signer,
    types::{SignRequest, SignResponse},
    Libp2pHost,
};
pub use libp2p::request_response::RequestId;
use libp2p::request_response::{
    Event as RequestResponseEvent, Message as RequestResponseMessage, ResponseChannel,
};
use log::{error, info};
use std::error::Error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignRequestResponseError {
    #[error("transaction not found")]
    NotFound(String),
    #[error("unknown error")]
    Unknown,
}

pub struct MessageRequest {
    pub request_id: RequestId,
    pub reply_channel: ResponseChannel<Vec<u8>>,
    pub request: SignRequest,
}

pub struct MessageResponse {
    pub request_id: RequestId,
    pub response: SignResponse,
}

pub async fn handle_response(
    master: SignerMaster,
    event: RequestResponseEvent<SignRequest, SignResponse>,
) -> Result<(), Box<dyn Error>> {
    match event {
        RequestResponseMessage::Response {
            request_id,
            response,
        } => {
            info!(
                "Request response 'Message::Response': {} {:?}",
                request_id, response
            );
            ()
        }
        _ => (),
    }
    Ok(())
}

pub async fn handle_request(
    node: &mut Libp2pHost,
    event: RequestResponseEvent<SignRequest, SignResponse>,
) -> Result<(), SignRequestResponseError> {
    match event {
        RequestResponseEvent::Message { message, .. } => match message {
            RequestResponseMessage::Request {
                request,
                channel,
                request_id,
            } => {
                info!("Request response 'Message::Request' for {:?}", request);

                let response = node
                    .signer
                    .sign(&request)
                    .map_err(|_| SignRequestResponseError::Unknown)?;

                node.swarm
                    .behaviour_mut()
                    .request_response
                    .send_response(channel, response)?;
            }
            _ => (),
        },
        RequestResponseEvent::OutboundFailure {
            request_id, error, ..
        } => {
            error!(
                "Request {} response outbound failure {:?}",
                request_id, error
            );
            // node.pending_request_file.remove(&request_id);
            // node.bridge.connect_blocking()?;
            // node.bridge.send(Instruction::respond_fetch(None)).await?;
        }
        RequestResponseEvent::InboundFailure { error, .. } => {
            error!("Request response inbound failure {:?}", error);
        }
        RequestResponseEvent::ResponseSent { .. } => (),
    }
    Ok(())
}
