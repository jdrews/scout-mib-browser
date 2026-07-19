mod config;
mod mib;

use std::sync::{Arc, RwLock};
use tauri::Manager;

/// Thread-safe handle to the MIB resolver stored in Tauri app state.
#[derive(Clone)]
pub struct MibResolverState {
    inner: Arc<RwLock<mib::Resolver>>,
}

impl MibResolverState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(mib::Resolver::new())),
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Ensure the default config file exists on first run.
            config::ensure_config_file().expect("failed to create config file");

            let path = config::config_path();
            app.manage(config::ConfigHandle { path });

            // Initialize empty MIB resolver.
            app.manage(MibResolverState::new());

            let window = app.get_webview_window("main").unwrap();
            window.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            config::config_read,
            config::config_write,
            config::config_get_path,
            mib_load_directories,
            mib_resolve_oid,
            mib_reverse_lookup,
            mib_status,
        ])
        .run(tauri::generate_context!())
        .expect("error running Scout MIB Browser");
}

/// Loads all MIB files from the given directories.
///
/// Returns the total number of nodes loaded and any fallback MIB names.
#[tauri::command]
fn mib_load_directories(
    resolver: tauri::State<MibResolverState>,
    directories: Vec<String>,
) -> Result<MibLoadStatus, String> {
    let mut res = resolver.inner.write().map_err(|e| e.to_string())?;
    res.load_directories(&directories);

    Ok(MibLoadStatus {
        node_count: res.node_count(),
        fallback_mibs: res.fallback_mib_names().cloned().collect(),
    })
}

/// Resolves a dotted-decimal OID to its MIB node.
#[tauri::command]
fn mib_resolve_oid(
    resolver: tauri::State<MibResolverState>,
    oid: String,
) -> Result<Option<mib::MibNode>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.resolve(&oid).cloned())
}

/// Looks up a MIB node name and returns its OID.
#[tauri::command]
fn mib_reverse_lookup(
    resolver: tauri::State<MibResolverState>,
    name: String,
) -> Result<Option<String>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.reverse_lookup(&name).map(String::from))
}

/// Returns the current resolver status.
#[tauri::command]
fn mib_status(resolver: tauri::State<MibResolverState>) -> Result<MibLoadStatus, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(MibLoadStatus {
        node_count: res.node_count(),
        fallback_mibs: res.fallback_mib_names().cloned().collect(),
    })
}

/// Status response for MIB loading operations.
#[derive(serde::Serialize)]
struct MibLoadStatus {
    /// Total number of indexed MIB nodes.
    node_count: usize,
    /// Names of MIB modules loaded via regex fallback.
    fallback_mibs: Vec<String>,
}
