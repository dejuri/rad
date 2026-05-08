use std::process::Command;
use std::path::Path;
use std::fs;
use colored::Colorize;
use std::collections::HashSet;
use crate::config::load_config;

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
        install_command: String,
    },
}

pub fn fetch_package(pkg_name: &str) -> Result<String, String> {
    let config = load_config();
    let local_path = format!("{}.toml", pkg_name);
    if Path::new(&local_path).exists() {
        // println!("[rad] using local {}", local_path);
        return Ok(local_path);
    }
    let url  = format!("{}/{}.toml", config.repo.url, pkg_name);
    let dest = format!("/tmp/rad/tomls/{}.toml", pkg_name);
    fs::create_dir_all("/tmp/rad/tomls").unwrap();
    // println!("[rad] fetching toml from {}...", url);
    let status = Command::new("wget")
        .args(["-q", "-O", &dest, &url])
        .status()
        .map_err(|e| format!("wget failed: {}", e))?;
    if !status.success() {
        return Err(format!("couldn't find package '{}' locally or in remote repo.", pkg_name));
    }
    Ok(dest)
}
pub fn parse_package(path: &str) -> Result<Package, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path, e))?;

    let mut name = String::new();
    let mut version = String::new();
    let mut description = String::new();
    let mut source = String::new();
    let mut build_system_str = String::new();
    let mut depends = Vec::new();
    let mut configure_args: Vec<String> = Vec::new();
    let mut build_commands: Vec<String> = Vec::new();
    let mut install_command = String::new();
    let mut multilib_support = false;
    let mut multilib_configure_args: Vec<String> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('[') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            let value = value.strip_prefix('"').and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value)
                .to_string();

            match key {
                "name"                    => name = value,
                "version"                 => version = value,
                "description"             => description = value,
                "source"                  => source = value,
                "system"                  => build_system_str = value,
                "depends"                 => {
                    depends = value.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                "configure_args"          => {
                    configure_args = value.split_whitespace()
                        .map(|s| s.to_string()).collect();
                }
                "build_commands"          => {
                    build_commands = vec![value];
                }
                "install_command" => install_command = value,
                "multilib_support" => multilib_support = value == "true",
                "multilib_configure_args" => {
                    multilib_configure_args = value.split_whitespace()
                        .map(|s| s.to_string()).collect();
                }
                _ => {}
            }
        }
    }

    let build_system = match build_system_str.as_str() {
        "autotools" => BuildSystem::Autotools,
        "cmake"     => BuildSystem::Cmake,
        "meson"     => BuildSystem::Meson,
        "cargo"     => BuildSystem::Cargo,
        "python"    => BuildSystem::Python,
        "make"      => BuildSystem::Make,
        "manual"    => {
            if build_commands.is_empty() {
                return Err("manual build system requires 'build_commands' field".to_string());
            }
            if install_command.is_empty() {
                return Err("manual build system requires 'install_command' field".to_string());
            }
            BuildSystem::Manual { build_commands, install_command }
        }
        other => return Err(format!("unknown build system: '{}'", other)),
    };

    if name.is_empty() || source.is_empty() {
        return Err("name and source are required fields in package".to_string());
    }

    Ok(Package {
        name, version, description, source, build_system,
        depends, configure_args, multilib_support, multilib_configure_args,
    })
}

pub fn package_info(pkg_name: &str, processing: &mut HashSet<String>) {
    processing.insert(pkg_name.to_string());
    let rad_path = match fetch_package(pkg_name) {
        Ok(p)  => p,
        Err(e) => { eprintln!("[rad] {} {}", "error:".red(), e); processing.remove(pkg_name); return; }
    };
    let pkg = match parse_package(&rad_path) {
        Ok(p)  => p,
        Err(e) => { eprintln!("[rad] {} {}", "parse error:".red(), e); processing.remove(pkg_name); return; }
    };
    println!("[rad] Info about {}{}:\n  \
    {}, \n  \
    Source of the package: {}, \n  \
    Version of the package: {}", pkg_name.yellow(),  if Path::new(&format!("{}.toml", pkg_name)).exists() { " (local)" } else { "" }, pkg.description, pkg.source, pkg.version);
}