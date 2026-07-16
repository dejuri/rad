use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub arch: ArchConfig,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub repo: RepoConfig,
}

#[derive(Deserialize, Default)]
pub struct ArchConfig {
    #[serde(default)]
    pub multilib: bool,
}

#[derive(Deserialize, Default)]
pub struct BuildConfig {
    #[serde(default)]
    pub makeopts: u8,
    pub ask: bool,
    pub bin_cache_dir: String,
}

#[derive(Deserialize, Default)]
pub struct RepoConfig {
    #[serde(default)]
    pub url: String,
}

pub fn load_config() -> Config {
    let path = "/etc/rad/config.toml";
    match fs::read_to_string(path) {
        Ok(content) => {
            match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Config error! Check your config, parser can't read it: {}", e);
                    Config::default()
                }
            }
        }
        Err(_) => {
            Config::default()
        }
    }
}