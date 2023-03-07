pub use stellar_base::Network;

const HORIZON_URL: &str = "https://horizon.stellar.org";
const HORIZON_TEST_URL: &str = "https://horizon-testnet.stellar.org";

#[derive(Clone)]
pub enum StellarNetwork {
    Testnet,
    Mainnet,
}

impl StellarNetwork {
    pub fn to_network_url(self) -> &'static str {
        match self {
            StellarNetwork::Testnet => HORIZON_TEST_URL,
            StellarNetwork::Mainnet => HORIZON_URL,
        }
    }

    pub fn to_stellar_network(self) -> Network {
        match self {
            StellarNetwork::Mainnet => Network::new_public(),
            StellarNetwork::Testnet => Network::new_test(),
        }
    }
}
