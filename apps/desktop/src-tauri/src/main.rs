#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// TODO(task-9): Replace placeholder icons at src-tauri/icons/ with real assets
//               generated via `pnpm exec tauri icon`. Current files are all
//               128x128 RGBA PNGs with fake .icns/.ico extensions — sufficient
//               for `cargo check` but will fail real bundle signing.
fn main() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init());
    manor_app::register(builder)
        .run(tauri::generate_context!())
        .expect("error while running Manor");
}
