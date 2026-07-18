use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.show()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running Scout MIB Browser");
}
