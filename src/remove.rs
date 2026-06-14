use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const DB_PATH: &str = "/var/lib/rad/installed";

fn files_owned_by_others(exclude_pkg: &str) -> std::io::Result<HashSet<String>> {
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

fn prune_empty_dirs(path: &Path) {
    let mut dir = match path.parent() {
        Some(p) => p.to_path_buf(),
        None => return,
    };
    loop {
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
    if !Path::new(&manifest_path).exists() {
        println!("[rad] {} {} is not installed.", "error:".red(), pkg_name);
        return Ok(());
    }

    println!("[rad] removing package: {}", pkg_name);
    let content = fs::read_to_string(&manifest_path)?;

    // If files are own by other pkgs - dont delete them
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