use crate::config::load_config;
use crate::index;
use colored::Colorize;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use crate::meta::read_meta;

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
    pub build_system: BuildSystem,
    pub depends: Vec<String>,
    pub configure_args: Vec<String>,
    pub multilib_support: bool,
    pub multilib_configure_args: Vec<String>,
}

#[derive(Debug)]
pub enum BuildSystem {
    Autotools,
    Cmake,
    Meson,
    Cargo,
    Python,
    Make,
    Manual {
        build_commands: Vec<String>,
        install_commands: Vec<String>,
    },
}

fn string_or_array<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    let value: Option<StringOrVec> = Option::deserialize(deserializer)?;
    Ok(match value {
        None => Vec::new(),
        Some(StringOrVec::Vec(v)) => v.into_iter().filter(|s| !s.is_empty()).collect(),
        Some(StringOrVec::String(s)) => {
            if s.is_empty() {
                Vec::new()
            } else if s.contains(" && ") {
                s.split(" && ")
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            } else {
                let parts: Vec<String> = s
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                if parts.len() > 1 { parts } else { vec![s.trim().to_string()] }
            }
        }
    })
}

fn bool_or_string<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrString {
        Bool(bool),
        String(String),
    }

    match Option::deserialize(deserializer)? {
        None => Ok(false),
        Some(BoolOrString::Bool(b)) => Ok(b),
        Some(BoolOrString::String(s)) => Ok(s.eq_ignore_ascii_case("true")),
    }
}

#[derive(Debug, Deserialize, Default)]
struct RawPackageSection {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    source: String,
}

#[derive(Debug, Deserialize, Default)]
struct RawBuildSection {
    #[serde(default)]
    system: String,
    #[serde(default, deserialize_with = "string_or_array")]
    depends: Vec<String>,
    #[serde(default, deserialize_with = "string_or_array")]
    configure_args: Vec<String>,
    #[serde(default, deserialize_with = "string_or_array")]
    build_commands: Vec<String>,
    #[serde(default, deserialize_with = "string_or_array", alias = "install_command")]
    install_commands: Vec<String>,
    #[serde(default, deserialize_with = "bool_or_string")]
    multilib_support: bool,
    #[serde(default, deserialize_with = "string_or_array")]
    multilib_configure_args: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawToml {
    #[serde(default)]
    package: RawPackageSection,
    #[serde(default)]
    build: RawBuildSection,
}

pub fn fetch_package(pkg_name: &str) -> Result<String, String> {
    let config = load_config();

    let local_path = format!("{}.toml", pkg_name);
    if Path::new(&local_path).exists() {
        return Ok(local_path);
    }

    let (atom, source) = index::resolve_with_source(pkg_name, &config)?;
    let dest = format!("/tmp/rad/tomls/{}.toml", atom);
    if let Some(parent) = Path::new(&dest).parent() {
        fs::create_dir_all(parent).unwrap();
    }

    if source.is_local {
        let src_path = format!("{}/{}.toml", source.base, atom);
        fs::copy(&src_path, &dest).map_err(|e| format!("couldn't find '{}' in {}: {}", atom, source.base, e))?;
    } else {
        let url = format!("{}/{}.toml", source.base, atom);
        let status = Command::new("wget")
            .args(["-q", "-O", &dest, &url])
            .status()
            .map_err(|e| format!("wget failed: {}", e))?;
        if !status.success() {
            return Err(format!("couldn't find '{}' in {}", atom, source.base));
        }
    }
    Ok(dest)
}

pub fn parse_package(path: &str) -> Result<Package, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;

    let raw: RawToml = toml::from_str(&content)
        .map_err(|e| format!("invalid toml in {}: {}", path, e))?;

    let RawPackageSection {
        name,
        version,
        description,
        source,
    } = raw.package;

    let RawBuildSection {
        system: build_system_str,
        depends,
        configure_args,
        build_commands,
        install_commands,
        multilib_support,
        multilib_configure_args,
    } = raw.build;

    let build_system = match build_system_str.as_str() {
        "autotools" => BuildSystem::Autotools,
        "cmake" => BuildSystem::Cmake,
        "meson" => BuildSystem::Meson,
        "cargo" => BuildSystem::Cargo,
        "python" => BuildSystem::Python,
        "make" => BuildSystem::Make,
        "manual" => {
            if build_commands.is_empty() {
                return Err("manual build system requires 'build_commands' field".to_string());
            }
            if install_commands.is_empty() {
                return Err("manual build system requires 'install_commands' field".to_string());
            }
            BuildSystem::Manual {
                build_commands,
                install_commands,
            }
        }
        other => return Err(format!("unknown build system: '{}'", other)),
    };

    if name.is_empty() || source.is_empty() {
        return Err("name and source are required fields in package".to_string());
    }

    Ok(Package {
        name,
        version,
        description,
        source,
        build_system,
        depends,
        configure_args,
        multilib_support,
        multilib_configure_args,
    })
}

pub fn package_info(pkg_name: &str, local: bool, processing: &mut HashSet<String>) {
    processing.insert(pkg_name.to_string());

    let config = load_config();
    
    let rad_path = if local {
        let path = if pkg_name.ends_with(".toml") { pkg_name.to_string() } else { format!("{}.toml", pkg_name) };
        if !Path::new(&path).exists() {
            eprintln!("[rad] {} local package file not found: {}", "error:".red(), path);
            processing.remove(pkg_name);
            return;
        }
        path
    } else {
        match fetch_package(pkg_name) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[rad] {} {}", "error:".red(), e);
                processing.remove(pkg_name);
                return;
            }
        }
    };

    let pkg = match parse_package(&rad_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[rad] {} {}", "parse error:".red(), e);
            processing.remove(pkg_name);
            return;
        }
    };

    let (atom, source_desc) = if local {
        (
            format!("local/{}", pkg.name),
            format!("local file: {}", rad_path.yellow()),
        )
    } else {
        match index::resolve_with_source(pkg_name, &config) {
            Ok((a, source)) => {
                let desc = if source.is_local {
                    format!("local overlay: {}", source.base)
                } else if source.base == config.repo.url {
                    format!("main repository: {}", source.base)
                } else {
                    format!("remote overlay: {}", source.base)
                };
                (a, desc)
            }
            Err(e) => {
                eprintln!("[rad] {} {}", "error:".red(), e);
                processing.remove(pkg_name);
                return;
            }
        }
    };

    let meta = read_meta(&atom);

    let installed_version_msg = if let Some(m) = meta {
        let meta_file_path = format!("/var/lib/rad/meta/{}.toml", atom);
        if Path::new(&meta_file_path).exists() {
            if m.version == pkg.version {
                format!(" = {}", m.version.green())
            } else {
                format!(" > {}", m.version.red())
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    println!(
        "[rad] Info about {}:\n  \
        - Description: {}\n  \
        - Package origin: {}\n  \
        - Package source: {}\n  \
        - Version: {}{}",
        atom.yellow(),
        pkg.description,
        source_desc,
        pkg.source,
        pkg.version,
        installed_version_msg,
    );
}