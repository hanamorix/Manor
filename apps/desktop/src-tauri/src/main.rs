#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let builder = tauri::Builder::default();
    life_app::register(builder)
        .run(tauri::generate_context!())
        .expect("error while running Life Assistant");
}
