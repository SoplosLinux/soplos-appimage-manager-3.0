# Changelog

All notable changes to Soplos AppImage Manager are documented in this file.

## [3.0.0-1-beta] - 2026-10-19

### Added

- Added Unix manual page documentation (`soplos-appimage-manager.1`) installed to `/usr/share/man/man1/`.
- Full 8-language translations for desktop entry (`soplos-appimage-manager.desktop.hbs`) and AppStream metainfo.
- Registered MIME type associations for standalone AppImage files (`application/x-iso9660-appimage`, `application/vnd.appimage`).
- Packaged application screenshots into `/usr/share/doc/soplos-appimage-manager/screenshots/`.
- Updated Debian packaging metadata and bumped project release to `3.0.0-1-beta`.

## [3.0.0-beta] - 2026-10-03

First beta of Soplos AppImage Manager 3.0.

### Added

- Detects when an AppImage with the same name and size is already installed and warns instead of silently creating a duplicate copy.
- A visible error dialog for operations that fail (adding an AppImage, etc.) — previously these only logged to the developer console, so a failure looked like the app had done nothing.

### Fixed

- Metadata extraction (`detect_appimage_type`) read the entire AppImage file into memory just to check 3 magic bytes near the start. For multi-gigabyte AppImages this ran on every list refresh, including after unrelated actions, and repeated overlapping reads could exhaust the system's memory.
- Launching an AppImage from the list didn't redirect its stdout/stderr, so its own output mixed into the app's own logs, and didn't give it its own process group, so it stayed tied to the manager's session instead of running as a fully independent process — matching what the 2.x Python version did.
- Extraction (`7z`/`7zz` or the `--appimage-extract` fallback) that hit its timeout was force-killed with SIGKILL, which gives a FUSE-mounting AppImage runtime no chance to unmount cleanly and can leave a stuck mountpoint behind. It now sends SIGTERM first and only escalates to SIGKILL after a grace period.
- Extraction and other filesystem/network-bound commands now consistently run off the Tauri main thread via `spawn_blocking`, so they can't freeze the window while they work.
- Downloading an update (`download_update`) buffered the whole AppImage into memory via `read_to_vec()`, which also silently applied ureq's default 10MiB response body cap — any update bigger than that (routine for AppImages, which often run into the hundreds of MB or multiple GB) failed with "the response body is larger than request limit: 10485760". Now streams the download straight to disk instead, with no size ceiling and no in-memory copy of the whole file.
- New app icon.

## [3.0.0-alpha] - 2026-10-03

Full architecture rewrite of Soplos AppImage Manager, from Python 3 + GTK3 to Tauri 2. See the Architecture section of the README for details.

### Added

- Rust backend (`src-tauri/`) replacing the Python 3 + GTK3 codebase: adding, integrating, running, editing and removing AppImages; icon and metadata extraction via `7z`/`7zz` with an `--appimage-extract` fallback; a filesystem watcher that keeps the list in sync with changes made outside the app; update checks against GitHub, GitLab, Codeberg and direct download URLs.
- Svelte 4 + Tailwind CSS frontend (`src/`) replacing the GTK3 UI, with a hero banner, a list of integrated AppImages, and shared popup components for properties, updates and deletion.
- Interface in eight languages: English, Spanish, German, French, Italian, Portuguese, Romanian and Russian. It starts in the system language and can be changed from the interface.
- Soplos's own light/dark theme, independent of the desktop environment's GTK/Qt theme.
