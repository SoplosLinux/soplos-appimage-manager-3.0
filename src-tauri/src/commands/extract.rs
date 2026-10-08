// AppImage metadata/icon extraction, ported from 2.x's
// core/appimage_manager.py _extract_metadata/_extract_appimage/_find_icon.
// Priority for extraction: 7zz/7z (fast, no execution of the binary) →
// --appimage-extract (always works, but has to actually run the AppImage).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// std::process has no built-in timeout, so extraction (which can hang if a
// third-party AppImage's own runtime misbehaves, e.g. needs FUSE that isn't
// available) is bounded by hand: spawn, poll non-blockingly, kill past the
// deadline. Matches the 60s/120s bounds the 2.x Python code had via
// subprocess.run(..., timeout=...) — dropping those during the port is what
// let a stuck extraction look exactly like the whole app freezing.
fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> bool {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else { return false };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => return true,
            Ok(None) => {
                if Instant::now() >= deadline {
                    // SIGTERM first so an AppImage runtime that mounted a
                    // FUSE squashfs (type 2, --appimage-extract) gets to run
                    // its own unmount handler. child.kill() alone sends
                    // SIGKILL, which skips that cleanup and leaves the
                    // mountpoint stuck for anything that touches it later.
                    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
                    let grace = Instant::now() + Duration::from_secs(5);
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => return false,
                            Ok(None) if Instant::now() < grace => std::thread::sleep(Duration::from_millis(100)),
                            _ => break,
                        }
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return false,
        }
    }
}

pub struct ExtractedMeta {
    pub name: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub version: String,
    pub comment: String,
    pub categories: String,
}

fn which(bin: &str) -> bool {
    std::env::var("PATH")
        .ok()
        .map(|path| path.split(':').any(|dir| Path::new(dir).join(bin).is_file()))
        .unwrap_or(false)
}

fn extract_appimage(appimage_path: &Path, dest_dir: &Path) -> Option<PathBuf> {
    let squashfs_root = dest_dir.join("squashfs-root");

    let sevenzip = if which("7zz") {
        Some("7zz")
    } else if which("7z") {
        Some("7z")
    } else {
        None
    };
    if let Some(tool) = sevenzip {
        let mut cmd = Command::new(tool);
        cmd.args([
            "x",
            &appimage_path.to_string_lossy(),
            &format!("-o{}", squashfs_root.to_string_lossy()),
            "-y",
            "-bso0",
            "-bsp0",
            "-ir!*.png",
            "-ir!*.svg",
            "-ir!*.desktop",
            "-ir0!.DirIcon",
        ])
        .current_dir(dest_dir);
        if !run_with_timeout(&mut cmd, Duration::from_secs(60)) {
            eprintln!("soplos-appimage-manager: 7z extraction of {} timed out after 60s", appimage_path.display());
        }
        if squashfs_root.exists() {
            return Some(squashfs_root);
        }
    }

    // Fallback: copy the AppImage into the work dir and run --appimage-extract
    // there (never execute it in place).
    let tmp_copy = dest_dir.join("app.appimage");
    if std::fs::copy(appimage_path, &tmp_copy).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp_copy, std::fs::Permissions::from_mode(0o755));
        }
        let mut cmd = Command::new(&tmp_copy);
        cmd.arg("--appimage-extract").current_dir(dest_dir);
        run_with_timeout(&mut cmd, Duration::from_secs(120));
        if squashfs_root.exists() {
            return Some(squashfs_root);
        }
    }

    None
}

fn find_icon(root: &Path, icon_name: &str) -> Option<PathBuf> {
    // 1. .DirIcon (file or symlink, including a relative target)
    let diricon = root.join(".DirIcon");
    if let Ok(link_target) = std::fs::read_link(&diricon) {
        let real = if link_target.is_absolute() {
            link_target
        } else {
            diricon.parent().unwrap_or(root).join(link_target)
        };
        if real.is_file() {
            return Some(real);
        }
    } else if diricon.is_file() {
        return Some(diricon);
    }

    let name_lower = icon_name.to_lowercase();

    // 2. Root-level {name}.svg / {name}.png, exact then case-insensitive
    for ext in ["svg", "png"] {
        let candidate = root.join(format!("{icon_name}.{ext}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext_ok = path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("svg") || e.eq_ignore_ascii_case("png")).unwrap_or(false);
            if ext_ok && path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_lowercase() == name_lower).unwrap_or(false) {
                return Some(path);
            }
        }
    }

    // 3. hicolor icon theme dirs, largest to smallest, PNG before SVG
    let hicolor = root.join("usr/share/icons/hicolor");
    for size in ["scalable/apps", "512x512/apps", "256x256/apps", "128x128/apps", "64x64/apps", "48x48/apps"] {
        for ext in ["png", "svg"] {
            let candidate = hicolor.join(size).join(format!("{icon_name}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
            let size_dir = hicolor.join(size);
            if let Ok(entries) = std::fs::read_dir(&size_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case(ext)).unwrap_or(false)
                        && path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_lowercase() == name_lower).unwrap_or(false)
                    {
                        return Some(path);
                    }
                }
            }
        }
    }

    // 4. usr/share/pixmaps/
    let pixmaps = root.join("usr/share/pixmaps");
    if pixmaps.is_dir() {
        for ext in ["png", "svg"] {
            let candidate = pixmaps.join(format!("{icon_name}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if let Ok(entries) = std::fs::read_dir(&pixmaps) {
            for entry in entries.flatten() {
                let path = entry.path();
                let ext_ok = path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("svg") || e.eq_ignore_ascii_case("png")).unwrap_or(false);
                if ext_ok && path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_lowercase() == name_lower).unwrap_or(false) {
                    return Some(path);
                }
            }
        }
    }

    // 5. Any .png then .svg anywhere under root, preferring a stem match
    for ext in ["png", "svg"] {
        let mut candidates = Vec::new();
        collect_by_ext(root, ext, &mut candidates);
        if let Some(exact) = candidates.iter().find(|c| c.file_stem().and_then(|s| s.to_str()).map(|s| s.to_lowercase() == name_lower).unwrap_or(false)) {
            return Some(exact.clone());
        }
        if let Some(first) = candidates.into_iter().next() {
            return Some(first);
        }
    }

    None
}

fn collect_by_ext(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_by_ext(&path, ext, out);
        } else if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case(ext)).unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Minimal .desktop reader — same approach as welcome-3.0/webapp-manager-3.0.
pub fn parse_desktop_file(path: &Path) -> HashMap<String, String> {
    let mut entry = HashMap::new();
    let Ok(content) = std::fs::read_to_string(path) else { return entry };
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[Desktop Entry]" {
            in_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            if in_section {
                break;
            }
            continue;
        }
        if !in_section || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            entry.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    entry
}

/// Extracts name, icon, version, comment and categories from an AppImage,
/// copying the icon into `icons_dir` under `app_id` before the extraction
/// work directory is cleaned up. Best-effort: returns whatever it could
/// find, never fails the caller.
pub fn extract_metadata(appimage_path: &Path, app_id: &str, icons_dir: &Path) -> ExtractedMeta {
    let mut result = ExtractedMeta { name: None, icon_path: None, version: String::new(), comment: String::new(), categories: "Utility;".to_string() };

    let tmp_dir = std::env::temp_dir().join(format!("soplos-appimage-extract-{app_id}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);

    if let Some(root) = extract_appimage(appimage_path, &tmp_dir) {
        let desktop_file = std::fs::read_dir(&root)
            .ok()
            .and_then(|entries| entries.flatten().find(|e| e.path().extension().and_then(|x| x.to_str()) == Some("desktop")))
            .map(|e| e.path())
            .or_else(|| {
                let mut found = None;
                find_desktop_recursive(&root, &mut found);
                found
            });

        if let Some(desktop_path) = desktop_file {
            let entry = parse_desktop_file(&desktop_path);
            if let Some(name) = entry.get("Name") {
                result.name = Some(name.clone());
            }
            result.version = entry.get("X-AppImage-Version").cloned().unwrap_or_default();
            result.comment = entry.get("Comment").cloned().unwrap_or_default();
            result.categories = entry.get("Categories").cloned().unwrap_or_else(|| "Utility;".to_string());

            let icon_name = entry.get("Icon").cloned().unwrap_or_default();
            let icon_name = icon_name.trim_end_matches(".png").trim_end_matches(".svg");
            if !icon_name.is_empty() {
                if let Some(icon_src) = find_icon(&root, icon_name) {
                    let suffix = icon_src.extension().and_then(|e| e.to_str()).unwrap_or("png");
                    let icon_dest = icons_dir.join(format!("{app_id}.{suffix}"));
                    if std::fs::copy(&icon_src, &icon_dest).is_ok() {
                        result.icon_path = Some(icon_dest);
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
    result
}

fn find_desktop_recursive(dir: &Path, found: &mut Option<PathBuf>) {
    if found.is_some() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_desktop_recursive(&path, found);
        } else if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
            *found = Some(path);
            return;
        }
        if found.is_some() {
            return;
        }
    }
}

/// Detects the AppImage ELF type from its magic bytes ("1" or "2"), or "?".
pub fn detect_appimage_type(path: &Path) -> String {
    // Reads only the first 11 bytes, never the whole file — this is called
    // on every list/add/integrate, including for multi-gigabyte AppImages,
    // and `std::fs::read` loading the entire file just to look at 3 magic
    // bytes at the end of a tiny header was the actual cause of the OOM
    // kills (confirmed via dmesg): several such reads overlapping in memory
    // for a 1+ GB AppImage, re-triggered by every list refresh, add up fast.
    use std::io::Read;
    let mut buf = [0u8; 11];
    let Ok(mut file) = std::fs::File::open(path) else {
        return "?".to_string();
    };
    if file.read_exact(&mut buf).is_err() {
        return "?".to_string();
    }
    let magic = &buf[8..11];
    if magic == [0x41, 0x49, 0x01] {
        "1".to_string()
    } else if magic == [0x41, 0x49, 0x02] {
        "2".to_string()
    } else {
        "?".to_string()
    }
}
