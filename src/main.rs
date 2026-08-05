use colored::Colorize;
use rad::config::*;
use rad::install::*;
use rad::package::*;
use rad::remove::*;
use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::env;

// Check for deps
fn is_there(name: &str) -> Result<(), String> {
    let status = Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| format!("{}", "install which first".blue()))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("not found"))
    }
}

// Parsing arguments
fn parse_short_flags(raw: &str) -> Option<(char, bool)> {
    if !raw.starts_with('-') || raw.starts_with("--") {
        return None;
    }
    let chars: Vec<char> = raw.chars().skip(1).collect();
    if chars.is_empty() {
        return None;
    }
    let local = chars.contains(&'l');
    let action: Vec<char> = chars.into_iter().filter(|&c| c != 'l').collect();
    if action.len() != 1 {
        return None;
    }
    Some((action[0], local))
}

// Main
fn main() {
    let config = load_config();
    let args: Vec<String> = env::args().collect();
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
    let prefix = "/usr";
    let overlays_formatted = config.repo.overlays
        .iter()
        .map(|s| format!("\n      {}", s.yellow()))
        .collect::<String>();

    let deps = vec!["cargo", "make", "cmake", "meson", "ninja", "pip", "tar", "unzip", "git", "wget", "sh"];
    if args.len() < 2 {
        eprintln!(
            "[rad] {} please specify a valid argument, use -h or --help",
            "error:".red()
        );
        return;
    }

    match (args[1].as_str(), parse_short_flags(args[1].as_str())) {

        // Help
        ("-h", _) | ("--help", _) => {
            println!(
                "{} v{}\n\n  \
                Usage: rad [command]\n\n  \
                Arguments:\n    \
                    -h, --help              print this menu\n    \
                    -V, --version           print rad version\n    \
                    -s, --sync              sync package index of current repository\n    \
                    -C, --clear-cache       clear rad cache and temporary files\n    \
                    -L, --list              list installed packages\n    \
                    -I, --info              info about rad on your system\n    \
                    -i, --install <pkg>     install a package\n    \
                    -b, --build <pkg>       build package source without installing\n    \
                    -f, --force <pkg>       force package installation\n    \
                    -r, --remove  <pkg>     remove a package\n    \
                    -P, --pkg-info <pkg>    info about specific package\n\n  \
                Options:\n    \
                    -l <path>               use local package (with -i, -f, -b and -P)\n\n  \
                Packages are searched in main repository and overlays\n    \
                Main repository: {}\n    \
                Overlays: {}",
                "Radian Automated TOML-packages Handler".bold(), version.yellow(), config.repo.url.yellow(), overlays_formatted.yellow()
            );
        }

        ("-I", _) | ("--info", _) => {
            
            // Start of info
            println!("[rad] info:\n\
            DEPENDENCIES");

            // Check for which
            let status = Command::new("which")
                .arg("which")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("  - which: {}", "found".green());
                }
                _ => {
                    println!("  - which: {}", "not found".red());
                }
            }   

            // Dependencies
            for i in deps {
                match is_there(i) {
                    Ok(_) => println!("  - {}: {}", i, "found".green()),
                    Err(e) => eprintln!("  - {}: {}", i, e.red()),
                }
            }

            // Config
            println!("CONFIG\n  \
                    ARCH\n    \
                        - multilib: {}\n  \
                    BUILD\n    \
                        - makeopts: {}\n    \
                        - ask: {}\n    \
                        - bin cache dir: {}\n  \
                    REPO\n    \
                        - url: {}\n    \
                        - overlays: {:?}",
                config.arch.multilib, config.build.makeopts, config.build.ask, config.build.bin_cache_dir, config.repo.url, config.repo.overlays
            );
        }

        ("-V", _) | ("--version", _) => println!(
            "rad - {}\n  version: {}",
            "Radian Automated TOML-packages Handler".bold(),
            version.yellow()
        ),

        (_, Some(('i', local))) => {
            let mut processing = HashSet::new();
            match args.get(2) {
                Some(name) => install_package(name, prefix, false, true, true, local, &mut processing),
                None => eprintln!("[rad] {} specify the package name", "error:".red()),
            }
        }

        (_, Some(('f', local))) => {
            let mut processing = HashSet::new();
            match args.get(2) {
                Some(name) => install_package(name, prefix, true, true, true, local, &mut processing),
                None => eprintln!("[rad] {} specify the package name", "error:".red()),
            }
        }

        (_, Some(('b', local))) => {
            let mut processing = HashSet::new();
            match args.get(2) {
                Some(name) => install_package(name, prefix, true, true, false, local, &mut processing),
                None => eprintln!("[rad] {} specify the package name", "error:".red()),
            }
        }

        (_, Some(('P', local))) => {
            let mut processing = HashSet::new();
            match args.get(2) {
                Some(name) => package_info(name, local, &mut processing),
                None => eprintln!("[rad] {} specify the package name", "error:".red()),
            }
        }

        ("-s", _) | ("--sync", _) => {
            println!("[rad] updating package index");
            let mut all_sources = vec![config.repo.url.clone()];
            all_sources.extend(config.repo.overlays.iter().cloned());

            for src in all_sources {
                if src.starts_with("http://") || src.starts_with("https://") {
                    match rad::index::refresh_overlay_index(&src) {
                        Ok(_) => println!("[rad] index updated: {}", src),
                        Err(e) => eprintln!("[rad] {} {} ({})", "error:".red(), e, src),
                    }
                } else {
                    println!("[rad] {} is local, nothing to sync", src);
                }
            }
        }

        ("-C", _) | ("--clear-cache", _) => {
            println!("[rad] clearing cache and build remains");
            clear_cache();
        }

        ("-r", _) | ("--remove", _) => match args.get(2) {
            Some(name) => {
                if let Err(e) = remove_package(name) {
                    eprintln!("[rad] {} {}", "removal error:".red(), e);
                }
            }
            None => eprintln!("[rad] {} specify the package name", "error:".red()),
        }

        ("-L", _) | ("--list", _) => {
            let db_path = "/var/lib/rad/installed";

            fn collect_packages(dir: &std::path::Path, prefix: &str, list: &mut Vec<String>) {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(name) = entry.file_name().to_str() {
                            if path.is_dir() {

                                let new_prefix = if prefix.is_empty() {
                                    name.to_string()
                                } else {
                                    format!("{}/{}", prefix, name)
                                };
                                collect_packages(&path, &new_prefix, list);
                            } else if path.is_file() {

                                let package_name = if prefix.is_empty() {
                                    name.to_string()
                                } else {
                                    format!("{}/{}", prefix, name)
                                };
                                list.push(package_name);
                            }
                        }
                    }
                }
            }

            let mut names: Vec<String> = Vec::new();
            collect_packages(std::path::Path::new(db_path), "", &mut names);
            names.sort();

            if names.is_empty() {
                println!("[rad] no packages installed yet.");
            } else {
                println!("[rad] installed packages:");
                for (i, name) in names.iter().enumerate() {
                    println!("{}. {}", i + 1, name);
                }
                println!("[rad] Total packages installed: {}", names.len());
            }
        }

        ("--hello", _) => println!("Hi there, bro"),

        (other, _) => eprintln!(
            "[rad] {} unknown argument '{}', try -h or --help",
            "error:".red(),
            other
        ),
    }
}