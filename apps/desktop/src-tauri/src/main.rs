#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if handle_scheduled_backup_cli() {
        return;
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init());
    manor_app::register(builder)
        .run(tauri::generate_context!())
        .expect("error while running Manor");
}

fn handle_scheduled_backup_cli() -> bool {
    let mut args = std::env::args_os();
    let _bin = args.next();
    let Some(flag) = args.next() else {
        return false;
    };
    if flag != std::ffi::OsStr::new(manor_app::safety::SCHEDULED_BACKUP_FLAG) {
        return false;
    }

    let Some(out_dir) = args.next() else {
        eprintln!(
            "{} requires an output directory",
            manor_app::safety::SCHEDULED_BACKUP_FLAG
        );
        std::process::exit(2);
    };

    match manor_app::safety::snapshot_commands::run_scheduled_backup(out_dir.into()) {
        Ok(path) => {
            eprintln!("Manor scheduled backup written to {}", path.display());
            true
        }
        Err(err) => {
            eprintln!("Manor scheduled backup failed: {err}");
            std::process::exit(1);
        }
    }
}
