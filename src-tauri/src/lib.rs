use tauri::{WebviewUrl, WebviewWindowBuilder};

/// Phase 0 skeleton: opens the flyout page in a plain window so the toolchain and the
/// Vite multi-page build can be checked. Phase 4 replaces this with the tray + hidden flyout.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            WebviewWindowBuilder::new(app, "flyout", WebviewUrl::App("flyout.html".into()))
                .title("Buds Control")
                .inner_size(320.0, 240.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Buds Control");
}
