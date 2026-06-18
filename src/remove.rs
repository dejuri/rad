use crate::config::load_config;
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::io;

const DB_PATH: &str = "/var/lib/rad/installed";

fn ask_to_remove() -> io::Result<()> {
    println!("[rad] Are you sure that you want remove this package? Y/n");
    let mut buffer = String::new();
    match io::stdin().read_line(&mut buffer) {
        Ok(_) => {},
        Err(error) => println!("[rad] {} {}", "error:".red(), error),
    }
    match buffer.trim() {
        "y" | "Y" => {
            println!("[rad] Ok, comrade, continuing");
            return Ok(()); 
        }
        "n" | "N" => {
            println!("[rad] Ok, comrade, aborting");
            std::process::exit(0);     
        }
        _ => {
            eprintln!("[rad] I don't understand you");
            let _ = ask_to_remove();
            return Ok(()); 
        }
    }
}

pub fn files_owned_by_others(exclude_pkg: &str) -> std::io::Result<HashSet<String>> {
    let mut shared = HashSet::new();
    let entries = match fs::read_dir(DB_PATH) {
        Ok(e) => e,
        Err(_) => return Ok(shared),
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name == exclude_pkg {
            continue;
        }
        if let Ok(content) = fs::read_to_string(entry.path()) {
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    shared.insert(line.to_string());
                }
            }
        }
    }
    Ok(shared)
}

pub fn prune_empty_dirs(path: &Path) {
    let mut dir = match path.parent() {
        Some(p) => p.to_path_buf(),
        None => return,
    };
    loop {
        // Never remove root or top-level system dirs. This is safety default
        if dir.as_os_str().is_empty() || dir == Path::new("/") {
            break;
        }
        match fs::read_dir(&dir) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    break;
                }
            }
            Err(_) => break,
        }
        if fs::remove_dir(&dir).is_err() {
            break;
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }
}

pub fn remove_package(pkg_name: &str) -> std::io::Result<()> {
    let manifest_path = format!("{}/{}", DB_PATH, pkg_name);
    let config = load_config();
    if !Path::new(&manifest_path).exists() {
        println!("[rad] {} {} is not installed.", "error:".red(), pkg_name);
        return Ok(());
    }

    let content = fs::read_to_string(&manifest_path)?;

    println!("\n[rad] {} is installed, going to remove files:\n{}", pkg_name, content.yellow());
    if config.build.ask {let _ = ask_to_remove(); }

    println!("[rad] removing package: {}", pkg_name);

    // If files are owned by other installed packages don't delete
    let shared = files_owned_by_others(pkg_name)?;

    let mut skipped = 0;
    let mut removed = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if shared.contains(line) {
            skipped += 1;
            continue;
        }

        let path = Path::new(line);

        if line == "/usr/share/info/dir" {
            continue;
        }

        if path.exists() {
            match fs::remove_file(path) {
                Ok(_) => {
                    removed += 1;
                    prune_empty_dirs(path);
                }
                Err(e) => eprintln!("[rad] could not remove {}: {}", line, e),
            }
        }
    }

    fs::remove_file(&manifest_path)?;

    if skipped > 0 {
        println!(
            "[rad] kept {} file(s) still used by other installed packages",
            skipped
        );
    }

    println!(
        "[rad] package {} successfully cleaned from your fantastic system ({} files removed)",
        pkg_name, removed
    );
    Ok(())
}