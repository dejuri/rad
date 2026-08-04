use std::fs;
use std::path::Path;
use std::process::Command;

const INDEX_DIR: &str = "/var/lib/rad/packages";
const INDEX_PATH: &str = "/var/lib/rad/packages/index";

pub struct Source {
    pub base: String,
    pub is_local: bool,
}

fn read_index(source: &Source) -> Result<String, String> {
    if source.is_local {
        let path = format!("{}/packages.index", source.base);
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path, e))
    } else {
        let cache_path = format!("/var/lib/rad/packages/{}.index", sanitize(&source.base));
        if !Path::new(&cache_path).exists() {
            let url = format!("{}/packages.index", source.base);
            let status = Command::new("wget")
                .args(["-q", "-O", &cache_path, &url])
                .status()
                .map_err(|e| format!("wget failed: {}", e))?;
            if !status.success() {
                return Err(format!("couldn't fetch packages.index from {}", source.base));
            }
        }
        fs::read_to_string(&cache_path).map_err(|e| format!("cannot read {}: {}", cache_path, e))
    }
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect()
}

pub fn resolve_with_source(pkg_name: &str, config: &crate::config::Config) -> Result<(String, Source), String> {
    let mut sources: Vec<Source> = config.repo.overlays.iter().map(|o| Source {
        base: o.clone(),
        is_local: !o.starts_with("http://") && !o.starts_with("https://"),
    }).collect();
    sources.push(Source { base: config.repo.url.clone(), is_local: false });

    for source in sources {
        let content = match read_index(&source) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if pkg_name.contains('/') {
            if content.lines().any(|line| line == pkg_name) {
                return Ok((pkg_name.to_string(), source));
            }
            continue;
        }

        let matches: Vec<&str> = content
            .lines()
            .filter(|line| line.rsplit('/').next() == Some(pkg_name))
            .collect();
        match matches.len() {
            0 => continue,
            1 => return Ok((matches[0].to_string(), source)),
            _ => return Err(format!(
                "'{}' is not specific enough in {}, found: {}. specify as <category>/<name>",
                pkg_name, source.base, matches.join(", ")
            )),
        }
    }
    Err(format!("package '{}' not found in repo or overlays", pkg_name))
}

pub fn refresh_overlay_index(overlay_url: &str) -> Result<(), String> {
    let cache_path = format!("/var/lib/rad/packages/{}.index", sanitize(overlay_url));
    let url = format!("{}/packages.index", overlay_url);
    let status = Command::new("wget")
        .args(["-q", "-O", &cache_path, &url])
        .status()
        .map_err(|e| format!("wget failed: {}", e))?;
    if !status.success() {
        return Err(format!("couldn't fetch packages.index from {}", overlay_url));
    }
    Ok(())
}

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

// Returns category/name for a bare package name
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