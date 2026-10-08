// AppImage CRUD, ported from 2.x's core/appimage_manager.py. Everything
// here is plain filesystem work under the user's own home directory
// (~/AppImages/ and ~/.local/share/applications/) — nothing needs root
// except the per-AppImage opt-in "Run as root" launch flag, which only
// affects the Exec= line of the generated .desktop file, not anything this
// backend itself runs.

use super::extract::{detect_appimage_type, extract_metadata, parse_desktop_file};
use crate::utils::{appimages_dir, desktop_dir, icons_dir};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize, Clone)]
pub struct AppImage {
    pub id: String,
    pub name: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "iconPath")]
    pub icon_path: Option<String>,
    #[serde(rename = "iconPreview")]
    pub icon_preview: Option<String>,
    #[serde(rename = "desktopFilePath")]
    pub desktop_file_path: Option<String>,
    #[serde(rename = "appimageType")]
    pub appimage_type: String,
    pub size: u64,
    pub version: String,
    pub comment: String,
    pub categories: String,
    #[serde(rename = "updateUrl")]
    pub update_url: String,
    #[serde(rename = "runAsRoot")]
    pub run_as_root: bool,
}

#[derive(Deserialize)]
pub struct AppImageInfoInput {
    pub name: String,
    pub version: String,
    pub comment: String,
    pub categories: String,
    #[serde(rename = "updateUrl")]
    pub update_url: String,
    #[serde(rename = "runAsRoot")]
    pub run_as_root: bool,
}

fn safe_id(name: &str) -> String {
    name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

fn to_appimage(id: String, file_path: PathBuf, icon_path: Option<PathBuf>, desktop_file_path: Option<PathBuf>, version: String, comment: String, categories: String, update_url: String, run_as_root: bool) -> std::io::Result<AppImage> {
    let size = fs::metadata(&file_path)?.len();
    let appimage_type = detect_appimage_type(&file_path);
    let icon_preview = icon_path.as_deref().and_then(crate::utils::icon_data_url);
    Ok(AppImage {
        id,
        name: file_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        file_path: file_path.to_string_lossy().to_string(),
        icon_path: icon_path.map(|p| p.to_string_lossy().to_string()),
        icon_preview,
        desktop_file_path: desktop_file_path.map(|p| p.to_string_lossy().to_string()),
        appimage_type,
        size,
        version,
        comment,
        categories,
        update_url,
        run_as_root,
    })
}

#[tauri::command]
pub async fn list_installed() -> Result<Vec<AppImage>, String> {
    tauri::async_runtime::spawn_blocking(list_installed_blocking).await.map_err(|e| e.to_string())?
}

// The actual (synchronous) implementation of every command in this file is
// a `_blocking` function run through `spawn_blocking`. A command declared
// `async fn` only changes WHERE Tauri dispatches it (off the main GTK
// thread, via Tokio) — a fully synchronous body inside it still runs to
// completion on whatever Tokio worker thread picked it up, which on a VM
// with few cores can starve Tokio's own small worker pool for as long as
// extraction takes (ureq/std::process calls never yield). spawn_blocking
// moves the work to Tokio's separate, much larger blocking-thread pool,
// which is the actual fix — not just the `async` keyword by itself.
fn list_installed_blocking() -> Result<Vec<AppImage>, String> {
    let appimages_dir = appimages_dir()?;
    let desktop_dir = desktop_dir()?;

    // Map exec target path -> (desktop file path, parsed entry), built from
    // every .desktop whose TryExec/Exec points into ~/AppImages/.
    let mut desktop_map: std::collections::HashMap<String, (PathBuf, std::collections::HashMap<String, String>)> = std::collections::HashMap::new();
    if let Ok(entries) = fs::read_dir(&desktop_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let parsed = parse_desktop_file(&path);
            let exec_val = parsed.get("TryExec").cloned().or_else(|| parsed.get("Exec").and_then(|e| e.split_whitespace().next()).map(|s| s.trim_start_matches("pkexec ").to_string()));
            if let Some(exec_val) = exec_val {
                if exec_val.starts_with(&appimages_dir.to_string_lossy().to_string()) {
                    desktop_map.insert(exec_val, (path, parsed));
                }
            }
        }
    }

    let mut result = Vec::new();
    let Ok(entries) = fs::read_dir(&appimages_dir) else { return Ok(result) };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("appimage")).unwrap_or(false)).collect();
    paths.sort();

    for path in paths {
        let key = path.to_string_lossy().to_string();
        let match_entry = desktop_map.get(&key);

        let mut name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut version = String::new();
        let mut comment = String::new();
        let mut categories = "Utility;".to_string();
        let mut update_url = String::new();
        let mut run_as_root = false;
        let mut icon_path = None;
        let mut desktop_file_path = None;

        if let Some((desktop_path, entry)) = match_entry {
            desktop_file_path = Some(desktop_path.clone());
            if let Some(n) = entry.get("Name") {
                name = n.clone();
            }
            version = entry.get("X-AppImage-Version").cloned().unwrap_or_default();
            comment = entry.get("Comment").cloned().unwrap_or_default();
            categories = entry.get("Categories").cloned().unwrap_or_else(|| "Utility;".to_string());
            update_url = entry.get("X-AppImage-UpdateUrl").cloned().unwrap_or_default();
            run_as_root = entry.get("X-AppImage-RunAsRoot").map(|v| v == "true").unwrap_or(false);
            if let Some(icon_raw) = entry.get("Icon") {
                let p = Path::new(icon_raw);
                if p.is_absolute() && p.exists() {
                    icon_path = Some(p.to_path_buf());
                }
            }
        }

        let id = safe_id(&name);
        if let Ok(appimage) = to_appimage(id, path, icon_path, desktop_file_path, version, comment, categories, update_url, run_as_root) {
            result.push(AppImage { name, ..appimage });
        }
    }

    Ok(result)
}

fn write_desktop_file(desktop_path: &Path, name: &str, exec_path: &Path, icon: &str, comment: &str, categories: &str, version: &str, update_url: &str, run_as_root: bool) -> std::io::Result<()> {
    let exec_value = if run_as_root { format!("pkexec {}", exec_path.display()) } else { exec_path.display().to_string() };
    let mut content = format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec={exec_value}\nTryExec={}\nIcon={icon}\nCategories={categories}\n",
        exec_path.display()
    );
    if !comment.is_empty() {
        content.push_str(&format!("Comment={comment}\n"));
    }
    if !version.is_empty() {
        content.push_str(&format!("X-AppImage-Version={version}\n"));
    }
    if !update_url.is_empty() {
        content.push_str(&format!("X-AppImage-UpdateUrl={update_url}\n"));
    }
    if run_as_root {
        content.push_str("X-AppImage-RunAsRoot=true\n");
    }
    content.push_str("X-AppImage-Integrate=true\n");
    fs::write(desktop_path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(desktop_path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn unique_desktop_path(app_id: &str) -> Result<PathBuf, String> {
    let dir = desktop_dir()?;
    let mut path = dir.join(format!("soplos-appimage-{app_id}.desktop"));
    let mut i = 1;
    while path.exists() {
        path = dir.join(format!("soplos-appimage-{app_id}_{i}.desktop"));
        i += 1;
    }
    Ok(path)
}

fn unique_dest_path(dir: &Path, source: &Path) -> PathBuf {
    let mut dest = dir.join(source.file_name().unwrap_or_default());
    if dest == *source {
        return dest;
    }
    let stem = source.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = source.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut i = 1;
    while dest.exists() {
        dest = dir.join(format!("{stem}_{i}.{ext}"));
        i += 1;
    }
    dest
}

fn update_desktop_db() {
    if let Ok(dir) = desktop_dir() {
        let _ = std::process::Command::new("update-desktop-database").arg(dir).arg("-q").output();
    }
}

fn integrate(dest_path: PathBuf) -> Result<AppImage, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755));
    }

    let app_id = safe_id(&dest_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let icons_dir = icons_dir()?;
    let meta = extract_metadata(&dest_path, &app_id, &icons_dir);

    let name = meta.name.unwrap_or_else(|| dest_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let final_id = safe_id(&name);
    let icon_value = meta.icon_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "application-x-executable".to_string());

    let desktop_path = unique_desktop_path(&final_id)?;
    write_desktop_file(&desktop_path, &name, &dest_path, &icon_value, &meta.comment, &meta.categories, &meta.version, "", false).map_err(|e| e.to_string())?;
    update_desktop_db();

    to_appimage(final_id, dest_path, meta.icon_path, Some(desktop_path), meta.version, meta.comment, meta.categories, String::new(), false).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_appimage_file(app: tauri::AppHandle) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Select an AppImage")
            .add_filter("AppImage files", &["AppImage", "appimage"])
            .blocking_pick_file()
            .and_then(|fp| fp.into_path().ok())
            .map(|p| p.to_string_lossy().to_string())
    })
    .await
    .unwrap_or(None)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn add_appimage(sourcePath: String, copy: bool) -> Result<AppImage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let source = PathBuf::from(sourcePath);
        let dest_dir = appimages_dir()?;

        // Same file name + same size already sitting in ~/AppImages/ means
        // this exact AppImage was added before. Without this check
        // unique_dest_path() below just suffixes a new copy (_1, _2, ...)
        // every time, so clicking Add again on the same file re-extracts
        // and re-integrates a full duplicate instead of saying it's already
        // installed — neither this nor the 2.x Python version ever had this
        // check, it's new.
        if let Some(file_name) = source.file_name() {
            let existing = dest_dir.join(file_name);
            if existing.exists() && existing != source {
                if let (Ok(existing_meta), Ok(source_meta)) = (fs::metadata(&existing), fs::metadata(&source)) {
                    if existing_meta.len() == source_meta.len() {
                        return Err("ALREADY_INSTALLED".to_string());
                    }
                }
            }
        }

        let dest = unique_dest_path(&dest_dir, &source);

        if copy {
            fs::copy(&source, &dest).map_err(|e| e.to_string())?;
        } else {
            fs::rename(&source, &dest).or_else(|_| fs::copy(&source, &dest).and_then(|_| fs::remove_file(&source))).map_err(|e| e.to_string())?;
        }

        integrate(dest)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn integrate_existing(filePath: String) -> Result<AppImage, String> {
    tauri::async_runtime::spawn_blocking(move || integrate(PathBuf::from(filePath))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn run_appimage(filePath: String, desktopFilePath: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(desktop_path) = desktopFilePath {
            let desktop_name = Path::new(&desktop_path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let has_gtk_launch = std::env::var("PATH").ok().map(|p| p.split(':').any(|d| Path::new(d).join("gtk-launch").is_file())).unwrap_or(false);
            if has_gtk_launch && !desktop_name.is_empty() {
                std::process::Command::new("gtk-launch")
                    .arg(&desktop_name)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    // New process group: the launched AppImage must not
                    // share the manager's own pgid/session, or a signal
                    // aimed at the manager (terminal Ctrl+C, the dev
                    // terminal closing, the manager hanging on anything
                    // unrelated) reaches the AppImage too, and vice versa.
                    .process_group(0)
                    .spawn()
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
        std::process::Command::new(&filePath)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn delete_appimage(filePath: String, iconPath: Option<String>, desktopFilePath: Option<String>) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut success = true;
        if let Some(desktop_path) = &desktopFilePath {
            if Path::new(desktop_path).exists() {
                if fs::remove_file(desktop_path).is_err() {
                    success = false;
                }
            }
        }
        if let Some(icon_path) = &iconPath {
            let icons_dir = icons_dir().unwrap_or_default();
            let icon_path_buf = PathBuf::from(icon_path);
            if icon_path_buf.starts_with(&icons_dir) && icon_path_buf.exists() {
                let _ = fs::remove_file(&icon_path_buf);
            }
        }
        if Path::new(&filePath).exists() {
            if fs::remove_file(&filePath).is_err() {
                success = false;
            }
        }
        update_desktop_db();
        success
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn update_appimage_info(filePath: String, desktopFilePath: Option<String>, iconPath: Option<String>, input: AppImageInfoInput) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(desktop_path) = desktopFilePath else { return Ok(false) };
        if !Path::new(&desktop_path).exists() {
            return Ok(false);
        }
        let icon_value = iconPath.unwrap_or_else(|| "application-x-executable".to_string());
        write_desktop_file(
            Path::new(&desktop_path),
            &input.name,
            Path::new(&filePath),
            &icon_value,
            &input.comment,
            &input.categories,
            &input.version,
            &input.update_url,
            input.run_as_root,
        )
        .map_err(|e| e.to_string())?;
        update_desktop_db();
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn read_icon_preview(path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || crate::utils::icon_data_url(Path::new(&path))).await.unwrap_or(None)
}
