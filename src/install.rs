use std::process::Command;
use std::fs;
use std::path::Path;
use std::collections::HashSet;
use colored::Colorize;
use std::io::Write;
use crate::config::load_config;
use crate::package::{Package, BuildSystem, parse_package, fetch_package};

pub fn install_package(pkg_name: &str, prefix: &str, processing: &mut HashSet<String>) {
    let config = load_config();
    if processing.contains(pkg_name) {
        eprintln!("[rad] {} circular dependency detected: {}!", "error:".red(), pkg_name);
        return;
    }
    if is_installed(pkg_name) {
        println!("[rad] {} is already installed, skipping.", pkg_name);
        return;
    }
    processing.insert(pkg_name.to_string());
    let rad_path = match fetch_package(pkg_name) {
        Ok(p)  => p,
        Err(e) => { eprintln!("[rad] {} {}", "error:".red(), e); processing.remove(pkg_name); return; }
    };

    let pkg = match parse_package(&rad_path) {
        Ok(p)  => p,
        Err(e) => { eprintln!("[rad] {} {}", "parse error:".red(), e); processing.remove(pkg_name); return; }
    };
    for dep in &pkg.depends {
        if !is_installed(dep) {
            println!("[rad] resolving dependency: {}", dep);
            install_package(dep, prefix, processing);
        }
    }
    println!("[rad] package: {} {}", pkg.name, pkg.version);
    println!("[rad] info: {}", pkg.description);
    println!("[rad] source: {}", pkg.source);
    let src_dir = match download_and_extract(&pkg) {
        Ok(d)  => d,
        Err(e) => { eprintln!("[rad] {} {}", "error:".red(), e); processing.remove(pkg_name); return; }
    };

    // 64 bit build
    let dest_dir = format!("/tmp/rad/image/{}", pkg_name);
    let _ = fs::remove_dir_all(&dest_dir);
    fs::create_dir_all(&dest_dir).unwrap();

    if let Err(e) = build_and_install(&pkg, &src_dir, prefix, &dest_dir, false) {
        eprintln!("[rad] {} {}", "build error:".red(), e);
        processing.remove(pkg_name);
        return;
    }
    println!("[rad] indexing files for {}...", pkg_name);
    if let Err(e) = register_package_files(pkg_name, &dest_dir) {
        eprintln!("[rad] registration error: {}", e);
    }
    if let Err(e) = merge_to_system(&dest_dir) {
        eprintln!("[rad] {} {}", "merge error:".red(), e);
        processing.remove(pkg_name);
        return;
    }
    let _ = fs::remove_dir_all(&dest_dir);

    // 32 bit build if needed
    if config.arch.multilib && pkg.multilib_support {
        println!("[rad] multilib enabled, building 32-bit flavor of {}...", pkg_name);
        let dest_dir_m32 = format!("/tmp/rad/image/{}-m32", pkg_name);
        let _ = fs::remove_dir_all(&dest_dir_m32);
        fs::create_dir_all(&dest_dir_m32).unwrap();

        if let Err(e) = build_and_install(&pkg, &src_dir, prefix, &dest_dir_m32, true) {
            eprintln!("[rad] {} {}", "multilib build error:".red(), e);
            processing.remove(pkg_name);
            return;
        }

        let m32_name = format!("{}-m32", pkg_name);
        println!("[rad] indexing 32-bit files for {}...", m32_name);
        if let Err(e) = register_package_files(&m32_name, &dest_dir_m32) {
            eprintln!("[rad] registration error (m32): {}", e);
        }

        if let Err(e) = merge_to_system(&dest_dir_m32) {
            eprintln!("[rad] {} {}", "m32 merge error:".red(), e);
            processing.remove(pkg_name);
            return;
        }
        let _ = fs::remove_dir_all(&dest_dir_m32);
    }

    let build_dir = format!("/tmp/rad/build/{}", pkg_name);
    let _ = fs::remove_dir_all(&build_dir);

    processing.remove(pkg_name);
    println!("[rad] installation of '{}' finished successfully.", pkg_name);
}

pub fn download_and_extract(pkg: &Package) -> Result<String, String> {
    let work_dir = format!("/tmp/rad/build/{}", pkg.name);
    fs::create_dir_all(&work_dir)
        .map_err(|e| format!("cannot create build dir: {}", e))?;

    if pkg.source.ends_with(".git")
        || (pkg.source.contains("github.com") && !pkg.source.contains(".tar"))
    {
        println!("[rad] git detected. Cloning {}...", pkg.source);
        let status = Command::new("git")
            .args(["clone", "--recursive", &pkg.source, &work_dir])
            .status()
            .map_err(|e| format!("git clone failed: {}", e))?;
        if !status.success() { return Err("git clone failed".to_string()); }
        return Ok(work_dir);
    }

    let archive_name = pkg.source.split('/').last().unwrap_or("source.tar.gz");
    let archive_path = format!("{}/{}", work_dir, archive_name);

    println!("[rad] downloading {}...", pkg.source);
    let status = Command::new("wget")
        .args(["-c", &pkg.source, "-O", &archive_path])
        .status()
        .map_err(|e| format!("download failed: {}", e))?;
    if !status.success() { return Err("download failed".to_string()); }

    println!("[rad] extracting {}...", archive_name);
    let extract_status = if archive_path.ends_with(".zip") {
        Command::new("unzip").args([&archive_path, "-d", &work_dir]).status()
    } else {
        Command::new("tar").args(["-xf", &archive_path, "-C", &work_dir]).status()
    }.map_err(|e| format!("extraction failed: {}", e))?;
    if !extract_status.success() { return Err("extraction failed".to_string()); }

    let versioned = format!("{}/{}-{}", work_dir, pkg.name, pkg.version);
    let plain     = format!("{}/{}", work_dir, pkg.name);
    if Path::new(&versioned).exists() { return Ok(versioned); }
    if Path::new(&plain).exists()     { return Ok(plain); }

    fs::read_dir(&work_dir).map_err(|e| e.to_string())?
        .flatten()
        .find(|e| e.path().is_dir())
        .map(|e| e.path().to_string_lossy().to_string())
        .ok_or_else(|| "could not find extracted source directory".to_string())
}

pub fn build_and_install(
    pkg: &Package,
    src_dir: &str,
    prefix: &str,
    dest_dir: &str,
    is_m32: bool,
) -> Result<(), String> {
    fs::create_dir_all(dest_dir).unwrap();
    let mut current_configure_args = pkg.configure_args.clone();
    let mut current_libdir = format!("{}/lib", prefix);

    if is_m32 {
        println!("[rad] building 32-bit version of {}", pkg.name);
        current_libdir = format!("{}/lib32", prefix);
        current_configure_args.push("--libdir=/usr/lib32".into());
        current_configure_args.push("CFLAGS=-m32".into());
        current_configure_args.push("CXXFLAGS=-m32".into());
        current_configure_args.push("LDFLAGS=-m32".into());
        current_configure_args.push("--host=i686-pc-linux-gnu".into());
        current_configure_args.extend(pkg.multilib_configure_args.clone());
    }

    match &pkg.build_system {
        BuildSystem::Autotools => {
            println!("[rad] build system: autotools");
            let mut cmd = Command::new("./configure");
            cmd.arg(format!("--prefix={}", prefix))
               .arg(format!("--libdir={}", current_libdir))
               .current_dir(src_dir);
            for arg in &current_configure_args { cmd.arg(arg); }
            run_cmd(cmd, "configure")?;
            run_cmd(make_cmd(src_dir, &["-j4"]), "make")?;
            run_cmd(make_cmd(src_dir, &[&format!("DESTDIR={}", dest_dir), "install"]), "make install")?;
        }

        BuildSystem::Make => {
            println!("[rad] build system: make");
            let mut args: Vec<String> = vec!["-j4".into()];
            for arg in &current_configure_args { args.push(arg.clone()); }
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            run_cmd(make_cmd(src_dir, &args_ref), "make")?;
            run_cmd(make_cmd(src_dir, &[
                &format!("DESTDIR={}", dest_dir),
                &format!("PREFIX={}", prefix),
                "install",
            ]), "make install")?;
        }

        BuildSystem::Cmake => {
            println!("[rad] build system: cmake + ninja");
            let build_dir = format!("{}/build", src_dir);
            let _ = fs::remove_dir_all(&build_dir);
            fs::create_dir_all(&build_dir).unwrap();
            let mut cmd = Command::new("cmake");
            cmd.arg("..").arg("-GNinja")
               .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix))
               .arg(format!("-DCMAKE_INSTALL_LIBDIR={}", current_libdir))
               .current_dir(&build_dir);
            if is_m32 {
                cmd.arg("-DCMAKE_C_FLAGS=-m32");
                cmd.arg("-DCMAKE_CXX_FLAGS=-m32");
            }
            for arg in &current_configure_args { cmd.arg(arg); }
            run_cmd(cmd, "cmake")?;
            run_cmd(ninja_cmd(&build_dir, &[]), "ninja")?;
            run_cmd(ninja_install_cmd(&build_dir, dest_dir), "ninja install")?;
        }

        BuildSystem::Meson => {
            println!("[rad] build system: meson + ninja");
            let build_dir = format!("{}/build", src_dir);
            let mut cmd = Command::new("meson");
            cmd.arg("setup").arg(&build_dir)
               .arg(format!("--prefix={}", prefix))
               .arg(format!("--libdir={}", current_libdir))
               .current_dir(src_dir);
            for arg in &current_configure_args { cmd.arg(arg); }
            run_cmd(cmd, "meson setup")?;
            run_cmd(ninja_cmd(&build_dir, &[]), "ninja")?;
            run_cmd(ninja_install_cmd(&build_dir, dest_dir), "ninja install")?;
        }

        BuildSystem::Cargo => {
            println!("[rad] build system: cargo");
            let mut cmd = Command::new("cargo");
            cmd.arg("build").arg("--release").current_dir(src_dir);
            run_cmd(cmd, "cargo build")?;
            let bin_dest = format!("{}{}/bin", dest_dir, prefix);
            fs::create_dir_all(&bin_dest).unwrap();
            let bin_src = format!("{}/target/release/{}", src_dir, pkg.name);
            fs::copy(&bin_src, format!("{}/{}", bin_dest, pkg.name))
                .map_err(|e| format!("copy binary failed: {}", e))?;
        }

        BuildSystem::Python => {
            println!("[rad] build system: python (pip)");
            let mut cmd = Command::new("pip");
            cmd.args(["install", "--prefix", prefix, "--root", dest_dir, "."])
               .current_dir(src_dir);
            run_cmd(cmd, "pip install")?;
        }

        BuildSystem::Manual { build_commands, install_command } => {
            println!("[rad] build system: manual");
            for (i, cmd_str) in build_commands.iter().enumerate() {
                println!("[rad] build step {}/{}: {}", i + 1, build_commands.len(), cmd_str);
                let status = Command::new("sh")
                    .arg("-c").arg(cmd_str)
                    .current_dir(src_dir)
                    .env("PREFIX", prefix)
                    .env("LIBDIR", &current_libdir)
                    .env("IS_M32", if is_m32 { "1" } else { "0" })
                    .status()
                    .map_err(|e| format!("build step failed to start: {}", e))?;
                if !status.success() {
                    return Err(format!("build step failed: {}", cmd_str));
                }
            }
            println!("[rad] install step: {}", install_command);
            let status = Command::new("sh")
                .arg("-c").arg(install_command)
                .current_dir(src_dir)
                .env("DESTDIR", dest_dir)
                .env("PREFIX", prefix)
                .env("LIBDIR", &current_libdir)
                .env("IS_M32", if is_m32 { "1" } else { "0" })
                .status()
                .map_err(|e| format!("install step failed to start: {}", e))?;
            if !status.success() {
                return Err(format!("install step failed: {}", install_command));
            }
        }
    }

    println!("[rad] build finished! Files are in {}", dest_dir);
    Ok(())
}

pub fn run_cmd(mut cmd: Command, label: &str) -> Result<(), String> {
    println!("[rad] running: {}...", label);
    let status = cmd.status()
        .map_err(|e| format!("{} failed to start: {}", label, e))?;
    if !status.success() {
        return Err(format!("{} exited with status: {}", label, status));
    }
    Ok(())
}

pub fn make_cmd(dir: &str, args: &[&str]) -> Command {
    let mut c = Command::new("make");
    for a in args { c.arg(a); }
    c.current_dir(dir);
    c
}

pub fn ninja_cmd(dir: &str, args: &[&str]) -> Command {
    let mut c = Command::new("ninja");
    for a in args { c.arg(a); }
    c.current_dir(dir);
    c
}

pub fn ninja_install_cmd(build_dir: &str, dest_dir: &str) -> Command {
    let mut c = Command::new("ninja");
    c.arg("install").env("DESTDIR", dest_dir).current_dir(build_dir);
    c
}

pub fn register_package_files(pkg_name: &str, dest_dir: &str) -> std::io::Result<()> {
    let db_path = "/var/lib/rad/installed";
    fs::create_dir_all(db_path)?;
    let mut manifest = fs::File::create(format!("{}/{}", db_path, pkg_name))?;
    let dest_path = Path::new(dest_dir);
    collect_files(dest_path, dest_path, &mut manifest)
}

pub fn collect_files(root: &Path, current: &Path, manifest: &mut fs::File) -> std::io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path  = entry.path();
        if path.is_dir() {
            collect_files(root, &path, manifest)?;
        } else {
            let relative = path.strip_prefix(root).unwrap();
            writeln!(manifest, "/{}", relative.display())?;
        }
    }
    Ok(())
}

pub fn merge_to_system(dest_dir: &str) -> Result<(), String> {
    println!("[rad] merging files to system...");
    let dest_path = Path::new(dest_dir);
    merge_dir(dest_path, dest_path, Path::new("/"))
        .map_err(|e| format!("merge failed: {}", e))?;
    println!("[rad] merge done.");
    Ok(())
}

pub fn merge_dir(root: &Path, current: &Path, target_base: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry    = entry?;
        let src_path = entry.path();
        let relative  = src_path.strip_prefix(root).unwrap();
        let dest_path = target_base.join(relative);
        if src_path.is_dir() {
            fs::create_dir_all(&dest_path)?;
            merge_dir(root, &src_path, target_base)?;
        }
        else {
            atomic_install(&src_path, &dest_path)?;
        }
    }
    Ok(())
}
pub fn atomic_install(src: &Path, dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension(
        format!("{}.rad_new",
            dest.extension().and_then(|e| e.to_str()).unwrap_or(""))
    );
    fs::copy(src, &tmp)?;
    if let Ok(meta) = fs::metadata(src) {
        let _ = fs::set_permissions(&tmp, meta.permissions());
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn is_installed(name: &str) -> bool {
    Path::new(&format!("/var/lib/rad/installed/{}", name)).exists()
}