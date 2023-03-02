use stellar_base::crypto::KeyPair;
pub use stellar_base::{Network, Transaction};

pub struct Client {
    pub kp: KeyPair,
    pub network: Network,
}

impl Client {
    pub fn new(seed: &str, network: Network) -> Result<Self, Box<dyn std::error::Error>> {
        let kp = new_keypair(seed)?;
        Ok(Client { kp, network })
    }

    pub fn sign(&self, mut tx: Transaction) -> Result<(), Box<dyn std::error::Error>> {
        Ok(sign(&self.kp, tx)?)
    }
}

fn new_keypair(seed: &str) -> Result<KeyPair, Box<dyn std::error::Error>> {
    Ok(KeyPair::from_secret_seed(seed)?)
}

fn sign(kp: &KeyPair, mut tx: Transaction) -> Result<(), Box<dyn std::error::Error>> {
    Ok(tx.sign(kp, &Network::new_public())?)
}
