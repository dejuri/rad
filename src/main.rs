use colored::Colorize;
use rad::config::*;
use rad::install::*;
use rad::package::*;
use rad::remove::*;
use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::env;
use std::fs;

// Check for deps
fn is_there(name: &str) -> Result<(), String> {
    let status = Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| format!("{}", "install which firstly".blue()))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("not found"))
    }
}

// Main
fn main() {
    let config = load_config();
    let args: Vec<String> = env::args().collect();
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
    let prefix = "/usr";
    let deps = vec!["cargo", "make", "cmake", "meson", "ninja", "pip", "tar", "unzip", "git", "wget", "sh"];
    if args.len() < 2 {
        eprintln!(
            "[rad] {} please specify a valid argument, use -h or --help",
            "error:".red()
        );
        return;
    }

    match args[1].as_str() {
        "-h" | "--help" => {
            println!(
                "{} v{}\n\n  \
                Usage: rad [command]\n\n  \
                Commands:\n    \
                    -h, --help              print this menu\n    \
                    -V, --version           print rad version\n    \
                    -s, --sync              sync package index of current repository\n    \
                    -C, --clear-cache       clear rad cache and temporary files\n    \
                    -L, --list              list installed packages\n    \
                    -I, --info              info about rad on your system\n    \
                    -i, --install <pkg>     install a package\n    \
                    -b, --build <pkg>       build package source without installing\n    \
                    -f, --force <pkg>       force package installation (used to update packages)\n    \
                    -r, --remove  <pkg>     remove a package\n    \
                    -P, --pkg-info <pkg>    info about specific package\n\n  \
                Packages are searched:\n    \
                    1. Locally:   ./<pkg>.toml\n    \
                    2. Remote:    {}/<pkg>.toml",
                "Radrix Automated TOML-packages Handler".bold(),
                version.yellow(),
                config.repo.url
            );
        }

        "-I" | "--info" => {
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
                        - url: {}",
                config.arch.multilib, config.build.makeopts, config.build.ask, config.build.bin_cache_dir, config.repo.url
            );
        }

        "-V" | "--version" => println!(
            "rad - {}\n  version: {}",
            "Radrix Automated TOML-packages Handler".bold(),
            version.yellow()
        ),

        "-i" | "--install" => {
            let mut processing = HashSet::new();
            match args.get(2) {
                Some(name) => install_package(name, prefix, false, true, true, &mut processing),
                None => eprintln!("[rad] {} specify the package name", "error:".red()),
            }
        }
        
        "-b" | "--build" => {
            let mut processing = HashSet::new();
            match args.get(2) {
                Some(name) => install_package(name, prefix, true, true, false, &mut processing),
                None => eprintln!("[rad] {} specify the package name", "error:".red()),
            }
        }

        "-s" | "--sync" => {
            println!("[rad] updating package index of the repository");
            match rad::index::refresh_index(&config.repo.url) {
                Ok(_) => println!("[rad] package index updated"),
                Err(e) => eprintln!("[rad] {} {}", "error:".red(), e),
            }
        }

        "-C" | "--clear-cache" => {
            println!("[rad] clearing cache and build remains");
            clear_cache();
        }

        "-f" | "--force" => {
            let mut processing = HashSet::new();
            match args.get(2) {
                Some(name) => install_package(name, prefix, true, true, true, &mut processing),
                None => eprintln!("[rad] {} specify the package name", "error:".red()),
            }
        }

        "-r" | "--remove" => match args.get(2) {
            Some(name) => {
                if let Err(e) = remove_package(name) {
                    eprintln!("[rad] {} {}", "removal error:".red(), e);
                }
            }
            None => eprintln!("[rad] {} specify the package name", "error:".red()),
        }

        "-L" | "--list" => {
            let db_path = "/var/lib/rad/installed";
            match fs::read_dir(db_path) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .flatten()
                        .filter_map(|entry| entry.file_name().into_string().ok())
                        .collect();
                    names.sort();

                    println!("[rad] installed packages:");
                    for (i, name) in names.iter().enumerate() {
                        println!("{}. {}", i + 1, name);
                    }
                    println!("[rad] Total packages installed: {}", names.len());
                }
                Err(_) => println!("[rad] no packages installed yet."),
            }
        }

        "-P" | "--pkg-info" => {
            let mut processing = HashSet::new();
            match args.get(2) {
                Some(name) => package_info(name, &mut processing),
                None => eprintln!("[rad] {} specify the package name", "error:".red()),
            }
        }

        "--hello" => println!("Hi there, bro"),

        other => eprintln!(
            "[rad] {} unknown argument '{}', try -h or --help",
            "error:".red(),
            other
        ),
    }
}