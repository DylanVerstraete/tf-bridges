use pretty_env_logger;
use std::env;
use std::path::Path;
use tf_libp2p::{get_psk, master::SignerMaster, Libp2pHost};
use tf_stellar::{fetch_peer_id_from_account, network::StellarNetwork, Client};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let args: Vec<String> = env::args().collect();

    let stellar_secret = &args[1];
    let relay_addr = &args[2];

    // let kp = ed25519::SecretKey::from_bytes(stellar_secret.as_bytes().try_into().unwrap());

    // let s = "SAN72MXJM3APLG3IPBFE4SO3WC7USSHFGJQKTUO7X66UIF4DAHLPV23L"
    // let peer key = "f4ded75a662ed6b140d1354964f24f1ed3db986e96c99827f242940b049b36d3"

    let peer_1_id = fetch_peer_id_from_account(
        "GDU22QMFGQVYYGAKD3UQQUFZVPUZMOKHNNZV3U4TOPOGW545GQS4R2IR".to_string(),
        StellarNetwork::Testnet,
    )
    .await?;

    println!("peer id: {:?}", peer_1_id);

    let _ = Client::new(&stellar_secret, StellarNetwork::Testnet)?;

    let psk = get_psk(&Path::new("."))?;
    let mut host = Libp2pHost::new(None, psk, None, None).await?;

    host.connect_to_relay(relay_addr.to_string()).await?;

    let master = SignerMaster::new(host.swarm.behaviour_mut());

    host.run().await.unwrap();

    Ok(())
}
