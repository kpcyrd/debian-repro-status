mod args;
mod dpkg;
mod errors;

use crate::args::Args;
use crate::errors::*;
use clap::Parser;
use colored::Colorize;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use indicatif::ProgressBar;
use rebuilderd_common::api::v0::{PkgRelease as RebuilderdPackage, Status};
use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;
use tokio::fs;

const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const READ_TIMEOUT: Duration = Duration::from_secs(180);

const MAX_CONCURRENT_REQUESTS: usize = 4;

fn default_arch_rebuilderd(arch: &str) -> String {
    format!("https://reproduce.debian.net/{arch}")
}

async fn rebuilderd_query_pkgs(
    args: &Args,
    progress_bar: &ProgressBar,
) -> Result<BTreeMap<String, Vec<RebuilderdPackage>>> {
    let http = reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()?;

    let responses = if let Some(path) = &args.rebuilderd_query_output {
        let buf = fs::read(&path).await.with_context(|| {
            anyhow!("Failed to read rebuilderd query output from file: {path:?}")
        })?;
        vec![serde_json::from_slice(&buf)?]
    } else {
        let mut endpoints = if !args.rebuilderd.is_empty() {
            args.rebuilderd
                .iter()
                .map(|url| url.trim_end_matches('/').to_string())
                .collect()
        } else if let Some(arch) = &args.architecture {
            // Use the default `reproduce.debian.net` instance,
            // with `all` and the given architecture
            VecDeque::from([
                default_arch_rebuilderd(arch),
                default_arch_rebuilderd("all"),
            ])
        } else {
            // Query dpkg for relevant architectures
            let native = dpkg::print_architecture().await?;
            let foreign = dpkg::print_foreign_architectures().await?;

            // Use the native architecture, `all` and any foreign ones
            let arches_iter = [native.as_str(), "all"]
                .into_iter()
                .chain(foreign.iter().map(|s| s.as_str()));

            // Derive `reproduce.debian.net` urls for each one
            arches_iter.map(default_arch_rebuilderd).collect()
        };

        let mut tasks = FuturesUnordered::new();
        let mut responses = Vec::new();

        while !endpoints.is_empty() || !tasks.is_empty() {
            // Spawn more tasks if there's space
            while tasks.len() < MAX_CONCURRENT_REQUESTS
                && let Some(endpoint) = endpoints.pop_front()
            {
                let url = format!("{endpoint}/api/v0/pkgs/list");
                let http = http.clone();
                tasks.push(async move {
                    let response = http
                        .get(url.as_str())
                        .send()
                        .await
                        .with_context(|| anyhow!("Failed to send http request: {url:?}"))?
                        .error_for_status()
                        .with_context(|| anyhow!("Failed to complete http request: {url:?}"))?
                        .json::<Vec<RebuilderdPackage>>()
                        .await
                        .with_context(|| anyhow!("Failed to parse http response: {url:?}"))?;
                    Result::<_, Error>::Ok(response)
                });
            }

            // Update progress status
            let done = responses.len();
            let total = responses.len() + tasks.len() + endpoints.len();
            progress_bar.set_message(format!("Retrieving packages... ({done}/{total})"));

            // Wait for the next response
            if let Some(pkgs) = tasks.next().await {
                responses.push(pkgs?);
            }
        }

        responses
    };

    let mut pkgs = BTreeMap::<_, Vec<_>>::new();
    for response in responses {
        for pkg in response {
            pkgs.entry(pkg.name.clone()).or_default().push(pkg);
        }
    }
    Ok(pkgs)
}

macro_rules! info {
    ($($arg:tt)*) => {{
        eprint!(" {} {} > ",
            "INFO ".green(),
            env!("CARGO_PKG_NAME").bold(),
        );
        eprintln!($($arg)*);
    }};
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(80));

    let (installed, reproduced) = tokio::try_join!(
        dpkg::query_packages(&args),
        rebuilderd_query_pkgs(&args, &progress_bar),
    )?;

    progress_bar.finish_and_clear();

    let mut negatives = 0;
    for pkg in &installed {
        let status_list = reproduced
            .get(&pkg.name)
            .into_iter()
            .flatten()
            .filter(|r| r.architecture == pkg.architecture)
            .filter(|r| r.version == pkg.version)
            .map(|r| r.status)
            .collect::<Vec<_>>();

        let status = if status_list.contains(&Status::Good) {
            Status::Good
        } else if status_list.contains(&Status::Bad) {
            Status::Bad
        } else {
            Status::Unknown
        };

        if status != Status::Good {
            negatives += 1;
        }

        if let Some(filter) = &args.filter
            && *status != *filter
        {
            continue;
        }

        if !args.summary {
            let icon = match status {
                Status::Good => "+".green(),
                Status::Bad => "-".red(),
                Status::Unknown => "?".yellow(),
            };
            println!(
                "[{icon}] {} {} {} {}",
                pkg.name,
                pkg.architecture,
                pkg.version,
                status.fancy()
            );
        }
    }

    if installed.is_empty() {
        eprintln!("Warning: No packages found.");
    } else {
        match negatives {
            0 => info!("All packages have been reproduced!"),
            1 => info!(
                "1/{} package could {} be reproduced.{}",
                installed.len(),
                "not".bold(),
                String::from(if installed.len() > 1 {
                    " Almost there."
                } else {
                    ""
                }),
            ),
            _ => info!(
                "{}/{} packages could {} be reproduced.",
                negatives,
                installed.len(),
                "not".bold(),
            ),
        }
        info!(
            "Your system has {:.2}% been reproduced.",
            ((installed.len() - negatives) as f64 / installed.len() as f64) * 100.
        )
    }

    Ok(())
}
