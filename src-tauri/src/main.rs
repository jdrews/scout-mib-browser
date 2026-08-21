mod config;
mod log;

use scout_mib as mib;
use scout_snmp as snmp;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tauri::Manager;
use tracing::warn;

/// Shared cancellation token for in-progress walks.
#[derive(Clone)]
pub struct WalkCancelToken {
    inner: Arc<AtomicBool>,
    handle: Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl Default for WalkCancelToken {
    fn default() -> Self {
        Self::new()
    }
}

impl WalkCancelToken {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)),
            handle: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Returns true if a cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::Acquire)
    }

    /// Signals the walk to stop and aborts the spawned task immediately.
    pub fn cancel(&self) {
        self.inner.store(true, Ordering::Release);
        if let Some(handle) = self.handle.lock().unwrap().take() {
            handle.abort();
        }
    }

    /// Resets the token so a new walk can be cancelled independently.
    pub fn reset(&self) {
        self.inner.store(false, Ordering::Release);
        *self.handle.lock().unwrap() = None;
    }

    /// Returns the inner Arc for passing to the engine.
    pub fn inner(&self) -> Arc<AtomicBool> {
        self.inner.clone()
    }

    /// Stores the JoinHandle so it can be aborted on cancel.
    pub fn set_handle(&self, handle: tokio::task::JoinHandle<()>) {
        *self.handle.lock().unwrap() = Some(handle);
    }
}

/// Thread-safe handle to the MIB resolver stored in Tauri app state.
#[derive(Clone)]
pub struct MibResolverState {
    inner: Arc<RwLock<mib::Resolver>>,
}

impl Default for MibResolverState {
    fn default() -> Self {
        Self::new()
    }
}

impl MibResolverState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(mib::Resolver::default())),
        }
    }
}

/// Thread-safe handle to the SNMP engine and its dedicated runtime, stored in Tauri app state.
#[derive(Clone)]
pub struct SnmpEngineState {
    inner: Arc<RwLock<snmp::SnmpEngine>>,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl SnmpEngineState {
    pub fn new() -> Result<Self, String> {
        // 8MB worker stacks: snmp2's connection code can recurse deeply and
        // overflow default tokio worker stacks (2MB).
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_stack_size(8 * 1024 * 1024)
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

        Ok(Self {
            inner: Arc::new(RwLock::new(snmp::SnmpEngine::new())),
            runtime: Arc::new(runtime),
        })
    }

    /// Handle for spawning work on the app-owned 8MB-stack runtime.
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }
}

/// Bridges the engine's walk streaming to Tauri IPC channels.
struct ChannelWalkSender {
    batch: tauri::ipc::Channel,
    complete: tauri::ipc::Channel,
}

impl snmp::WalkBatchSender for ChannelWalkSender {
    fn send_binding(&self, binding: &snmp::VariableBinding) -> bool {
        match serde_json::to_string(binding) {
            Ok(json) => self.batch.send(json.into()).is_ok(),
            Err(e) => {
                warn!("Walk batch serialization failed: {}", e);
                false
            }
        }
    }

    fn send_complete(&self, result: &snmp::ResultSet) {
        match serde_json::to_string(result) {
            Ok(json) => {
                if let Err(e) = self.complete.send(json.into()) {
                    warn!("Walk complete channel send failed: {}", e);
                }
            }
            Err(e) => warn!("Walk complete serialization failed: {}", e),
        }
    }
}

fn main() {
    let log_buffer = log::init_logging().expect("failed to initialize logging");
    tracing::info!("Scout MIB Browser started");
    let snmp_state = SnmpEngineState::new().expect("failed to create SNMP engine");

    let builder = tauri::Builder::default();

    #[cfg(feature = "wdio")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());

    #[cfg(feature = "wdio")]
    let builder = builder.plugin(tauri_plugin_wdio::init());

    builder
        .setup(move |app| {
            log::set_tauri_app_handle(app.handle().clone());

            config::ensure_config_file().expect("failed to create config file");

            let path = config::config_path();
            app.manage(config::ConfigHandle { path });

            app.manage(MibResolverState::new());
            app.manage(snmp_state.clone());
            app.manage(log_buffer);
            app.manage(WalkCancelToken::new());

            let window = app.get_webview_window("main").unwrap();
            window.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            config::config_read,
            config::config_write,
            config::config_get_path,
            config::config_write_target,
            mib_load_directories,
            mib_resolve_oid,
            mib_reverse_lookup,
            mib_status,
            mib_tree,
            mib_children,
            mib_search,
            mib_unload,
            mib_loaded_list,
            mib_table_columns,
            mib_oid_name_map,
            snmp_connect,
            snmp_get,
            snmp_get_next,
            snmp_walk_streaming,
            snmp_bulk_walk_streaming,
            snmp_cancel_walk,
            snmp_set,
            snmp_walk_table,
            fs_write_file,
            dialog_open_directory,
            dialog_save_file,
            log::log_read,
            log::log_clear,
            log::log_path,
        ])
        .run(tauri::generate_context!())
        .expect("error running Scout MIB Browser");
}

// ── MIB Commands ─────────────────────────────────────────────────────────────

/// Loads all MIB files from the given directories.
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

/// Returns the hierarchical MIB tree for rendering in the UI.
#[tauri::command]
fn mib_tree(resolver: tauri::State<MibResolverState>) -> Result<Vec<mib::TreeNode>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.build_tree())
}

/// Returns direct children of the given OID for lazy loading.
#[tauri::command]
fn mib_children(
    resolver: tauri::State<MibResolverState>,
    oid: String,
) -> Result<Vec<mib::TreeNode>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.get_children(&oid))
}

/// Searches for MIB nodes matching the given query (autocomplete).
#[tauri::command]
fn mib_search(
    resolver: tauri::State<MibResolverState>,
    query: String,
) -> Result<Vec<mib::MibNode>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.search(&query))
}

/// Unloads all nodes from the given MIB module.
#[tauri::command]
fn mib_unload(
    resolver: tauri::State<MibResolverState>,
    mib_name: String,
) -> Result<MibLoadStatus, String> {
    let mut res = resolver.inner.write().map_err(|e| e.to_string())?;
    res.unload_mib(&mib_name);
    Ok(MibLoadStatus {
        node_count: res.node_count(),
        fallback_mibs: res.fallback_mib_names().cloned().collect(),
    })
}

/// Returns metadata about all currently loaded MIB modules.
#[tauri::command]
fn mib_loaded_list(
    resolver: tauri::State<MibResolverState>,
) -> Result<Vec<mib::LoadedMibInfo>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.loaded_mibs())
}

/// Returns column OIDs for a TABLE node.
#[tauri::command]
fn mib_table_columns(
    resolver: tauri::State<MibResolverState>,
    table_oid: String,
) -> Result<Vec<String>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.get_table_columns(&table_oid))
}

/// Returns all OID → name pairs for frontend resolution.
#[tauri::command]
fn mib_oid_name_map(
    resolver: tauri::State<MibResolverState>,
) -> Result<Vec<(String, String)>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.oid_name_map())
}

// ── SNMP Commands ────────────────────────────────────────────────────────────

/// Tests connectivity to a Target (async — runs off the main thread).
#[tauri::command]
async fn snmp_connect(
    engine_state: tauri::State<'_, SnmpEngineState>,
    params: SnmpCommandParams,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let oids: Vec<String> = vec!["1.3.6.1.2.1.1.9.1.5.0".to_string()];

    // Run on the app-owned runtime (8MB worker stacks) to avoid tokio worker
    // stack overflow inside snmp2's connection code (which can recurse deeply).
    let engine = engine_state
        .inner
        .read()
        .map_err(|e| e.to_string())?
        .clone();
    let handle = engine_state.runtime_handle();
    let join_handle = handle.spawn(async move { engine.get(&target, &oids).await });

    // Await completion with a timeout.
    match tokio::time::timeout(std::time::Duration::from_secs(10), join_handle).await {
        Ok(Ok(rs)) => rs,
        Ok(Err(join_err)) => Err(format!("Task panicked: {}", join_err)),
        Err(_timeout) => Err("Connection timed out (10s)".to_string()),
    }
}

/// Executes a Get operation for the given OIDs.
#[tauri::command]
async fn snmp_get(
    engine_state: tauri::State<'_, SnmpEngineState>,
    params: SnmpCommandParams,
    oids: Vec<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let engine = engine_state
        .inner
        .read()
        .map_err(|e| e.to_string())?
        .clone();
    let handle = engine_state.runtime_handle();
    handle
        .spawn(async move { engine.get(&target, &oids).await })
        .await
        .map_err(|e| format!("Get task failed: {}", e))?
}

/// Executes a GetNext operation for the given OIDs.
#[tauri::command]
async fn snmp_get_next(
    engine_state: tauri::State<'_, SnmpEngineState>,
    params: SnmpCommandParams,
    oids: Vec<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let engine = engine_state
        .inner
        .read()
        .map_err(|e| e.to_string())?
        .clone();
    let handle = engine_state.runtime_handle();
    handle
        .spawn(async move { engine.get_next(&target, &oids).await })
        .await
        .map_err(|e| format!("GetNext task failed: {}", e))?
}

/// Executes a Walk operation from the given root OID (streaming via channels).
#[tauri::command]
async fn snmp_walk_streaming(
    engine_state: tauri::State<'_, SnmpEngineState>,
    cancel_token: tauri::State<'_, WalkCancelToken>,
    params: SnmpCommandParams,
    root_oid: String,
    batch_channel: tauri::ipc::Channel,
    complete_channel: tauri::ipc::Channel,
) -> Result<(), String> {
    cancel_token.reset();
    let target = build_target(&params);
    let engine = engine_state
        .inner
        .read()
        .map_err(|e| e.to_string())?
        .clone();
    let cancel = (*cancel_token).inner();
    let sender = Arc::new(ChannelWalkSender {
        batch: batch_channel,
        complete: complete_channel,
    });
    let handle = engine.walk_streaming(
        &engine_state.runtime_handle(),
        &target,
        &root_oid,
        sender,
        Some(cancel),
    );
    cancel_token.set_handle(handle);
    Ok(())
}

/// Executes a BulkWalk operation from the given root OID (streaming via channels).
#[tauri::command]
async fn snmp_bulk_walk_streaming(
    engine_state: tauri::State<'_, SnmpEngineState>,
    cancel_token: tauri::State<'_, WalkCancelToken>,
    params: SnmpCommandParams,
    root_oid: String,
    batch_channel: tauri::ipc::Channel,
    complete_channel: tauri::ipc::Channel,
) -> Result<(), String> {
    cancel_token.reset();
    let target = build_target(&params);
    if matches!(target.version, snmp::Version::V1) {
        return Err("BulkWalk is not supported in SNMPv1 — use Walk instead".to_string());
    }
    let engine = engine_state
        .inner
        .read()
        .map_err(|e| e.to_string())?
        .clone();
    let cancel = (*cancel_token).inner();
    let sender = Arc::new(ChannelWalkSender {
        batch: batch_channel,
        complete: complete_channel,
    });
    let handle = engine.bulk_walk_streaming(
        &engine_state.runtime_handle(),
        &target,
        &root_oid,
        sender,
        Some(cancel),
    );
    cancel_token.set_handle(handle);
    Ok(())
}

/// Cancels an in-progress walk.
#[tauri::command]
fn snmp_cancel_walk(cancel_token: tauri::State<'_, WalkCancelToken>) {
    cancel_token.cancel();
}

/// Executes a Set operation to write a value at the given OID.
#[tauri::command]
async fn snmp_set(
    engine_state: tauri::State<'_, SnmpEngineState>,
    params: SnmpCommandParams,
    oid: String,
    value_type: String,
    value: serde_json::Value,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let set_value = parse_set_value(&value_type, &value)?;
    let engine = engine_state
        .inner
        .read()
        .map_err(|e| e.to_string())?
        .clone();
    let handle = engine_state.runtime_handle();
    handle
        .spawn(async move { engine.set(&target, &oid, set_value).await })
        .await
        .map_err(|e| format!("Set task failed: {}", e))?
}

/// Walks all columns of a table and returns results as a pivoted grid.
#[tauri::command]
async fn snmp_walk_table(
    engine_state: tauri::State<'_, SnmpEngineState>,
    params: SnmpCommandParams,
    table_oid: String,
    column_oids: Vec<String>,
) -> Result<snmp::TableResult, String> {
    let target = build_target(&params);
    let engine = engine_state
        .inner
        .read()
        .map_err(|e| e.to_string())?
        .clone();
    let handle = engine_state.runtime_handle();
    handle
        .spawn(async move { engine.walk_table(&target, &table_oid, &column_oids).await })
        .await
        .map_err(|e| format!("WalkTable task failed: {}", e))?
}

// ── File System Commands ─────────────────────────────────────────────────────

/// Writes a string to the given file path.
#[tauri::command]
fn fs_write_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| format!("Failed to write {}: {}", path, e))?;
    Ok(())
}

/// Opens a native directory picker dialog.
#[tauri::command]
fn dialog_open_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let window = app
        .get_webview_window("main")
        .expect("main window not found");
    let path = rfd::FileDialog::new().set_parent(&window).pick_folder();
    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

/// Opens a native save file dialog.
#[tauri::command]
fn dialog_save_file(
    app: tauri::AppHandle,
    _default_path: String,
) -> Result<Option<String>, String> {
    let window = app
        .get_webview_window("main")
        .expect("main window not found");
    let path = rfd::FileDialog::new().set_parent(&window).save_file();
    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Shared Target connection parameters for SNMP commands.
#[derive(Clone, serde::Deserialize)]
struct SnmpCommandParams {
    host: String,
    port: u16,
    version: String,
    community: Option<String>,
    #[serde(default)]
    v3_username: Option<String>,
    #[serde(default)]
    v3_auth_protocol: Option<String>,
    #[serde(default)]
    v3_auth_passphrase: Option<String>,
    #[serde(default)]
    v3_priv_protocol: Option<String>,
    #[serde(default)]
    v3_priv_passphrase: Option<String>,
}

/// Builds a Target from command parameters.
fn build_target(params: &SnmpCommandParams) -> snmp::Target {
    let community = params
        .community
        .clone()
        .unwrap_or_else(|| "public".to_string());

    match params.version.to_lowercase().as_str() {
        "v1" => snmp::Target::v1(&params.host, params.port, community),
        "v3" => {
            let auth_protocol = match params.v3_auth_protocol.as_deref() {
                Some("md5") => snmp::AuthProtocol::Md5,
                Some("sha1") => snmp::AuthProtocol::Sha1,
                Some("sha224") => snmp::AuthProtocol::Sha224,
                Some("sha256") => snmp::AuthProtocol::Sha256,
                Some("sha384") => snmp::AuthProtocol::Sha384,
                Some("sha512") => snmp::AuthProtocol::Sha512,
                _ => snmp::AuthProtocol::None,
            };

            let priv_protocol = match params.v3_priv_protocol.as_deref() {
                Some("des") => snmp::PrivProtocol::Des,
                Some("aes128") => snmp::PrivProtocol::Aes128,
                Some("aes192") => snmp::PrivProtocol::Aes192,
                Some("aes256") => snmp::PrivProtocol::Aes256,
                _ => snmp::PrivProtocol::None,
            };

            let security = snmp::SnmpV3SecurityConfig {
                username: params.v3_username.clone().unwrap_or_default(),
                auth_protocol,
                auth_passphrase: params.v3_auth_passphrase.clone().unwrap_or_default(),
                priv_protocol,
                priv_passphrase: params.v3_priv_passphrase.clone().unwrap_or_default(),
            };
            snmp::Target::v3(&params.host, params.port, security)
        }
        _ => snmp::Target::v2c(&params.host, params.port, community),
    }
}

/// Parses a JSON value into a SetValue based on the type string.
fn parse_set_value(value_type: &str, value: &serde_json::Value) -> Result<snmp::SetValue, String> {
    match value_type.to_lowercase().as_str() {
        "integer" | "integer32" => {
            let v = value
                .as_i64()
                .ok_or_else(|| "Integer value expected".to_string())?;
            Ok(snmp::SetValue::Integer(v))
        }
        "octetstring" | "octet-string" | "displaystring" => {
            let s = value
                .as_str()
                .ok_or_else(|| "String value expected".to_string())?;
            Ok(snmp::SetValue::OctetString(s.as_bytes().to_vec()))
        }
        "gauge32" | "gauge" => {
            let v = value
                .as_u64()
                .ok_or_else(|| "Gauge32 value expected".to_string())?;
            Ok(snmp::SetValue::Unsigned32(v as u32))
        }
        "counter32" => {
            let v = value
                .as_u64()
                .ok_or_else(|| "Counter32 value expected".to_string())?;
            Ok(snmp::SetValue::Counter32(v as u32))
        }
        "counter64" => {
            let v = value
                .as_u64()
                .ok_or_else(|| "Counter64 value expected".to_string())?;
            Ok(snmp::SetValue::Counter64(v))
        }
        "ipaddress" | "ip-address" => {
            let s = value
                .as_str()
                .ok_or_else(|| "IP address string expected".to_string())?;
            Ok(snmp::SetValue::IpAddress(s.to_string()))
        }
        "timeticks" | "time-ticks" => {
            let v = value
                .as_u64()
                .ok_or_else(|| "TimeTicks value expected".to_string())?;
            Ok(snmp::SetValue::TimeTicks(v as u32))
        }
        "oid" | "objectidentifier" | "object-identifier" => {
            let s = value
                .as_str()
                .ok_or_else(|| "OID string expected".to_string())?;
            Ok(snmp::SetValue::ObjectIdentifier(s.to_string()))
        }
        _ => Err(format!("Unknown Set value type: {}", value_type)),
    }
}

/// Status response for MIB loading operations.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MibLoadStatus {
    /// Total number of indexed MIB nodes.
    node_count: usize,
    /// Names of MIB modules loaded via regex fallback.
    fallback_mibs: Vec<String>,
}
