use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version)]
pub struct Args {
    /// Use this architecture instead of auto-detecting
    #[arg(long)]
    pub architecture: Option<String>,
    /// Change rebuilderd url endpoint
    #[arg(short = 'H', long)]
    pub rebuilderd: Option<String>,
    /// Read dpkg-query output from file instead of running the binary
    #[arg(long)]
    pub dpkg_query_output: Option<PathBuf>,
    /// Read rebuilderd package list from file instead of querying over the network
    #[arg(long)]
    pub rebuilderd_query_output: Option<PathBuf>,
}
