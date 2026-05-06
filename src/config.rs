use std::fs;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub arch: ArchConfig,
        #[serde(default)]
    pub repo: RepoConfig,
}

#[derive(Deserialize, Default)]
pub struct ArchConfig {
    #[serde(default)]
    pub multilib: bool,
}

#[derive(Deserialize, Default)]
pub struct RepoConfig {
    #[serde(default)]
    pub url: String,
}

pub fn load_config() -> Config {
    let path = "/etc/rad/config.toml";
    if let Ok(content) = fs::read_to_string(path) {
        toml::from_str(&content).unwrap_or(Config { arch: ArchConfig::default(), repo: RepoConfig::default()  })
    } else {
        Config { arch: ArchConfig::default(), repo: RepoConfig::default() }
    }
}