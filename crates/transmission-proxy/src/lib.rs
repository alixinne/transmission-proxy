use std::{num::NonZeroUsize, path::PathBuf};

use clap::Parser;
use color_eyre::eyre;
use hyper::Uri;
use rand::Rng;
use tracing::{span, warn, Level};

mod acl;
mod auth;
mod config;
mod error;
mod rpc;
mod server;
pub mod torrent;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Address and port to listen on
    #[clap(
        long,
        default_value = "http://localhost:3000/transmission",
        env = "TRANSMISSION_PROXY_BIND"
    )]
    pub bind: Uri,

    /// Public url this proxy is accessible at
    #[clap(long, env = "TRANSMISSION_PROXY_PUBLIC_URL")]
    pub public_url: Option<Uri>,

    /// Root path for static assets
    #[clap(long, default_value = "public", env = "TRANSMISSION_PROXY_SERVE_ROOT")]
    pub serve_root: PathBuf,

    /// Upstream Transmission daemon
    #[clap(
        long,
        default_value = "http://localhost:9091",
        env = "TRANSMISSION_PROXY_UPSTREAM"
    )]
    pub upstream: Uri,

    /// Number of worker threads
    #[clap(long, default_value = "1", env = "TRANSMISSION_PROXY_WORKER_THREADS")]
    pub worker_threads: NonZeroUsize,

    /// Log level
    #[clap(long, default_value = "info", env = "TRANSMISSION_PROXY_LOG")]
    pub log: String,

    /// Path to the configuration file
    #[clap(
        long,
        default_value = "transmission-proxy.yaml",
        env = "TRANSMISSION_PROXY_CONFIG"
    )]
    pub config: PathBuf,

    /// Secret key for signing JWTs
    #[clap(long, default_value = "", env = "TRANSMISSION_PROXY_SECRET_KEY")]
    pub secret_key: String,
}

impl Args {
    pub fn public_url(&self) -> Uri {
        self.public_url.clone().unwrap_or_else(|| self.bind.clone())
    }
}

pub async fn run(mut args: Args) -> eyre::Result<()> {
    // Parse configuration
    let config: config::Config = {
        let span = span!(Level::INFO, "config", config = %args.config.display());
        let _guard = span.enter();

        let f = std::fs::File::open(&args.config)?;
        serde_yaml::from_reader(f)?
    };

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

    server::run(args, config).await
}
