// Update checking, ported from 2.x's core/appimage_manager.py update
// methods. Three independent sources, tried in this priority order:
//   api    -> GitHub, GitLab or Codeberg/Forgejo project URL (releases API)
//   direct -> a direct link to an .appimage file (HEAD + Last-Modified)
//   tool   -> appimageupdatetool, reading the binary's own embedded .upd_info
// "none" means nothing is configured and no fallback tool is installed.

use serde::Serialize;
use std::path::Path;
use std::process::Command;

fn which(bin: &str) -> bool {
    std::env::var("PATH").ok().map(|path| path.split(':').any(|dir| Path::new(dir).join(bin).is_file())).unwrap_or(false)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_update_mode(updateUrl: String) -> String {
    if !updateUrl.is_empty() {
        if parse_github(&updateUrl).is_some() || parse_gitlab(&updateUrl).is_some() || parse_forgejo(&updateUrl).is_some() {
            return "api".to_string();
        }
        if updateUrl.to_lowercase().ends_with(".appimage") {
            return "direct".to_string();
        }
    }
    if which("appimageupdatetool") || which("AppImageUpdate") {
        return "tool".to_string();
    }
    "none".to_string()
}

fn parse_github(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://github.com/").or_else(|| url.strip_prefix("http://github.com/"))?;
    let mut parts = rest.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.split(['/', '?', '#']).next()?.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

fn parse_gitlab(url: &str) -> Option<(String, String, bool)> {
    let rest = url.strip_prefix("https://gitlab.com/").or_else(|| url.strip_prefix("http://gitlab.com/"))?;
    let mut parts = rest.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let repo_part = parts.next()?;
    let repo = repo_part.split(['/', '?', '#']).next()?.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo, url.contains("/-/packages")))
}

fn parse_forgejo(url: &str) -> Option<(String, String, String)> {
    let rest = url.strip_prefix("https://codeberg.org/").or_else(|| url.strip_prefix("http://codeberg.org/"))?;
    let mut parts = rest.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.split(['/', '?', '#']).next()?.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(("codeberg.org".to_string(), owner, repo))
}

#[derive(Serialize, Default)]
pub struct UpdateCheckResult {
    #[serde(rename = "hasUpdate")]
    pub has_update: Option<bool>,
    pub message: String,
    #[serde(rename = "latestVersion")]
    pub latest_version: String,
    #[serde(rename = "downloadUrl")]
    pub download_url: String,
    #[serde(rename = "assetName")]
    pub asset_name: String,
    #[serde(rename = "assetSize")]
    pub asset_size: u64,
}

fn version_numbers(v: &str) -> Vec<u32> {
    v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect()
}

fn current_arch_aliases() -> &'static [&'static str] {
    match std::env::consts::ARCH {
        "x86_64" => &["x86_64", "amd64"],
        "aarch64" => &["aarch64", "arm64"],
        other => {
            // consts::ARCH is 'static, so this leak-free match covers the
            // common cases; anything else just matches on itself below.
            if other == "x86" {
                &["i686", "x86"]
            } else {
                &["x86_64", "amd64"]
            }
        }
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn check_for_update_api(updateUrl: String, currentVersion: String, currentFileStem: String) -> UpdateCheckResult {
    tauri::async_runtime::spawn_blocking(move || check_for_update_api_blocking(updateUrl, currentVersion, currentFileStem))
        .await
        .unwrap_or_else(|e| UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() })
}

// See the comment on list_installed_blocking in appimages.rs: the actual
// network/subprocess work for every command in this file runs through
// spawn_blocking, not just inside an `async fn`, so it can't starve
// Tokio's small worker pool on a low-core machine.
#[allow(non_snake_case)]
fn check_for_update_api_blocking(updateUrl: String, currentVersion: String, currentFileStem: String) -> UpdateCheckResult {
    #[derive(Debug)]
    struct Asset {
        name: String,
        url: String,
        size: u64,
    }

    let mut assets: Vec<Asset> = Vec::new();
    let latest_version;

    if let Some((owner, repo)) = parse_github(&updateUrl) {
        let api_url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
        match ureq::get(&api_url).header("User-Agent", "soplos-appimage-manager/3.0").header("Accept", "application/vnd.github+json").call() {
            Ok(mut resp) => {
                let bytes = match resp.body_mut().read_to_vec() {
                    Ok(v) => v,
                    Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
                };
                let data: serde_json::Value = match serde_json::from_slice(&bytes) {
                    Ok(v) => v,
                    Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
                };
                latest_version = data.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").trim_start_matches('v').to_string();
                if let Some(arr) = data.get("assets").and_then(|v| v.as_array()) {
                    for a in arr {
                        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if name.to_lowercase().ends_with(".appimage") {
                            assets.push(Asset {
                                name,
                                url: a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                size: a.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                            });
                        }
                    }
                }
            }
            Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
        }
    } else if let Some((host, owner, repo)) = parse_forgejo(&updateUrl) {
        let api_url = format!("https://{host}/api/v1/repos/{owner}/{repo}/releases?limit=1");
        match ureq::get(&api_url).header("User-Agent", "soplos-appimage-manager/3.0").call() {
            Ok(mut resp) => {
                let bytes = match resp.body_mut().read_to_vec() {
                    Ok(v) => v,
                    Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
                };
                let data: serde_json::Value = match serde_json::from_slice(&bytes) {
                    Ok(v) => v,
                    Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
                };
                let releases = data.as_array().cloned().unwrap_or_default();
                let Some(release) = releases.first() else {
                    return UpdateCheckResult { has_update: None, message: "No releases found".to_string(), ..Default::default() };
                };
                latest_version = release.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").trim_start_matches('v').to_string();
                if let Some(arr) = release.get("assets").and_then(|v| v.as_array()) {
                    for a in arr {
                        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if name.to_lowercase().ends_with(".appimage") {
                            assets.push(Asset {
                                name,
                                url: a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                size: a.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                            });
                        }
                    }
                }
            }
            Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
        }
    } else if let Some((owner, repo, _is_pkg)) = parse_gitlab(&updateUrl) {
        // Releases API only (the 2.x package-registry path is a rarer case,
        // kept out here to avoid chaining a second request per package).
        let encoded = format!("{owner}%2F{repo}");
        let api_url = format!("https://gitlab.com/api/v4/projects/{encoded}/releases");
        match ureq::get(&api_url).header("User-Agent", "soplos-appimage-manager/3.0").call() {
            Ok(mut resp) => {
                let bytes = match resp.body_mut().read_to_vec() {
                    Ok(v) => v,
                    Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
                };
                let data: serde_json::Value = match serde_json::from_slice(&bytes) {
                    Ok(v) => v,
                    Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
                };
                let releases = data.as_array().cloned().unwrap_or_default();
                let Some(release) = releases.first() else {
                    return UpdateCheckResult { has_update: None, message: "No releases found on GitLab".to_string(), ..Default::default() };
                };
                latest_version = release.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").trim_start_matches('v').to_string();
                if let Some(links) = release.get("assets").and_then(|v| v.get("links")).and_then(|v| v.as_array()) {
                    for link in links {
                        let name = link.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let url = link.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if name.to_lowercase().contains(".appimage") || url.to_lowercase().contains(".appimage") {
                            assets.push(Asset { name, url, size: 0 });
                        }
                    }
                }
            }
            Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
        }
    } else {
        return UpdateCheckResult { has_update: None, message: "Not a valid GitHub, GitLab or Codeberg URL".to_string(), ..Default::default() };
    }

    if assets.is_empty() {
        return UpdateCheckResult { has_update: None, message: "No AppImage assets found in the latest release".to_string(), latest_version, ..Default::default() };
    }

    let mut current_version = currentVersion.trim_start_matches('v').to_string();
    if current_version.is_empty() {
        // Fallback: pull a version-looking fragment out of the filename.
        if let Some(pos) = currentFileStem.rfind(['-', '_']) {
            let tail = &currentFileStem[pos + 1..];
            let cleaned = tail.trim_start_matches('v');
            if cleaned.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                current_version = cleaned.to_string();
            }
        }
    }

    if current_version.is_empty() {
        return UpdateCheckResult {
            has_update: None,
            message: format!("Cannot determine current version to compare against v{latest_version}. Please set version in Properties."),
            latest_version,
            ..Default::default()
        };
    }

    let has_update = version_numbers(&latest_version) > version_numbers(&current_version);

    let preferred = current_arch_aliases();
    let best = assets.iter().find(|a| preferred.iter().any(|p| a.name.to_lowercase().contains(p))).unwrap_or(&assets[0]);

    UpdateCheckResult {
        has_update: Some(has_update),
        message: format!("v{latest_version}  (current: v{current_version})"),
        latest_version,
        download_url: best.url.clone(),
        asset_name: best.name.clone(),
        asset_size: best.size,
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn check_for_update_direct(updateUrl: String, currentFilePath: String) -> UpdateCheckResult {
    tauri::async_runtime::spawn_blocking(move || check_for_update_direct_blocking(updateUrl, currentFilePath))
        .await
        .unwrap_or_else(|e| UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() })
}

#[allow(non_snake_case)]
fn check_for_update_direct_blocking(updateUrl: String, currentFilePath: String) -> UpdateCheckResult {
    let mut asset_name = updateUrl.rsplit('/').next().unwrap_or("").to_string();
    if !asset_name.to_lowercase().ends_with(".appimage") {
        asset_name.push_str(".AppImage");
    }

    let resp = match ureq::head(&updateUrl).header("User-Agent", "soplos-appimage-manager/3.0").call() {
        Ok(r) => r,
        Err(e) => return UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
    };
    let last_modified = resp.headers().get("Last-Modified").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let content_length: u64 = resp.headers().get("Content-Length").and_then(|v| v.to_str().ok()).and_then(|s| s.parse().ok()).unwrap_or(0);

    let (has_update, message) = match last_modified {
        Some(lm) => {
            let file_mtime = std::fs::metadata(&currentFilePath).and_then(|m| m.modified()).ok();
            match (httpdate::parse_http_date(&lm).ok(), file_mtime) {
                (Some(remote), Some(local)) => {
                    let newer = remote > local;
                    (newer, if newer { "A newer version is available".to_string() } else { "File appears to be up to date".to_string() })
                }
                _ => (true, "Cannot compare dates, offering download".to_string()),
            }
        }
        None => (true, "Server does not provide version info".to_string()),
    };

    UpdateCheckResult { has_update: Some(has_update), message, latest_version: String::new(), download_url: updateUrl, asset_name, asset_size: content_length }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn download_update(downloadUrl: String, assetName: String, oldFilePath: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || download_update_blocking(downloadUrl, assetName, oldFilePath)).await.map_err(|e| e.to_string())?
}

#[allow(non_snake_case)]
fn download_update_blocking(downloadUrl: String, assetName: String, oldFilePath: String) -> Result<String, String> {
    let appimages_dir = crate::utils::appimages_dir()?;
    let mut asset_name = assetName;
    if !asset_name.to_lowercase().ends_with(".appimage") {
        asset_name.push_str(".AppImage");
    }

    let old_path = std::path::PathBuf::from(&oldFilePath);
    let mut new_path = appimages_dir.join(&asset_name);
    if new_path.exists() && new_path != old_path {
        let stem = new_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = new_path.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut i = 1;
        while new_path.exists() && new_path != old_path {
            new_path = appimages_dir.join(format!("{stem}_{i}.{ext}"));
            i += 1;
        }
    }
    let tmp_path = new_path.with_extension("part");

    // AppImages routinely run into the hundreds of MB to multiple GB —
    // read_to_vec() both buffers the whole thing in memory AND enforces
    // ureq's default 10MiB response body cap (the exact "larger than
    // request limit: 10485760" error this used to fail with). Streaming
    // straight from the response reader to the file avoids both: no
    // in-memory copy of a multi-GB download, and no size ceiling at all.
    let mut resp = ureq::get(&downloadUrl).header("User-Agent", "soplos-appimage-manager/3.0").call().map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
    std::io::copy(&mut resp.body_mut().as_reader(), &mut file).map_err(|e| e.to_string())?;
    drop(file);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }

    if old_path.exists() && old_path != new_path {
        let _ = std::fs::remove_file(&old_path);
    }
    std::fs::rename(&tmp_path, &new_path).map_err(|e| e.to_string())?;

    Ok(new_path.to_string_lossy().to_string())
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn check_for_update_tool(filePath: String) -> UpdateCheckResult {
    tauri::async_runtime::spawn_blocking(move || check_for_update_tool_blocking(&filePath))
        .await
        .unwrap_or_else(|e| UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() })
}

fn check_for_update_tool_blocking(file_path: &str) -> UpdateCheckResult {
    let tool = if which("appimageupdatetool") { "appimageupdatetool" } else if which("AppImageUpdate") { "AppImageUpdate" } else {
        return UpdateCheckResult { has_update: None, message: "appimageupdatetool is not installed".to_string(), ..Default::default() };
    };
    match Command::new(tool).args(["--check-for-update", file_path]).output() {
        Ok(output) => {
            let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).trim().to_string();
            let has_update = match output.status.code() {
                Some(1) => Some(true),
                Some(0) => Some(false),
                _ => None,
            };
            UpdateCheckResult { has_update, message: text, ..Default::default() }
        }
        Err(e) => UpdateCheckResult { has_update: None, message: e.to_string(), ..Default::default() },
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn run_update_tool(filePath: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let tool = if which("appimageupdatetool") { "appimageupdatetool" } else if which("AppImageUpdate") { "AppImageUpdate" } else {
            return Err("appimageupdatetool is not installed".to_string());
        };
        let status = Command::new(tool).args(["--overwrite", &filePath]).status().map_err(|e| e.to_string())?;
        Ok(status.success())
    })
    .await
    .map_err(|e| e.to_string())?
}
