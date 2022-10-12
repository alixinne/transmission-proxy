use std::str::FromStr;

use clap::Parser;
use color_eyre::eyre;

use tracing_subscriber::{prelude::*, util::SubscriberInitExt, EnvFilter};

use transmission_proxy::Args;

fn main() -> eyre::Result<()> {
    let args = Args::parse();

    // Setup eyre
    color_eyre::install()?;

    // Setup tracing
    let _subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_str(&args.log)?)
        .finish()
        .with(tracing_error::ErrorLayer::default())
        .try_init()?;

    // Start runtime
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(args.worker_threads.into())
        .enable_all()
        .build()
        .expect("failed to create tokio runtime")
        .block_on(transmission_proxy::run(args))
}
