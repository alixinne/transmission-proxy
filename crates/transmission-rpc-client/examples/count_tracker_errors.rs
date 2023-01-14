use std::{borrow::Cow, process::exit};
use transmission_rpc_client::types::TorrentGet;

struct Stats {
    /// Number of torrents with errors
    tracker_errors: i32,
    /// Total number of torrents
    total: i32,
}

async fn get_torrent_stats() -> anyhow::Result<Stats> {
    let mut client =
        transmission_rpc_client::client::Client::new("http://localhost:9091/transmission/rpc")?;

    let mut tracker_errors = 0;
    let mut total = 0;

    for torrent in &client
        .torrent_get(TorrentGet {
            fields: vec![
                Cow::Borrowed("id"),
                Cow::Borrowed("name"),
                Cow::Borrowed("trackerStats"),
            ],
            ..Default::default()
        })
        .await?
        .torrents
    {
        if !torrent
            .tracker_stats
            .as_ref()
            .unwrap()
            .iter()
            .any(|stat| stat.last_scrape_succeeded || stat.last_announce_succeeded)
        {
            tracker_errors += 1;
        } else {
            println!("{} has no errors", torrent.name);
        }

        total += 1;
    }

    Ok(Stats {
        tracker_errors,
        total,
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    if let Ok(Stats {
        tracker_errors,
        total,
    }) = get_torrent_stats().await
    {
        println!("Torrents with errors: {}", tracker_errors);
        println!("Total torrents: {}", total);

        if tracker_errors >= total / 2 {
            exit(1);
        }
    }

    Ok(())
}
