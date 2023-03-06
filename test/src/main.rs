use std::env;
use std::path::Path;
use tf_libp2p::{get_psk, Libp2pHost};
// use tf_stellar::{Client, Network};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let _ = Client::new("seed", Network::new_public())?;

    let args: Vec<String> = env::args().collect();

    let relay_addr = &args[1];
    let relay_peer_id = &args[2];

    let psk = get_psk(&Path::new("."))?;
    let host = Libp2pHost::new(None, psk).await?;
    println!("host created");

    // tokio::spawn(async move {
    //     host.start(30334).await;
    // });

    host.connect_to_relay(relay_addr.to_string(), &relay_peer_id)
        .await?;

    Ok(())
}
