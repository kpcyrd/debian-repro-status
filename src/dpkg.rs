use crate::args::Args;
use crate::errors::*;
use std::io::BufRead;
use std::process::Stdio;
use std::str::FromStr;
use tokio::fs;
use tokio::process::Command;

#[derive(Debug, PartialEq)]
pub struct DpkgPackage {
    pub status: String,
    pub name: String,
    pub architecture: String,
    pub version: String,
}

impl DpkgPackage {
    pub fn is_installed(&self) -> bool {
        self.status == "installed"
    }
}

impl FromStr for DpkgPackage {
    type Err = Error;

    fn from_str(line: &str) -> Result<DpkgPackage> {
        let Some((status, line)) = line.split_once(' ') else {
            bail!("Malformed dpkg output, could not locate pkg name delimiting space");
        };
        let Some((name, line)) = line.split_once(' ') else {
            bail!("Malformed dpkg output, could not locate architecture delimiting space");
        };
        let name = name.rsplit_once(':').map(|(name, _)| name).unwrap_or(name);
        let Some((architecture, version)) = line.split_once(' ') else {
            bail!("Malformed dpkg output, could not locate version delimiting space");
        };
        Ok(DpkgPackage {
            status: status.to_string(),
            name: name.to_string(),
            architecture: architecture.to_string(),
            version: version.to_string(),
        })
    }
}

pub async fn print_architecture() -> Result<String> {
    let exit = Command::new("dpkg")
        .args(["--print-architecture"])
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .await?;
    if !exit.status.success() {
        bail!(
            "Failed to query native architecture: exit={:?}",
            exit.status
        );
    }
    let output = exit.stdout.trim_ascii().to_owned();
    let output = String::from_utf8(output)?;
    Ok(output)
}

pub async fn print_foreign_architectures() -> Result<Vec<String>> {
    let exit = Command::new("dpkg")
        .args(["--print-foreign-architectures"])
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .await?;
    if !exit.status.success() {
        bail!(
            "Failed to query foreign architectures: exit={:?}",
            exit.status
        );
    }
    let output = exit.stdout.trim_ascii().to_owned();
    let output = String::from_utf8(output)?;
    let output = output.lines().map(String::from).collect();
    Ok(output)
}

pub async fn query_packages(args: &Args) -> Result<Vec<DpkgPackage>> {
    let output = if let Some(path) = &args.dpkg_query_output {
        fs::read(&path)
            .await
            .with_context(|| anyhow!("Failed to read dpkg-query output from file: {path:?}"))?
    } else {
        let exit = Command::new("dpkg-query")
            .args([
                "-f",
                "${db:Status-Status} ${binary:Package} ${Architecture} ${Version}\n",
                "-W",
            ])
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .await?;
        if !exit.status.success() {
            bail!("Failed to query installed debian packages: exit={exit:?}");
        }
        exit.stdout
    };

    let mut pkgs = Vec::new();
    for line in output.lines() {
        let line = line?;
        let pkg = line.parse::<DpkgPackage>()?;
        if pkg.is_installed() {
            pkgs.push(pkg);
        }
    }
    Ok(pkgs)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn parse_dpkg_query_line() {
        let line = "installed login amd64 1:4.16.0-2+really2.40.2-11";
        let pkg = DpkgPackage::from_str(line).unwrap();
        assert_eq!(
            pkg,
            DpkgPackage {
                status: "installed".to_string(),
                name: "login".to_string(),
                architecture: "amd64".to_string(),
                version: "1:4.16.0-2+really2.40.2-11".to_string(),
            }
        );
        assert!(pkg.is_installed());
    }

    #[test]
    fn parse_dpkg_query_arch_suffixed_line() {
        let line = "installed libudev1:amd64 amd64 257~rc3-1";
        let pkg = DpkgPackage::from_str(line).unwrap();
        assert_eq!(
            pkg,
            DpkgPackage {
                status: "installed".to_string(),
                name: "libudev1".to_string(),
                architecture: "amd64".to_string(),
                version: "257~rc3-1".to_string(),
            }
        );
    }

    #[test]
    fn parse_dpkg_query_removed() {
        let line = "config-files nginx-common all 1.26.3-2";
        let pkg = DpkgPackage::from_str(line).unwrap();
        assert_eq!(
            pkg,
            DpkgPackage {
                status: "config-files".to_string(),
                name: "nginx-common".to_string(),
                architecture: "all".to_string(),
                version: "1.26.3-2".to_string(),
            }
        );
        assert!(!pkg.is_installed());
    }
}
