use std::{num::NonZeroUsize, path::PathBuf, str::FromStr};

use clap::Parser;
use color_eyre::eyre;
use hyper::Uri;
use rand::Rng;
use tracing::{span, warn, Level};
use tracing_subscriber::{prelude::*, util::SubscriberInitExt, EnvFilter};

mod acl;
mod auth;
mod config;
mod error;
mod ext;
mod proxy;
mod rpc;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Address and port to listen on
    #[clap(
        long,
        default_value = "http://localhost:3000/transmission",
        env = "TRANSMISSION_PROXY_BIND"
    )]
    bind: Uri,

    /// Root path for static assets
    #[clap(long, default_value = "public", env = "TRANSMISSION_PROXY_SERVE_ROOT")]
    serve_root: PathBuf,

    /// Upstream Transmission daemon
    #[clap(
        long,
        default_value = "http://localhost:9091",
        env = "TRANSMISSION_PROXY_UPSTREAM"
    )]
    upstream: Uri,

    /// Number of worker threads
    #[clap(long, default_value = "1", env = "TRANSMISSION_PROXY_WORKER_THREADS")]
    worker_threads: NonZeroUsize,

    /// Log level
    #[clap(long, default_value = "info", env = "TRANSMISSION_PROXY_LOG")]
    log: String,

    /// Path to the configuration file
    #[clap(
        long,
        default_value = "transmission-proxy.yaml",
        env = "TRANSMISSION_PROXY_CONFIG"
    )]
    config: PathBuf,

    /// Secret key for signing JWTs
    #[clap(long, default_value = "", env = "TRANSMISSION_PROXY_SECRET_KEY")]
    secret_key: String,
}

async fn run(args: Args) -> eyre::Result<()> {
    // Parse configuration
    let config: config::Config = {
        let span = span!(Level::INFO, "config", config = %args.config.display());
        let _guard = span.enter();

        let f = std::fs::File::open(&args.config)?;
        serde_yaml::from_reader(f)?
    };

    proxy::run(args, config).await
}

fn main() -> eyre::Result<()> {
    let mut args = Args::parse();

    // Setup eyre
    color_eyre::install()?;

    // Setup tracing
    let _subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_str(&args.log)?)
        .finish()
        .with(tracing_error::ErrorLayer::default())
        .try_init()?;

    // Generate key if needed
    if args.secret_key.is_empty() {
        const LEN: usize = 32;
        let mut rng = rand::thread_rng();
        args.secret_key.reserve(LEN);
        for _ in 0..LEN {
            args.secret_key.push(rng.gen_range('0'..'z'))
        }

        warn!("generated secret key because none was specified");
    }

    // Start runtime
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(args.worker_threads.into())
        .enable_all()
        .build()
        .expect("failed to create tokio runtime")
        .block_on(run(args))
}
