use crate::{
    types::{SignRequest, SignResponse},
    Libp2pHost,
};
pub use libp2p::request_response::RequestId;
use libp2p::request_response::{
    Event as RequestResponseEvent, Message as RequestResponseMessage, ResponseChannel,
};
use log::{error, info};

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

pub async fn handle_request_response(
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

                // Send the request on the signing requests channel
                // For the client now to handle and send back a response
                node.pending_singing_requests
                    .0
                    .send(MessageRequest {
                        request_id,
                        reply_channel: channel,
                        request,
                    })
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
                node.signing_requests_responses
                    .0
                    .send(MessageResponse {
                        request_id,
                        response,
                    })
                    .map_err(|_| SignRequestResponseError::Unknown)?;
            }
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
