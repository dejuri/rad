use colored::Colorize;
use rad::config::*;
use rad::install::*;
use rad::package::*;
use rad::remove::*;
use std::collections::HashSet;
use std::env;
use std::fs;

// Main
fn main() {
    let config = load_config();
    let args: Vec<String> = env::args().collect();
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
    let prefix = "/usr";
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
                    -i, --install <pkg>     install a package\n    \
                    -r, --remove  <pkg>     remove a package\n    \
                    -L, --list              list installed packages\n    \
                    -P, --pkg-info <pkg>    info about specific package\n    \
                    -I, --info              info about rad on your system\n\n  \
                Packages are searched:\n    \
                    1. Locally:   ./<pkg>.toml\n    \
                    2. Remote:    {}/<pkg>.toml",
                "Radrix Automated TOML-packages Handler".bold(),
                version.yellow(),
                config.repo.url
            );
        }
        "-I" | "--info" => {
            let multilib_status = if config.arch.multilib {
                "yes".green()
            } else {
                "no".red()
            };
            println!(
                "[rad] info:\n  \
            ARCH\n    \
            - multilib: {}\n  \
            REPO\n    \
            - url: {}",
                multilib_status, config.repo.url
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
                Some(name) => install_package(name, prefix, &mut processing),
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
        },

        "-L" | "--list" => {
            let db_path = "/var/lib/rad/installed";
            match fs::read_dir(db_path) {
                Ok(entries) => {
                    println!("[rad] installed packages:");
                    let mut i = 0;
                    for entry in entries.flatten() {
                        i += 1;
                        if let Ok(name) = entry.file_name().into_string() {
                            println!("{}. {}", i, name);
                        }
                    }
                    println!("[rad] Total packages installed: {}", i);
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

        "--hello" => println!("Hello World, btw microslop sucks"),

        other => eprintln!(
            "[rad] {} unknown argument '{}', try -h or --help",
            "error:".red(),
            other
        ),
    }
}
