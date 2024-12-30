mod args;
mod dpkg;
mod errors;

use crate::args::Args;
use crate::errors::*;
use clap::Parser;
use colored::Colorize;
use indicatif::ProgressBar;
use rebuilderd_common::{PkgRelease as RebuilderdPackage, Status};
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::fs;

const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

async fn rebuilderd_query_pkgs(args: &Args) -> Result<BTreeMap<String, Vec<RebuilderdPackage>>> {
    let response = if let Some(path) = &args.rebuilderd_query_output {
        let buf = fs::read(&path).await.with_context(|| {
            anyhow!("Failed to read rebuilderd query output from file: {path:?}")
        })?;
        serde_json::from_slice(&buf)?
    } else {
        let endpoint = if let Some(url) = &args.rebuilderd {
            url.trim_end_matches('/').to_string()
        } else {
            let arch = dpkg::print_architecture(args).await?;
            format!("https://{arch}.reproduce.debian.net")
        };

        let url = format!("{endpoint}/api/v0/pkgs/list");

        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()?;

        http.get(url.as_str())
            .send()
            .await
            .with_context(|| anyhow!("Failed to send http request: {url:?}"))?
            .error_for_status()
            .with_context(|| anyhow!("Failed to complete http request: {url:?}"))?
            .json::<Vec<RebuilderdPackage>>()
            .await
            .with_context(|| anyhow!("Failed to parse http response: {url:?}"))?
    };

    let mut pkgs = BTreeMap::<_, Vec<_>>::new();
    for pkg in response {
        pkgs.entry(pkg.name.clone()).or_default().push(pkg);
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
    progress_bar.set_message("Retrieving packages...");

    let (installed, reproduced) =
        tokio::try_join!(dpkg::query_packages(&args), rebuilderd_query_pkgs(&args),)?;

    progress_bar.finish_and_clear();

    let mut negatives = 0;
    for pkg in &installed {
        let status = if let Some(reproduced) = reproduced.get(&pkg.name) {
            reproduced
                .iter()
                .filter(|r| r.architecture == pkg.architecture)
                .filter(|r| r.version == pkg.version)
                .map(|r| r.status)
                .next()
                .unwrap_or(Status::Unknown)
        } else {
            Status::Unknown
        };

        if status != Status::Good {
            negatives += 1;
        }

        if let Some(filter) = &args.filter {
            if *status != *filter {
                continue;
            }
        }

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

    if installed.is_empty() {
        eprintln!("Warning: No packages found.");
    } else {
        match negatives {
            0 => info!("All packages are reproducible!"),
            1 => info!(
                "1/{} package is {} reproducible.{}",
                installed.len(),
                "not".bold(),
                String::from(if installed.len() > 1 {
                    " Almost there."
                } else {
                    ""
                }),
            ),
            _ => info!(
                "{}/{} packages are {} reproducible.",
                negatives,
                installed.len(),
                "not".bold(),
            ),
        }
        info!(
            "Your system is {:.2}% reproducible.",
            ((installed.len() - negatives) as f64 / installed.len() as f64) * 100.
        )
    }

    Ok(())
}
