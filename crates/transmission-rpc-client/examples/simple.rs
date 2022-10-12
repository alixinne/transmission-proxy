use std::borrow::Cow;
use transmission_rpc_client::types::TorrentGet;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut client =
        transmission_rpc_client::client::Client::new("http://localhost:9091/transmission/rpc")?;

    println!(
        "{:#?}",
        client
            .torrent_get(TorrentGet {
                fields: vec![Cow::Borrowed("id"), Cow::Borrowed("trackerStats")],
                ..Default::default()
            })
            .await
    );

    Ok(())
}
