//! Webview configuration for media rendering.
//!
//! WebKitGTK 2.54 refuses to display a custom-scheme URL (`asset://`) from a
//! page served over `http:`, which is what the dev build is (the UI is the
//! Vite dev server on `http://localhost:1420`). Packaged builds load from
//! `tauri://localhost` and pass the check. Adding the asset scheme to each
//! webview's CORS allowlist is the narrowest switch that clears the check;
//! other knobs (marking the scheme CORS-enabled, local, or display-isolated)
//! do not. Harmless in packaged builds, which never hit the block.

/// Puts `asset://*/*` on each webview's CORS allowlist on Linux.
pub fn allow_asset_on_http<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("asset-allowlist")
        .on_webview_ready(|webview| {
            #[cfg(target_os = "linux")]
            if let Err(error) = webview.with_webview(|platform| {
                use webkit2gtk::WebViewExt;
                platform.inner().set_cors_allowlist(&["asset://*/*"]);
            }) {
                log::warn!(
                    "Failed to allow asset: URLs on {}: {error}",
                    webview.label()
                );
            }
            #[cfg(not(target_os = "linux"))]
            let _ = webview;
        })
        .build()
}
