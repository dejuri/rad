use std::fs;
use std::path::Path;
use std::process::Command;

pub struct Source {
    pub base: String,
    pub is_local: bool,
}

fn read_index(source: &Source) -> Result<String, String> {
    if source.is_local {
        let path = format!("{}/packages.index", source.base);
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path, e))
    } else {
        let cache_path = format!("/var/lib/rad/packages/index/{}.index", sanitize(&source.base));
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
    let cache_path = format!("/var/lib/rad/packages/index/{}.index", sanitize(overlay_url));
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