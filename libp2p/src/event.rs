use crate::{
    traits::Signer,
    types::{SignRequest, SignResponse},
    Libp2pHost,
};
use libp2p::request_response::{Event as RequestResponseEvent, Message as RequestResponseMessage};
use log::{debug, error, info};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignRequestResponseError {
    #[error("transaction not found")]
    NotFound(String),
    #[error("failed to sign")]
    FailedToSign(String),
    #[error("failed to send")]
    FailedToSend,
    #[error("unknown error")]
    Unknown,
}

pub async fn handle_request_response<S>(
    node: &mut Libp2pHost<S>,
    event: RequestResponseEvent<SignRequest, SignResponse>,
) -> Result<(), SignRequestResponseError>
where
    S: Signer,
{
    match event {
        RequestResponseEvent::Message { message, .. } => match message {
            RequestResponseMessage::Request {
                request,
                channel,
                request_id: _req_id,
            } => {
                info!("request response 'Message::Request' for {:?}", request);

                let response = node
                    .signer
                    .sign(&request)
                    .map_err(|e| SignRequestResponseError::FailedToSign(e.to_string()))?;

                debug!("request is signed: {:?}", response);
                node.swarm
                    .behaviour_mut()
                    .request_response
                    .send_response(channel, response)
                    .map_err(|_| SignRequestResponseError::FailedToSend)?;
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
                if let Some(tx) = node.responses.remove(&request_id) {
                    tx.send(response).unwrap();
                }
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
        RequestResponseEvent::ResponseSent { request_id, peer } => {
            info!("Request response response sent {} {:?}", request_id, peer);
        }
    }
    Ok(())
}
