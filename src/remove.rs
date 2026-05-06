use std::fs;
use std::path::Path;

pub fn remove_package(pkg_name: &str) -> std::io::Result<()> {
    let manifest_path = format!("/var/lib/rad/installed/{}", pkg_name);
    if !Path::new(&manifest_path).exists() {
        println!("[rad] package {} is not installed.", pkg_name);
        return Ok(());
    }
    println!("[rad] removing package: {}", pkg_name);
    let content = fs::read_to_string(&manifest_path)?;
    for line in content.lines() {
        if line == "/usr/share/info/dir" { continue; }
        let path = Path::new(line);
        if path.exists() {
            if let Err(e) = fs::remove_file(path) {
                eprintln!("[rad] could not remove {}: {}", line, e);
            }
        }
    }
    fs::remove_file(&manifest_path)?;
    println!("[rad] package {} successfully cleaned from your fantastic system", pkg_name);
    Ok(())
}