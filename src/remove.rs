use crate::config::load_config;
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::io;

const DB_PATH: &str = "/var/lib/rad/installed";

pub fn clear_cache() {
    let config = load_config();
    if Path::new("/tmp/rad").exists() {
        match fs::remove_dir_all("/tmp/rad") {
            Ok(_) => println!("[rad] succesfully removed {}", "/tmp/rad".yellow()),
            Err(e) => eprintln!("[rad] {} couldn't remove {}: {}", "error:".red(), "/tmp/rad".yellow(), e),
        }
    }
    else {
        println!("[rad] {} doesn't exist, nothing to clear", "/tmp/rad".yellow());
    }
    if Path::new(&config.build.bin_cache_dir).exists() {
        match fs::remove_dir_all(&config.build.bin_cache_dir) {
            Ok(_) => println!("[rad] succesfully removed {}", config.build.bin_cache_dir.yellow()),
            Err(e) => eprintln!("[rad] {} couldn't remove {}: {}", "error:".red(), config.build.bin_cache_dir.yellow(), e),
        }
    }
    else {
        println!("[rad] {} doesn't exist, nothing to clear", config.build.bin_cache_dir.yellow());
    }
    println!("[rad] cache clearing completed")
}

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

pub fn resolve_package_atom(input: &str) -> Option<String> {
    if input.contains('/') {
        let full_path = format!("{}/{}", DB_PATH, input);
        if Path::new(&full_path).exists() {
            return Some(input.to_string());
        }
        return None;
    }

    let db_dir = Path::new(DB_PATH);
    if let Ok(categories) = fs::read_dir(db_dir) {
        for cat_entry in categories.flatten() {
            if cat_entry.path().is_dir() {
                let category_name = cat_entry.file_name().into_string().ok()?;
                let manifest_path = cat_entry.path().join(input);
                if manifest_path.exists() {
                    return Some(format!("{}/{}", category_name, input));
                }
            }
        }
    }
    None
}

pub fn files_owned_by_others(exclude_atom: &str) -> std::io::Result<HashSet<String>> {
    let mut shared = HashSet::new();
    let db_dir = Path::new(DB_PATH);
    
    if !db_dir.exists() {
        return Ok(shared);
    }

    for cat_entry in fs::read_dir(db_dir)?.flatten() {
        let cat_path = cat_entry.path();
        if !cat_path.is_dir() {
            continue;
        }
        let category_name = match cat_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        for pkg_entry in fs::read_dir(&cat_path)?.flatten() {
            let pkg_path = pkg_entry.path();
            if !pkg_path.is_file() {
                continue;
            }
            let pkg_name = match pkg_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            let current_atom = format!("{}/{}", category_name, pkg_name);
            if current_atom == exclude_atom {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&pkg_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        shared.insert(line.to_string());
                    }
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

pub fn remove_package(input_name: &str) -> std::io::Result<()> {
    let pkg_atom = match resolve_package_atom(input_name) {
        Some(atom) => atom,
        None => {
            println!("[rad] {} package '{}' is not installed", "error:".red(), input_name);
            return Ok(());
        }
    };

    let manifest_path = format!("{}/{}", DB_PATH, pkg_atom);
    
    let config = load_config();

    let content = fs::read_to_string(&manifest_path)?;

    println!("\n[rad] {} is installed, going to remove files:\n{}", pkg_atom, content.yellow());
    if config.build.ask { let _ = ask_to_remove(); }

    println!("[rad] removing package: {}", pkg_atom);

    let shared = files_owned_by_others(&pkg_atom)?;

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
                Err(e) => eprintln!("[rad] {} could not remove {}: {}", "error:".red(), line, e),
            }
        }
    }

    fs::remove_file(&manifest_path)?;

    let meta_path = format!("/var/lib/rad/meta/{}.toml", pkg_atom);
    if Path::new(&meta_path).exists() {
        if let Err(e) = fs::remove_file(&meta_path) {
            eprintln!("[rad] {} could not remove metadata file: {}", "warning:".purple(), e);
        }
    }

    if skipped > 0 {
        println!(
            "[rad] kept {} file(s) still used by other installed packages",
            skipped
        );
    }

    println!(
        "[rad] package {} successfully cleaned from your fantastic system ({} files removed)",
        pkg_atom, removed
    );
    Ok(())
}