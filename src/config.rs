use crate::bang::Bang;
pub use crate::cli::{Cli, SubCommand};
use serde::Deserialize;
use std::net::IpAddr;

/// Configuration read from the file.
#[derive(Deserialize, Debug, Default)]
pub struct FileConfig {
    pub(crate) port: Option<u16>,
    pub(crate) ip: Option<IpAddr>,
    pub(crate) bangs_url: Option<String>,
    pub(crate) default_search: Option<String>,
    pub(crate) bangs: Option<Vec<Bang>>,
}

/// Configuration read from the CLI.
#[derive(Debug, Default)]
pub struct Config {
    pub(crate) port: Option<u16>,
    pub(crate) ip: Option<IpAddr>,
    pub(crate) bangs_url: Option<String>,
    pub(crate) default_search: Option<String>,
}

/// Final application configuration.
#[derive(Clone)]
pub struct AppConfig {
    pub(crate) port: u16,
    pub(crate) ip: IpAddr,
    pub(crate) bangs_url: String,
    pub(crate) default_search: String,
    pub(crate) bangs: Option<Vec<Bang>>,
}

impl Config {
    /// Merge CLI configuration with an optional file configuration.
    /// CLI options take precedence over file values.
    #[allow(dead_code)]
    pub(crate) fn merge(self, file: Option<FileConfig>) -> AppConfig {
        let file = file.unwrap_or(FileConfig {
            port: None,
            ip: None,
            bangs_url: None,
            default_search: None,
            bangs: None,
        });
        AppConfig {
            port: self.port.or(file.port).unwrap_or(3000),
            ip: self
                .ip
                .or(file.ip)
                .unwrap_or_else(|| "0.0.0.0".parse().unwrap()),
            bangs_url: self
                .bangs_url
                .or(file.bangs_url)
                .unwrap_or_else(|| "https://duckduckgo.com/bang.js".to_string()),
            default_search: self
                .default_search
                .or(file.default_search)
                .unwrap_or_else(|| "https://www.startpage.com/do/dsearch?query={}".to_string()),
            bangs: file.bangs,
        }
    }
}

impl FileConfig {
    /// Merge CLI configuration with an optional file configuration.
    /// CLI options take precedence over file values.
    pub(crate) fn merge(self, config: Config) -> AppConfig {
        AppConfig {
            port: config.port.or(self.port).unwrap_or(3000),
            ip: config
                .ip
                .or(self.ip)
                .unwrap_or_else(|| "0.0.0.0".parse().unwrap()),
            bangs_url: config
                .bangs_url
                .or(self.bangs_url)
                .unwrap_or_else(|| "https://duckduckgo.com/bang.js".to_string()),
            default_search: config
                .default_search
                .or(self.default_search)
                .unwrap_or_else(|| "https://www.startpage.com/do/dsearch?query={}".to_string()),
            bangs: self.bangs,
        }
    }
}

impl From<Cli> for Config {
    fn from(cli: Cli) -> Self {
        match cli.command {
            Some(SubCommand::Serve { port, ip }) => Self {
                port,
                ip,
                bangs_url: cli.bangs_url,
                default_search: cli.default_search,
            },
            Some(SubCommand::Resolve { query: _ }) => Self {
                port: None,
                ip: None,
                bangs_url: cli.bangs_url,
                default_search: cli.default_search,
            },
            _ => Self::default(),
        }
    }
}
