use colored::Colorize;
use rad::config::*;
use rad::install::*;
use rad::package::*;
use rad::remove::*;
use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::env;

fn is_there(name: &str) -> Result<(), String> {
    let status = Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| format!("{}", "install which first".blue()))?;

    if status.success() { Ok(()) } else { Err(format!("not found")) }
}

fn main() {
    let config = load_config();
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
    let prefix = "/usr";
    let overlays_formatted = config.repo.overlays
        .iter()
        .map(|s| format!("\n      {}", s.yellow()))
        .collect::<String>();

    let deps = vec!["cargo", "make", "cmake", "meson", "ninja", "pip", "tar", "unzip", "git", "wget", "sh"];

    let mut pargs = pico_args::Arguments::from_env();

    if env::args().len() < 2 {
        eprintln!(
            "[rad] {} please specify a valid argument, use -h or --help",
            "error:".red()
        );
        return;
    }

    let help = pargs.contains(["-h", "--help"]);
    let info = pargs.contains(["-I", "--info"]);
    let ver = pargs.contains(["-V", "--version"]);
    let sync = pargs.contains(["-s", "--sync"]);
    let clear = pargs.contains(["-C", "--clear-cache"]);
    let list = pargs.contains(["-L", "--list"]);
    let hello = pargs.contains("--hello");
    let local = pargs.contains(["-l", "--local"]);
    let install_pkg: Option<String> = pargs.opt_value_from_str(["-i", "--install"]).ok().flatten();
    let force_pkg: Option<String> = pargs.opt_value_from_str(["-f", "--force"]).ok().flatten();
    let build_pkg: Option<String> = pargs.opt_value_from_str(["-b", "--build"]).ok().flatten();
    let pkg_info: Option<String> = pargs.opt_value_from_str(["-P", "--pkg-info"]).ok().flatten();
    let remove_pkg: Option<String> = pargs.opt_value_from_str(["-r", "--remove"]).ok().flatten();

    if help {
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
                -l, --local             use path of local toml package (with -i, -f, -b and -P)\n\n  \
            Packages are searched in main repository and overlays\n    \
            Main repository: {}\n    \
            Overlays: {}",
            "Radian Automated TOML-packages Handler".bold(), version.yellow(), config.repo.url.yellow(), overlays_formatted.yellow()
        );
    } else if info {
        println!("[rad] info:\nDEPENDENCIES");
        let status = Command::new("which").arg("which").stdout(Stdio::null()).stderr(Stdio::null()).status();
        match status {
            Ok(s) if s.success() => println!("  - which: {}", "found".green()),
            _ => println!("  - which: {}", "not found".red()),
        }   
        for i in deps {
            match is_there(i) {
                Ok(_) => println!("  - {}: {}", i, "found".green()),
                Err(e) => eprintln!("  - {}: {}", i, e.red()),
            }
        }
        println!("CONFIG\n  ARCH\n    - multilib: {}\n  BUILD\n    - makeopts: {}\n    - ask: {}\n    - bin cache dir: {}\n  REPO\n    - url: {}\n    - overlays: {:?}",
            config.arch.multilib, config.build.makeopts, config.build.ask, config.build.bin_cache_dir, config.repo.url, config.repo.overlays
        );
    } else if ver {
        println!(
            "rad - {}\n  version: {}",
            "Radian Automated TOML-packages Handler".bold(),
            version.yellow()
        );
    } else if sync {
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
    } else if clear {
        println!("[rad] clearing cache and build remains");
        clear_cache();
    } else if list {
        let db_path = "/var/lib/rad/installed";

        fn collect_packages(dir: &std::path::Path, prefix: &str, list: &mut Vec<String>) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = entry.file_name().to_str() {
                        if path.is_dir() {
                            let new_prefix = if prefix.is_empty() { name.to_string() } else { format!("{}/{}", prefix, name) };
                            collect_packages(&path, &new_prefix, list);
                        } else if path.is_file() {
                            let package_name = if prefix.is_empty() { name.to_string() } else { format!("{}/{}", prefix, name) };
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
    } else if hello {
        println!("Hi there, bro");
    } else if let Some(name) = install_pkg {
        let mut processing = HashSet::new();
        install_package(&name, prefix, false, true, true, local, &mut processing);
    } else if let Some(name) = force_pkg {
        let mut processing = HashSet::new();
        install_package(&name, prefix, true, true, true, local, &mut processing);
    } else if let Some(name) = build_pkg {
        let mut processing = HashSet::new();
        install_package(&name, prefix, true, true, false, local, &mut processing);
    } else if let Some(name) = pkg_info {
        let mut processing = HashSet::new();
        package_info(&name, local, &mut processing);
    } else if let Some(name) = remove_pkg {
        if let Err(e) = remove_package(&name) {
            eprintln!("[rad] {} {}", "removal error:".red(), e);
        }
    } else {
        eprintln!(
            "[rad] {} try -h or --help to learn how to specify arguments correctly",
            "error:".red()
        );
    }
}