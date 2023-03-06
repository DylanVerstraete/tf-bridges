use std::env;
use std::path::Path;
use tf_libp2p::{get_psk, Libp2pHost};
// use tf_stellar::{Client, Network};
use pretty_env_logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    // let _ = Client::new("seed", Network::new_public())?;

    let args: Vec<String> = env::args().collect();

    let relay_addr = &args[1];

    let psk = get_psk(&Path::new("."))?;
    let mut host = Libp2pHost::new(None, psk).await?;

    host.connect_to_relay(relay_addr.to_string()).await?;

    host.run().await.unwrap();

    Ok(())
}
