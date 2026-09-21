use crate::config::load_config;
use crate::index;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use colored::Colorize;
use serde::Deserialize;
use regex::Regex;
use crate::meta::read_meta;
use crate::version;
use crate::version::PackageSpec;
use crate::version::Status;

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub version: Vec<String>,
    pub description: String,
    pub source: Vec<String>,
    pub unfree: bool,
    pub build_system: BuildSystem,
    pub depends: Vec<String>,
    pub configure_args: Vec<String>,
    pub multilib_support: bool,
    pub multilib_configure_args: Vec<String>,
    pub post_install: Vec<String>,
    pub verbose: bool,
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
            let s = s.trim();
            if s.is_empty() {
                Vec::new()
            } else if s.contains(" && ") {
                s.split(" && ")
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            } else if s.contains(',') {
                s.split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            } else {
                vec![s.to_string()]
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
pub(crate) struct RawPackageSection {
    #[serde(default)]
    name: String,
    #[serde(default, deserialize_with = "string_or_array", alias = "versions")]
    version: Vec<String>,
    #[serde(default)]
    description: String,
    #[serde(default, deserialize_with = "string_or_array", alias = "sources")]
    source: Vec<String>,
    #[serde(default)]
    unfree: bool,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct RawBuildSection {
    #[serde(default)]
    system: String,
    #[serde(default, deserialize_with = "string_or_array")]
    depends: Vec<String>,
    #[serde(default, deserialize_with = "string_or_array")]
    configure_args: Vec<String>,
    #[serde(default, deserialize_with = "string_or_array", alias = "build_command")]
    build_commands: Vec<String>,
    #[serde(default, deserialize_with = "string_or_array", alias = "install_command")]
    install_commands: Vec<String>,
    #[serde(default, deserialize_with = "bool_or_string")]
    multilib_support: bool,
    #[serde(default, deserialize_with = "string_or_array")]
    multilib_configure_args: Vec<String>,
    #[serde(default, deserialize_with = "string_or_array")]
    post_install: Vec<String>,
    #[serde(default, deserialize_with = "bool_or_string")]
    verbose: bool,
}

#[derive(Debug, Deserialize, Default)]
struct RawToml {
    #[serde(default)]
    package: RawPackageSection,
    #[serde(default)]
    build: RawBuildSection,
}
impl Package {

    /// Get source for the corresponding version
    pub fn get_source_for_version(&self, target_ver: &str) -> String {
        if let Some(idx) = self.version.iter().position(|v| v == target_ver) {
            self.source
                .get(idx)
                .cloned()
                .unwrap_or_else(|| self.source.first().cloned().unwrap_or_default())
        } else {
            self.source.first().cloned().unwrap_or_default()
        }
    }
}
impl TryFrom<RawToml> for Package {
    type Error = String;

    fn try_from(raw: RawToml) -> Result<Self, Self::Error> {
        let RawPackageSection {
            name,
            version,
            description,
            source,
            unfree,
        } = raw.package;

        if name.is_empty() {
            return Err("field 'name' is required in [package]".to_string());
        }

        if version.is_empty() {
            return Err("field 'version' (or 'versions') is required in [package]".to_string());
        }
        // Check source: needed only if unfree == false
        if !unfree && source.is_empty() {
            return Err("field 'source' is required in [package] when unfree is false".to_string());
        }

        let RawBuildSection {
            system: build_system_str,
            depends,
            configure_args,
            build_commands,
            install_commands,
            multilib_support,
            multilib_configure_args,
            post_install,
            verbose,
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

        Ok(Package {
            name,
            version,
            description,
            source,
            unfree,
            build_system,
            depends,
            configure_args,
            multilib_support,
            multilib_configure_args,
            post_install,
            verbose,
        })
    }
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

pub fn interpolate_cmd(cmd: &str, current_pkg: &Package, current_version: &str) -> String {
    let result = cmd
        .replace("{version}", current_version)
        .replace("{name}", &current_pkg.name);

    let re = Regex::new(r"\{([a-zA-Z0-9_\-]+/[a-zA-Z0-9_\-]+)\.([a-zA-Z0-9_]+)\}").unwrap();

    re.replace_all(&result, |caps: &regex::Captures| {
        let atom = &caps[1];
        let field = &caps[2];

        match field {
            "version" => {
                if let Some(installed_meta) = crate::meta::read_meta(atom) {
                    installed_meta.version
                } else {
                    eprintln!("[rad] warning: package '{}' is not installed for placeholder substitution", atom);
                    format!("{}-NOT-INSTALLED", atom)
                }
            }
            _ => caps[0].to_string(),
        }
    }).to_string()
}
pub fn parse_package(path: &str) -> Result<Package, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;

    let raw: RawToml = toml::from_str(&content)
        .map_err(|e| format!("invalid toml in {}: {}", path, e))?;

    raw.try_into()
}

pub fn package_info(pkg_name: &str, local: bool, processing: &mut HashSet<String>) {
    
    // `name@version` (a local toml path is taken as it is)
    let spec = if local {
        PackageSpec { name: pkg_name, version: None }
    } else {
        PackageSpec::parse(pkg_name)
    };
    let pkg_name = spec.name;

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
            return
        }
    };

    let (atom, source_desc) = if local {
        (
            format!("local/{}", pkg.name),
            format!("local file: {}", rad_path),
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
    let pkg_latest_version = version::latest(&pkg.version)
        .cloned()
        .unwrap_or_default();
    let target_version = version::latest(&pkg.version)
        .cloned()
        .unwrap_or_default();
    let installed_version_msg = match meta {
        Some(m) if Path::new(&format!("/var/lib/rad/meta/{}.toml", atom)).exists() => {
            let status = version::status(&m.version, &pkg.version);
            let shown = if pkg.version.contains(&m.version) {
                format!(" = {}", m.version).green()
            } else if status == Status::Outdated {
                format!(" > {}", m.version).red()
            } else {
                format!(" > {}", m.version).yellow()
            };
            if status == Status::Upgradable {
                format!("{} {}", shown, "[U]".yellow().bold())
            } else {
                shown.to_string()
            }
        }
        _ => String::new(),
    };

    let available_versions = version::sorted_desc(&pkg.version).join(", ");
    let current_source = pkg.get_source_for_version(&target_version);
    println!(
        "[rad] Info about {}{}:\n  \
        - Description: {}\n  \
        - Package origin: {}\n\
        {}  - Version: {}{}; Available: {}",
        atom.yellow(),
        if pkg.unfree { " [PROPRIETARY]".red() } else { "".red() },
        pkg.description,
        source_desc,
        if !pkg.unfree { format!("  - Package source: {}\n", current_source) } else { "".to_string() },
        pkg_latest_version,
        installed_version_msg,
        format!("{}", available_versions.trim_end_matches('\n'))
    );
}