use bson::{from_slice, to_vec};
use serde::{Deserialize, Serialize};

pub type MintRequest = Vec<u8>;
pub type StellarRequest = Vec<u8>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SignRequest {
    MintRequest(MintRequest),
    StellarRequest(StellarRequest),
}

pub type SignResponse = Vec<u8>;

impl TryFrom<&[u8]> for SignRequest {
    type Error = bson::de::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        from_slice(value)
    }
}

impl TryInto<Vec<u8>> for SignRequest {
    type Error = bson::ser::Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(to_vec(&self)?)
    }
}
