// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK's DMABUF renderer aborts with "Error 71 (Protocol error) dispatching to
    // Wayland display" on some NVIDIA + Wayland setups. Turn it off unless the user chose
    // otherwise; this has to happen before GTK starts.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    omalook_lib::run()
}
