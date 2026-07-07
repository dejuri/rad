use std::fs;
use std::path::Path;
use std::process::Command;

const INDEX_DIR: &str = "/var/lib/rad/packages";
const INDEX_PATH: &str = "/var/lib/rad/packages/index";

pub fn refresh_index(repo_url: &str) -> Result<(), String> {
    fs::create_dir_all(INDEX_DIR).map_err(|e| format!("cannot create {}: {}", INDEX_DIR, e))?;
    let url = format!("{}/packages.index", repo_url);
    let status = Command::new("wget")
        .args(["-q", "-O", INDEX_PATH, &url])
        .status()
        .map_err(|e| format!("wget failed: {}", e))?;
    if !status.success() {
        return Err(format!("couldn't fetch packages.index from {}", repo_url));
    }
    Ok(())
}

// Returns category/name for a bare package name if the input already contains /
pub fn resolve(pkg_name: &str, repo_url: &str) -> Result<String, String> {
    if pkg_name.contains('/') {
        return Ok(pkg_name.to_string());
    }

    if !Path::new(INDEX_PATH).exists() {
        refresh_index(repo_url)?;
    }

    let lookup = |content: &str| -> Vec<String> {
        content
            .lines()
            .filter(|line| {
                line.rsplit('/').next() == Some(pkg_name)
            })
            .map(|s| s.to_string())
            .collect()
    };

    let content = fs::read_to_string(INDEX_PATH)
        .map_err(|e| format!("cannot read {}: {}", INDEX_PATH, e))?;
    let mut matches = lookup(&content);

    match matches.len() {
        0 => Err(format!("package '{}' not found in package index", pkg_name)),
        1 => Ok(matches.remove(0)),
        _ => Err(format!(
            "'{}' is not specific enough, it is found in: {}. specify as <category>/<name> please",
            pkg_name,
            matches.join(", ")
        )),
    }
}