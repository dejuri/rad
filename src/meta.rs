use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
pub struct InstalledMeta {
    pub version: String,
    pub category: String,
    pub depends: Vec<String>,
    pub installed_at: String,
}

pub fn meta_path(atom: &str) -> String {
    format!("/var/lib/rad/meta/{}.toml", atom)
}

pub fn write_meta(atom: &str, version: &str, category: &str, depends: &[String]) -> std::io::Result<()> {
    let path = meta_path(atom);
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent)?;
    }
    let meta = InstalledMeta {
        version: version.to_string(),
        category: category.to_string(),
        depends: depends.to_vec(),
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    fs::write(path, toml::to_string_pretty(&meta).unwrap())
}

fn walk_meta_dir(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut result = Vec::new();
    if !dir.exists() {
        return Ok(result);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            result.extend(walk_meta_dir(&path)?);
        } else if path.extension().is_some_and(|e| e == "toml") {
            let atom = path
                .strip_prefix("/var/lib/rad/meta")
                .unwrap()
                .with_extension("")
                .to_string_lossy()
                .trim_start_matches('/')
                .to_string();
            result.push(atom);
        }
    }
    Ok(result)
}

pub fn find_dependents(pkg_name: &str) -> Vec<String> {
    let mut result = Vec::new();
    let base = Path::new("/var/lib/rad/meta");
    if let Ok(entries) = walk_meta_dir(base) {
        for atom in entries {
            if let Some(m) = read_meta(&atom) {
                if m.depends.iter().any(|d| d == pkg_name) {
                    result.push(atom);
                }
            }
        }
    }
    result
}

pub fn read_meta(atom: &str) -> Option<InstalledMeta> {
    let content = fs::read_to_string(meta_path(atom)).ok()?;
    toml::from_str(&content).ok()
}