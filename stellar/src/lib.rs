use base64::{engine::general_purpose, Engine as _};
use std::str;
use stellar_base::crypto::KeyPair;
pub use stellar_base::{Network, PublicKey, Transaction};
use stellar_horizon::api::accounts;
use stellar_horizon::client::{HorizonClient, HorizonHttpClient};
use stellar_horizon::resources::Signer;
pub mod network;
use network::StellarNetwork;

pub struct Client {
    pub kp: KeyPair,
    pub network: StellarNetwork,
}

impl Client {
    pub fn new(seed: &str, network: StellarNetwork) -> Result<Self, Box<dyn std::error::Error>> {
        let kp = new_keypair(seed)?;
        Ok(Client { kp, network })
    }

    pub fn sign(&self, tx: Transaction) -> Result<(), Box<dyn std::error::Error>> {
        Ok(sign(&self.kp, tx, self.network.clone())?)
    }
}

fn new_keypair(seed: &str) -> Result<KeyPair, Box<dyn std::error::Error>> {
    Ok(KeyPair::from_secret_seed(seed)?)
}

fn sign(
    kp: &KeyPair,
    mut tx: Transaction,
    network: StellarNetwork,
) -> Result<(), Box<dyn std::error::Error>> {
    Ok(tx.sign(kp, &network.to_stellar_network())?)
}

pub async fn fetch_singers_from_account(
    address: String,
    network: StellarNetwork,
) -> Result<Vec<Signer>, Box<dyn std::error::Error>> {
    let public = PublicKey::from_account_id(&address)?;
    let a = accounts::single(&public);

    let horizon_cl = HorizonHttpClient::new(network.to_network_url())?;

    let resp = horizon_cl.request(a).await?;

    Ok(resp.1.signers)
}

// Reads data entry on the target account and looks for the value of "id"
// This value should be a libp2p peer id
pub async fn fetch_peer_id_from_account(
    address: String,
    network: StellarNetwork,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let public = PublicKey::from_account_id(&address)?;
    let a = accounts::single(&public);

    let horizon_cl = HorizonHttpClient::new(network.to_network_url())?;

    let mut resp = horizon_cl.request(a).await?;

    let id = match resp.1.data.remove("id") {
        Some(d) => Some(
            String::from_utf8(
                general_purpose::STANDARD
                    .decode(d)
                    .expect("valid base58 id"),
            )
            .expect("found invalid utf-8"),
        ),
        None => None,
    };

    Ok(id)
}
