use std::path::Path;

use tf_libp2p::{get_psk, Libp2pHost};
use tf_stellar::{Client, Network};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let _ = Client::new("seed", Network::new_public())?;

    let psk = get_psk(&Path::new("."))?;
    let host = Libp2pHost::new(None, psk).await?;
    println!("host created");
    host.start(
        String::from("/ip4/127.0.0.1/tcp/4001"),
        "12D3KooWSp9rRj1yG5jbz754EkpGVfGP7GB9smgMCjXczcDACRC3",
    )
    .await?;

    Ok(())
}
