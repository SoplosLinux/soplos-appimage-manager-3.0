# Changelog

All notable changes to Soplos AppImage Manager are documented in this file.

## [3.0.0] - 2026-11-17

First stable release.

## [3.0.0-6-beta] - 2026-11-04

### Fixed

- The application metadata points to the launcher that the package installs, `soplos-appimage-manager.desktop`, so the software centers link the entry of the menu with the program.

### Changed

- New screenshot of the list of AppImages.

## [3.0.0-5-beta] - 2026-10-29

### Changed

- The window no longer has the frame of the system: it is a card with rounded corners and its own minimize, maximize and close buttons at the top right. It can be moved from the top strip, the banner and the side bar, and double click on the strip maximizes it. The rounded corners need a compositing manager running, as the one of Xfce, Plasma or GNOME.

## [3.0.0-4-beta] - 2026-10-28

### Changed

- New application icon, in the sizes of the package, the window and the interface.
- New hero image, with the circuit style of the other Soplos 3.0 programs.
- New screenshots of the main window, the properties, the add dialog and the duplicate check, taken with the interface in English.

## [3.0.0-3-beta] - 2026-10-23

### Added

- The window follows the light or dark theme of the desktop. It starts in the theme of the desktop and notices when it changes. The theme is read from the XDG portal and, when that says nothing, from each desktop: the colors of `kdeglobals` on Plasma, the name of the GTK theme on Xfce (a guess from words such as "dark") and the color scheme of GNOME. The button of the window still switches the theme and keeps what it chose; switching to the theme the desktop has goes back to following it.

## [3.0.0-2-beta] - 2026-10-22

### Added

- `CHANGELOG.md` is installed as `/usr/share/doc/soplos-appimage-manager/changelog.gz`.
- AppStream metainfo: categories and keywords in the eight languages, the same ones as the desktop entry.
- The Debian copyright now declares Lucide (ISC), used for the interface icons.
- README: version, license, platform, Tauri, Rust and Svelte badges, and new sections on the three update methods (release APIs of GitHub, GitLab and Codeberg, direct download by modification date, and AppImageUpdate when installed), the recommended dependencies and where the files go.

### Changed

- Manual page rewritten: integration, metadata extraction, duplicate detection, customization, updates, live monitoring, the "Run as root" option, keyboard shortcuts and files. It is installed compressed, as `/usr/share/man/man1/soplos-appimage-manager.1.gz`.

### Fixed

- AppStream metainfo: the developer and project group identifiers were `org.soploslinux`. They are now `org.soplos`, matching the soplos.org domain and the other Soplos apps.
- AppStream metainfo: the screenshots were bare file names instead of web addresses, so software centers could not show them and the file did not validate. They now use their `raw.githubusercontent.com` address.
- Manual page no longer lists `~/.config/soplos-appimage-manager/`, which the application never used, and now lists `~/AppImages/.icons/`, where the extracted icons go.

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
