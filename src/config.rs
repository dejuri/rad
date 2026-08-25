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
    #[serde(default)]
    pub overlays: Vec<String>,
}

fn expand_home(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            let home_str = home.to_string_lossy();
            return path.replacen('~', &home_str, 1);
        }
    }
    path.to_string()
}

pub fn load_config() -> Config {
    let path = "/etc/rad/config.toml";
    match fs::read_to_string(path) {
        Ok(content) => {
            match toml::from_str::<Config>(&content) {
                Ok(mut config) => {
                    config.build.bin_cache_dir = expand_home(&config.build.bin_cache_dir);
                    config.repo.overlays = config.repo.overlays
                        .into_iter()
                        .map(|p| expand_home(&p))
                        .collect();
                    config
                }
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