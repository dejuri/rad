use crate::config::load_config;
use crate::package::{BuildSystem, Package, fetch_package, parse_package};
use crate::index;
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::io;
use std::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use crate::meta::{write_meta, find_dependents};
use crate::verbosity::is_verbose;

fn spinner(msg: &str) -> Option<ProgressBar> {
    if is_verbose() {
        return None;
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("[{spinner:}] [rad] {msg}")
            .unwrap()
            .tick_chars("\\|/--"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(200));
    Some(pb)
}

fn ask_to_install() -> io::Result<()> {
    println!("[rad] Are you sure that you want install this package? Y/n");
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
            let _ = ask_to_install();
            return Ok(()); 
        }
    }
}
pub fn install_package(pkg_name: &str, prefix: &str, force: bool, askable: bool, going_install: bool, local: bool, processing: &mut HashSet<String>) {
    let config = load_config();

    let rad_path = if local {
        let path = if pkg_name.ends_with(".toml") { pkg_name.to_string() } else { format!("{}.toml", pkg_name) };
        if !Path::new(&path).exists() {
            eprintln!("[rad] {} local package file not found: {}", "error:".red(), path);
            return;
        }
        path
    } else {
        match fetch_package(pkg_name) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[rad] {} {}", "error:".red(), e);
                return;
            }
        }
    };

    let pkg = match parse_package(&rad_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[rad] {} {}", "parse error:".red(), e);
            return;
        }
    };

    let (atom, origin_str) = if local {
        (
            format!("local/{}", pkg.name),
            format!("local file ({})", rad_path),
        )
    } else {
        match index::resolve_with_source(pkg_name, &config) {
            Ok((a, source)) => {
                let desc = if source.is_local {
                    format!("local overlay ({})", source.base)
                } else if source.base == config.repo.url {
                    "main repository".to_string()
                } else {
                    format!("remote overlay ({})", source.base)
                };
                (a, desc)
            }
            Err(e) => {
                eprintln!("[rad] {} {}", "error:".red(), e);
                processing.remove(pkg_name);
                return;
            }
        }
    };


    let category = atom.rsplit_once('/').map(|(c, _)| c).unwrap_or("");

    // Skip if the same version is installed
    let installed_meta = crate::meta::read_meta(&atom);
    let needs_upgrade = match &installed_meta {
        Some(m) => m.version != pkg.version,
        None => false,
    };

    if installed_meta.is_some() && !needs_upgrade && !force {
        println!("[rad] {} is already up to date ({})", atom.yellow(), pkg.version);
        return;
    }

    // Package information
    println!("[rad] Building package {} ({})\n  \
                    - Description: {}\n  \
                    - Package origin: {}\n\
                    {}", 
                    atom.yellow(), 
                    pkg.version.yellow(), 
                    pkg.description, 
                    origin_str, 
                    if !pkg.unfree { format!("  - Package source: {}", pkg.source) } else { String::from("  - Package is proprietary!") });
    if is_installed(&atom) && going_install {
        println!("  - Installed version: {}", installed_meta.unwrap().version)
    }
    else { println!() }

    if !config.build.allow_unfree && pkg.unfree {
        println!(" \
            [rad] {} This package is marked as proprietary! You can install this at your own risk,\n  \
            to allow rad installing proprietary packages, change the next option in {} to true:\n\n    \
            {}\n    \
            {} = true\n\n  \
            You was warned, that you have no warranty for this. You are on your own, good luck", "error:".red(), "/etc/rad/config.toml".bold(), "[build]".bold(), "allow_unfree".blue()
        );

        std::process::exit(1);
    }
    if config.build.ask && askable && going_install {
        let _ = ask_to_install();
    }

    if processing.contains(&pkg.name) {
        eprintln!(
            "[rad] {} circular dependency detected: {}!",
            "error:".red(),
            atom.yellow()
        );
        return;
    }

    processing.insert(pkg.name.clone());

    for dep in &pkg.depends {
        let dep_atom = index::resolve_with_source(dep, &config)
            .map(|(a, _)| a)
            .unwrap_or_else(|_| dep.clone());
        if !is_installed(&dep_atom) {
            println!("[rad] resolving dependency: {}", dep);
            install_package(dep, prefix, false, false, true, false, processing);
        }
    }

    let src_dir = match download_and_extract(&pkg) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[rad] {} {}", "error:".red(), e);
            processing.remove(&pkg.name);
            return;
        }
    };

    // 64 bit build
    let dest_dir = format!("/tmp/rad/image/{}", pkg.name);
    let _ = fs::remove_dir_all(&dest_dir);
    fs::create_dir_all(&dest_dir).unwrap();

    if let Err(e) = build_and_install(&pkg, &src_dir, prefix, &dest_dir, false) {
        eprintln!("[rad] {} {}", "build error:".red(), e);
        processing.remove(&pkg.name);
        return;
    }

    // 32 bit build if needed
    if config.arch.multilib && pkg.multilib_support {
        println!("[rad] this package is multilib, so i build 32-bit version now");
        if let Err(e) = build_and_install(&pkg, &src_dir, prefix, &dest_dir, true) {
            eprintln!("[rad] {} {}", "multilib build error:".red(), e);
            processing.remove(&pkg.name);
            return;
        }
    }

    // Read the old manifest before overwriting it
    let db_manifest = format!("/var/lib/rad/installed/{}", atom);
    let old_files: HashSet<String> = fs::read_to_string(&db_manifest)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

        
    // Now go register
    if going_install {
        let pb = spinner(&format!("registering files for {}...", pkg.name));
        if let Err(e) = register_package_files(&atom, &dest_dir) {
            eprintln!("[rad] registration error: {}", e);
        }
        if let Err(e) = write_meta(&atom, &pkg.version, category, &pkg.depends) {
            eprintln!("[rad] error writing meta: {}", e);
        }
        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
    
        // And now merging
        if let Err(e) = merge(&dest_dir, "/") {
            eprintln!("[rad] {} {}", "merge error:".red(), e);
            processing.remove(&pkg.name);
            return;
        }
    }
    else{
        if let Err(e) = merge(&dest_dir, &format!("{}/{}", &config.build.bin_cache_dir, pkg.name)) {
            eprintln!("[rad] {} {}", "merge error:".red(), e);
            processing.remove(&pkg.name);
            return;
        }
    }
    
    // Remove outdated files, that weren't installed in new version of package
    if (force || needs_upgrade) && !old_files.is_empty() {
        cleanup_orphaned_files(&atom, &old_files);
    }

    let _ = fs::remove_dir_all(&dest_dir);
    let build_dir = format!("/tmp/rad/build/{}", pkg_name);
    let _ = fs::remove_dir_all(&build_dir);

    processing.remove(&pkg.name);
    if going_install {
        println!("[rad] installation of {} finished successfully", atom.yellow());
    }
    else {
        println!("[rad] building binary of {} finished successfully", atom.yellow());
    }

    if needs_upgrade && going_install {
        for dependent in find_dependents(&pkg.name) {
            println!("[rad] {} depends on updated {}, rebuilding", dependent, atom);
            install_package(&dependent, prefix, true, false, true, false, processing);
        }
    }
}

pub fn download_and_extract(pkg: &Package) -> Result<String, String> {
    let work_dir = format!("/tmp/rad/build/{}", pkg.name);
    fs::create_dir_all(&work_dir).map_err(|e| format!("cannot create build dir: {}", e))?;

    if pkg.source.ends_with(".git")
        || (pkg.source.contains("github.com") && !pkg.source.contains(".tar"))
    {
        let mut cmd = Command::new("git");
        cmd.args(["clone", "--recursive", &pkg.source, &work_dir]);
        run_cmd(cmd, &format!("cloning {}", pkg.source))?;
        return Ok(work_dir);
    }

    let archive_name = pkg.source.split('/').next_back().unwrap_or("source.tar.gz");
    let archive_path = format!("{}/{}", work_dir, archive_name);

    let mut wget_cmd = Command::new("wget");
    wget_cmd.args(["-c", &pkg.source, "-O", &archive_path]);
    run_cmd(wget_cmd, &format!("downloading {}", archive_name))?;

    let extract_cmd = if archive_path.ends_with(".zip") {
        let mut c = Command::new("unzip");
        c.args([&archive_path, "-d", &work_dir]);
        c
    } else {
        let mut c = Command::new("tar");
        c.args(["-xf", &archive_path, "-C", &work_dir]);
        c
    };
    run_cmd(extract_cmd, &format!("extracting {}", archive_name))?;

    let versioned = format!("{}/{}-{}", work_dir, pkg.name, pkg.version);
    let plain = format!("{}/{}", work_dir, pkg.name);
    if Path::new(&versioned).exists() {
        return Ok(versioned);
    }
    if Path::new(&plain).exists() {
        return Ok(plain);
    }

    fs::read_dir(&work_dir)
        .map_err(|e| e.to_string())?
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
    let config = load_config();
    let cores = if config.build.cores == 0 {
        num_cpus::get().to_string()
    } else {
        config.build.cores.to_string()
    };

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
            if is_verbose() {
                println!("[rad] build system compiler: autotools");
            }
            let mut cmd = Command::new("./configure");
            cmd.arg(format!("--prefix={}", prefix))
                .arg(format!("--libdir={}", current_libdir))
                .current_dir(src_dir);
            for arg in &current_configure_args {
                cmd.arg(arg);
            }
            run_cmd(cmd, "configure")?;
            run_cmd(make_cmd(src_dir, &[&format!("-j{}", cores)]), "make")?;
            run_cmd(
                make_cmd(src_dir, &[&format!("DESTDIR={}", dest_dir), "install"]),
                "make install",
            )?;
        }

        BuildSystem::Make => {
            if is_verbose() {
                println!("[rad] build system compiler: make");
            }
            let mut args: Vec<String> = vec![format!("-j{}", cores)];
            for arg in &current_configure_args {
                args.push(arg.clone());
            }
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            run_cmd(make_cmd(src_dir, &args_ref), "make")?;
            run_cmd(
                make_cmd(
                    src_dir,
                    &[
                        &format!("DESTDIR={}", dest_dir),
                        &format!("PREFIX={}", prefix),
                        "install",
                    ],
                ),
                "make install",
            )?;
        }

        BuildSystem::Cmake => {
            if is_verbose() {
                println!("[rad] build system compiler: cmake/ninja");
            }
            let build_dir = format!("{}/build", src_dir);
            let _ = fs::remove_dir_all(&build_dir);
            fs::create_dir_all(&build_dir).unwrap();
            let mut cmd = Command::new("cmake");
            cmd.arg("..")
                .arg("-GNinja")
                .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix))
                .arg(format!("-DCMAKE_INSTALL_LIBDIR={}", current_libdir))
                .current_dir(&build_dir);
            if is_m32 {
                cmd.arg("-DCMAKE_C_FLAGS=-m32");
                cmd.arg("-DCMAKE_CXX_FLAGS=-m32");
            }
            for arg in &current_configure_args {
                cmd.arg(arg);
            }
            run_cmd(cmd, "cmake")?;
            run_cmd(ninja_cmd(&build_dir, &["-j", &cores]), "ninja")?;
            run_cmd(ninja_install_cmd(&build_dir, dest_dir), "ninja install")?;
        }

        BuildSystem::Meson => {
            if is_verbose() {
                println!("[rad] build system compiler: meson/ninja");
            }
            let build_dir = format!("{}/build", src_dir);
            let mut cmd = Command::new("meson");
            cmd.arg("setup")
                .arg(&build_dir)
                .arg(format!("--prefix={}", prefix))
                .arg(format!("--libdir={}", current_libdir))
                .current_dir(src_dir);
            for arg in &current_configure_args {
                cmd.arg(arg);
            }
            run_cmd(cmd, "meson setup")?;
            run_cmd(ninja_cmd(&build_dir, &["-j", &cores]), "ninja")?;
            run_cmd(ninja_install_cmd(&build_dir, dest_dir), "ninja install")?;
        }

        BuildSystem::Cargo => {
            if is_verbose() {
                println!("[rad] build system compiler: cargo");
            }
            let mut cmd = Command::new("cargo");
            cmd.arg("build")
                .arg("--release")
                .arg("--jobs")
                .arg(cores)
                .current_dir(src_dir);
            run_cmd(cmd, "cargo build")?;
            let bin_dest = format!("{}{}/bin", dest_dir, prefix);
            fs::create_dir_all(&bin_dest).unwrap();
            let bin_src = format!("{}/target/release/{}", src_dir, pkg.name);
            fs::copy(&bin_src, format!("{}/{}", bin_dest, pkg.name))
                .map_err(|e| format!("copy binary failed: {}", e))?;
        }

        BuildSystem::Python => {
            if is_verbose() {
                println!("[rad] build system compiler: python/pip");
            }
            let mut cmd = Command::new("pip");
            cmd.args(["install", "--prefix", prefix, "--root", dest_dir, "."])
                .current_dir(src_dir);
            run_cmd(cmd, "pip install")?;
        }

        BuildSystem::Manual {
            build_commands,
            install_commands,
        } => {
            if is_verbose() {
                println!("[rad] build system: manual");
            }
            for (i, cmd_str) in build_commands.iter().enumerate() {
                let mut cmd = Command::new("sh");
                cmd.arg("-c")
                    .arg(cmd_str)
                    .current_dir(src_dir)
                    .env("PREFIX", prefix)
                    .env("LIBDIR", &current_libdir)
                    .env("IS_M32", if is_m32 { "1" } else { "0" })
                    .env("RAD_MULTILIB", if config.arch.multilib { "1" } else { "0" })
                    .env("RAD_CORES", &cores);
                run_cmd(
                    cmd,
                    &format!("build step {}/{}: {}", i + 1, build_commands.len(), cmd_str),
                )?;
            }
            for (i, cmd_str) in install_commands.iter().enumerate() {
                let mut cmd = Command::new("sh");
                cmd.arg("-c")
                    .arg(cmd_str)
                    .current_dir(src_dir)
                    .env("DESTDIR", dest_dir)
                    .env("PREFIX", prefix)
                    .env("LIBDIR", &current_libdir)
                    .env("IS_M32", if is_m32 { "1" } else { "0" })
                    .env("RAD_MULTILIB", if config.arch.multilib { "1" } else { "0" })
                    .env("RAD_CORES", &cores);
                run_cmd(
                    cmd,
                    &format!("install step {}/{}: {}", i + 1, install_commands.len(), cmd_str),
                )?;
            }
        }
    }

    if is_verbose() {
        println!("[rad] build finished");
    }
    Ok(())
}

pub fn run_cmd(mut cmd: Command, label: &str) -> Result<(), String> {
    if is_verbose() {
        println!("[rad] {}...", label);
        let status = cmd
            .status()
            .map_err(|e| format!("{} failed to start: {}", label, e))?;
        if !status.success() {
            return Err(format!("{} exited with status: {}", label, status));
        }
        return Ok(());
    }

    let pb = spinner(&format!("{}...", label));
    let output = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("{} failed to start: {}", label, e))?;

    if output.status.success() {
        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
        println!("[rad] {} done", label);
        Ok(())
    }
    else {
        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
        println!("[rad] {} failed", label);
        
        // On failure we still want the logs, even without -v, so print whatever the command produced before bubbling up the error
        io::Write::write_all(&mut io::stdout(), &output.stdout).ok();
        io::Write::write_all(&mut io::stderr(), &output.stderr).ok();
        Err(format!("{} exited with status: {}", label, output.status))
    }
}

pub fn make_cmd(dir: &str, args: &[&str]) -> Command {
    let mut c = Command::new("make");
    for a in args {
        c.arg(a);
    }
    c.current_dir(dir);
    c
}

pub fn ninja_cmd(dir: &str, args: &[&str]) -> Command {
    let mut c = Command::new("ninja");
    for a in args {
        c.arg(a);
    }
    c.current_dir(dir);
    c
}

pub fn ninja_install_cmd(build_dir: &str, dest_dir: &str) -> Command {
    let mut c = Command::new("ninja");
    c.arg("install")
        .env("DESTDIR", dest_dir)
        .current_dir(build_dir);
    c
}

pub fn register_package_files(atom: &str, dest_dir: &str) -> std::io::Result<()> {
    let manifest_path = format!("/var/lib/rad/installed/{}", atom);
    if let Some(parent) = Path::new(&manifest_path).parent() {
        fs::create_dir_all(parent)?;
    }
    let mut manifest = fs::File::create(&manifest_path)?;
    let dest_path = Path::new(dest_dir);
    collect_files(dest_path, dest_path, &mut manifest)
}

pub fn cleanup_orphaned_files(pkg_name: &str, old_files: &HashSet<String>) {
    let db_manifest = format!("/var/lib/rad/installed/{}", pkg_name);
    let new_files: HashSet<String> = fs::read_to_string(&db_manifest)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    // Files owned by other packages, never touch these
    let shared = crate::remove::files_owned_by_others(pkg_name)
        .unwrap_or_default();

    let orphans: Vec<&String> = old_files
        .iter()
        .filter(|f| !new_files.contains(*f) && !shared.contains(*f))
        .collect();

    if orphans.is_empty() {
        return;
    }

    println!("[rad] cleaning {} orphaned file(s) from previous install", orphans.len());
    for path_str in orphans {
        let path = Path::new(path_str);
        if path.exists() {
            match fs::remove_file(path) {
                Ok(_) => {
                    crate::remove::prune_empty_dirs(path);
                }
                Err(e) => eprintln!("[rad] error: could not remove orphan {}: {}", path_str, e),
            }
        }
    }
}

pub fn collect_files(root: &Path, current: &Path, manifest: &mut fs::File) -> std::io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, manifest)?;
        } else {
            let relative = path.strip_prefix(root).unwrap();
            writeln!(manifest, "/{}", relative.display())?;
        }
    }
    Ok(())
}

pub fn merge(dest_dir: &str, install_path: &str) -> Result<(), String> {
    let pb = spinner("merging files...");
    let dest_path = Path::new(dest_dir);
    let result = merge_dir(dest_path, dest_path, Path::new(install_path))
        .map_err(|e| format!("merge failed: {}", e));
    match &pb {
        Some(pb) => pb.finish_and_clear(),
        None => println!("[rad] merge done"),
    }
    result
}

pub fn merge_dir(root: &Path, current: &Path, target_base: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let relative = src_path.strip_prefix(root).unwrap();
        let dest_path = target_base.join(relative);
        if file_type.is_dir() {
            fs::create_dir_all(&dest_path)?;
            merge_dir(root, &src_path, target_base)?;
        } else {
            atomic_install(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

// Atomic install
pub fn atomic_install(src: &Path, dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        let target = fs::read_link(src)?;
        if dest.exists() {
            let _ = fs::remove_file(dest);
        }
        unix_fs::symlink(target, dest)?;
    } else {
        let tmp_name = match dest.file_name() {
            Some(name) => {
                let mut n = name.to_os_string();
                n.push(".rad_new");
                n
            }
            None => return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "destination path has no file name",
            )),
        };
        let tmp = dest.with_file_name(tmp_name);
        fs::copy(src, &tmp)?;
        let _ = fs::set_permissions(&tmp, meta.permissions());
        fs::rename(&tmp, dest)?;
    }
    Ok(())
}

pub fn is_installed(name: &str) -> bool {
    Path::new(&format!("/var/lib/rad/installed/{}", name)).exists()
}