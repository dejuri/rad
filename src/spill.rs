// If there is insufficient RAM, build data from rad is offloaded to the hard drive
use crate::config::load_config;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const BUILD_ROOT: &str = "/tmp/rad/build";

const MIN_FREE_BYTES: u64 = 512 * 1024 * 1024;

pub fn build_root_of(path: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(BUILD_ROOT).ok()?;
    let pkg = rest.components().next()?;
    Some(Path::new(BUILD_ROOT).join(pkg))
}

fn ok(cmd: &mut Command) -> bool {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn output_of(cmd: &mut Command) -> Option<String> {
    let out = cmd.stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn is_mountpoint(p: &Path) -> bool {
    ok(Command::new("mountpoint").arg("-q").arg(p))
}

fn on_tmpfs(p: &Path) -> bool {
    output_of(Command::new("stat").args(["-f", "-c", "%T"]).arg(p)).as_deref() == Some("tmpfs")
}

fn free_bytes(p: &Path) -> Option<u64> {
    let s = output_of(Command::new("df").args(["-B1", "--output=avail"]).arg(p))?;
    s.lines().last()?.trim().parse().ok()
}

pub fn low_on_space(p: &Path) -> bool {
    free_bytes(p).is_some_and(|f| f < MIN_FREE_BYTES)
}

pub fn try_spill(work_dir: &Path) -> bool {
    let base = load_config().build.spill_dir;
    if base.is_empty() {
        return false;
    }
    if is_mountpoint(work_dir) {
        return false;
    }
    if !on_tmpfs(work_dir) {
        return false;
    }
    let Some(pkg) = work_dir.file_name().filter(|n| *n != OsStr::new("")) else {
        return false;
    };

    let base = PathBuf::from(base);
    let target = base.join(pkg);
    let _ = fs::remove_dir_all(&target);
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("[rad] Spill: cannot create {}: {}", target.display(), e);
        return false;
    }
    if on_tmpfs(&target) {
        eprintln!("[rad] Spill: build.spill_dir ({}) is also tmpfs, ignoring", base.display());
        return false;
    }

    println!("[rad] Tmpfs is full, moving the build tree to {}", target.display());

    // 1. Copy everything
    let src = format!("{}/.", work_dir.display());
    if !ok(Command::new("cp").args(["-a", &src]).arg(&target)) {
        eprintln!("[rad] Spill: copy to disk failed");
        let _ = fs::remove_dir_all(&target);
        return false;
    }

    // 2. Free the RAM: empty the tmpfs directory
    if let Ok(entries) = fs::read_dir(work_dir) {
        for e in entries.flatten() {
            let p = e.path();
            let _ = if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                fs::remove_dir_all(&p)
            } else {
                fs::remove_file(&p)
            };
        }
    }

    // 3. Put the on-disk copy at the original path
    if !ok(Command::new("mount").arg("--bind").arg(&target).arg(work_dir)) {
        eprintln!("[rad] Spill: mount --bind failed (not root?), restoring");
        let _ = ok(Command::new("cp").args(["-a", &format!("{}/.", target.display())]).arg(work_dir));
        let _ = fs::remove_dir_all(&target);
        return false;
    }
    true
}

pub fn release(pkg_name: &str) {
    if pkg_name.is_empty() || pkg_name.contains('/') {
        return;
    }
    let work = Path::new(BUILD_ROOT).join(pkg_name);
    if is_mountpoint(&work) && !ok(Command::new("umount").arg(&work)) {
        let _ = ok(Command::new("umount").arg("-l").arg(&work));
    }
    if is_mountpoint(&work) {
        return;
    }
    let base = load_config().build.spill_dir;
    if !base.is_empty() {
        let _ = fs::remove_dir_all(Path::new(&base).join(pkg_name));
    }
}
