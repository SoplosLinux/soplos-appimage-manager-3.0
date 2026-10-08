// Watches ~/AppImages/ and ~/.local/share/applications/ so the list
// refreshes itself when files change outside the app (a new AppImage
// dropped in by a file manager, a .desktop edited by hand, etc.) — ported
// from 2.x's GFileMonitor-based _setup_file_monitor, debounced the same
// 500 ms so a burst of events (like an extraction writing several files)
// only triggers one frontend reload.

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc;
use std::time::Duration;
use tauri::Emitter;

pub fn start(app: tauri::AppHandle) {
    let Ok(appimages_dir) = crate::utils::appimages_dir() else { return };
    let Ok(desktop_dir) = crate::utils::desktop_dir() else { return };

    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(_) => return,
        };
        let _ = watcher.watch(&appimages_dir, RecursiveMode::NonRecursive);
        let _ = watcher.watch(&desktop_dir, RecursiveMode::NonRecursive);

        loop {
            // Block for the first event, then drain anything else that
            // arrives within the debounce window before emitting once.
            let Ok(_first) = rx.recv() else { break };
            while rx.recv_timeout(Duration::from_millis(500)).is_ok() {}
            let _ = app.emit("appimages-changed", ());
        }
    });
}
