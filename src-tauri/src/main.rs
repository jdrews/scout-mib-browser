mod config;
mod log;

use scout_mib as mib;
use scout_snmp as snmp;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tauri::Manager;
use tracing::{info, warn};

/// Shared cancellation token for in-progress SNMP operations. One token is
/// used by every cancellable command — the three streaming ones (Walk, Bulk
/// Walk, Get Table) and, since launch-beta/08, Get / GetNext / Set.
#[derive(Clone)]
pub struct WalkCancelToken {
    inner: Arc<AtomicBool>,
    handle: Arc<std::sync::Mutex<Option<tokio::task::AbortHandle>>>,
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

    /// Stores the task's abort handle so cancel() can stop it immediately.
    pub fn set_handle(&self, handle: tokio::task::AbortHandle) {
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

    /// Runs `f` with read access to the resolver on the app-owned runtime
    /// (8MB worker/blocking stacks — AGENTS.md backend layout). All MIB read
    /// commands go through here: the main thread never walks the tree, and
    /// blocking work stays on the one runtime whose stacks the app controls.
    pub async fn read_with<T, F>(
        &self,
        runtime: &tokio::runtime::Handle,
        label: &str,
        f: F,
    ) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&mib::Resolver) -> T + Send + 'static,
    {
        let inner = self.inner.clone();
        runtime
            .spawn_blocking(move || {
                let res = inner.read().map_err(|e| e.to_string())?;
                Ok(f(&res))
            })
            .await
            .map_err(|e| format!("{label} task failed: {e}"))?
    }
}

/// Handle to the SNMP engine and its dedicated runtime, stored in Tauri app state.
#[derive(Clone)]
pub struct SnmpEngineState {
    engine: snmp::SnmpEngine,
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
            engine: snmp::SnmpEngine::new(),
            runtime: Arc::new(runtime),
        })
    }

    /// Handle for spawning work on the app-owned 8MB-stack runtime.
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    /// Spawns an engine operation on the app-owned runtime, registers its
    /// abort handle with the shared cancel token, and awaits the result
    /// (launch-beta/08). Stop/Escape aborts the task at once; the aborted
    /// join maps to `Cancelled by user` so the UI reads the same text the
    /// engine's token checks produce.
    pub async fn run_cancellable<T, F>(
        &self,
        label: &str,
        cancel_token: &WalkCancelToken,
        op: F,
    ) -> Result<T, String>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        cancel_token.reset();
        let join = self.runtime.spawn(op);
        cancel_token.set_handle(join.abort_handle());
        join.await.map_err(|e| {
            if e.is_cancelled() {
                "Cancelled by user".to_string()
            } else {
                format!("{} task failed: {}", label, e)
            }
        })
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

/// Bridges the engine's table-retrieval streaming to Tauri IPC channels.
struct ChannelTableSender {
    progress: tauri::ipc::Channel,
    complete: tauri::ipc::Channel,
}

impl snmp::TableRowSender for ChannelTableSender {
    fn send_progress(&self, count: usize) -> bool {
        match serde_json::to_string(&count) {
            Ok(json) => self.progress.send(json.into()).is_ok(),
            Err(e) => {
                warn!("Table progress serialization failed: {}", e);
                false
            }
        }
    }

    fn send_rows(&self, rows: &[snmp::TableRow]) -> bool {
        let msg = snmp::TableStreamMsg::Rows {
            rows: rows.to_vec(),
        };
        match serde_json::to_string(&msg) {
            Ok(json) => self.complete.send(json.into()).is_ok(),
            Err(e) => {
                warn!("Table row batch serialization failed: {}", e);
                false
            }
        }
    }

    fn send_complete(&self, meta: &snmp::TableComplete) {
        let msg = snmp::TableStreamMsg::Complete { meta: meta.clone() };
        match serde_json::to_string(&msg) {
            Ok(json) => {
                if let Err(e) = self.complete.send(json.into()) {
                    warn!("Table complete channel send failed: {}", e);
                }
            }
            Err(e) => warn!("Table complete serialization failed: {}", e),
        }
    }
}

/// Maps MIB index metadata to the SNMP crate's decode specs.
fn mib_index_specs(
    resolver: &RwLock<mib::Resolver>,
    table_oid: &str,
) -> Vec<snmp::IndexColumnSpec> {
    let Ok(guard) = resolver.read() else {
        return Vec::new();
    };
    guard
        .get_table_info(table_oid)
        .map(|info| {
            info.index_columns
                .iter()
                .map(|c| snmp::IndexColumnSpec {
                    name: c.name.clone(),
                    implied: c.implied,
                    encoding: match &c.encoding {
                        mib::IndexEncoding::Integer => snmp::IndexEncoding::Integer,
                        mib::IndexEncoding::IpAddress => snmp::IndexEncoding::IpAddress,
                        mib::IndexEncoding::FixedString(n) => snmp::IndexEncoding::FixedString(*n),
                        mib::IndexEncoding::Variable => snmp::IndexEncoding::Variable,
                    },
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Startup banner for the System Log (launch-beta/12): beta bug reports
/// need a build and a platform to point at. The version is the crate
/// version — `tauri_build` syncs it from `tauri.conf.json`, which the
/// release pipeline stamps with the tag.
fn startup_banner() -> String {
    format!(
        "Scout MIB Browser {} on {}-{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

fn main() {
    let log_buffer = log::init_logging().expect("failed to initialize logging");
    tracing::info!("{}", startup_banner());
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
            // Validate at startup: a corrupt config is quarantined to a
            // timestamped backup and the app starts from defaults. The
            // warning lands in the System Log via the UI log layer
            // (launch-beta/05).
            config::load_config_quarantining(&path);
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
            mib_load,
            mib_resolve_oid,
            mib_reverse_lookup,
            mib_status,
            mib_tree,
            mib_children,
            mib_subtree,
            mib_search,
            mib_unload_many,
            mib_loaded_list,
            mib_table_columns,
            mib_table_info,
            mib_node_details,
            mib_oid_name_map,
            snmp_connect,
            snmp_get,
            snmp_get_next,
            snmp_walk_streaming,
            snmp_bulk_walk_streaming,
            snmp_cancel_walk,
            snmp_set,
            snmp_get_table,
            dialog_open_directory,
            dialog_open_files,
            dialog_save_file,
            log::log_read,
            log::log_clear,
            log::log_path,
            log::log_frontend,
        ])
        .run(tauri::generate_context!())
        .expect("error running Scout MIB Browser");
}

// ── MIB Commands ─────────────────────────────────────────────────────────────

/// Loads MIB files from the given directories **plus** the given explicit
/// files, plus the bundled standards MIBs when neither source has content.
///
/// Runs off the main thread on the app-owned runtime (launch-beta/07): the
/// file stamps are snapshotted under a short read lock, directories are
/// scanned and new/changed files are parsed **outside the resolver lock**
/// (8MB stacks — mib-rs parses recursively and a load-path panic is fatal),
/// and only the final index swap is serialized. Read commands (tree,
/// search, Manage MIBs list) therefore stay responsive while a load is in
/// flight.
#[tauri::command]
async fn mib_load(
    app: tauri::AppHandle,
    engine_state: tauri::State<'_, SnmpEngineState>,
    resolver: tauri::State<'_, MibResolverState>,
    directories: Vec<String>,
    files: Vec<String>,
) -> Result<MibLoadStatus, String> {
    let mut dirs = directories.clone();
    // The bundled standards MIBs cover systems without a system MIB directory
    // (Windows/macOS, or Linux without net-snmp). They are skipped when any
    // user directory has content or explicit files are configured, so bundled
    // copies never shadow user MIBs.
    if !directories.iter().any(|d| dir_has_entries(d)) && files.is_empty() {
        if let Some(bundled) = bundled_mibs_dir(&app) {
            info!(
                "No MIB files in configured directories; using bundled MIBs at {}",
                bundled
            );
            dirs.insert(0, bundled);
        }
    }

    let inner = resolver.inner.clone();
    let stats = engine_state
        .runtime_handle()
        .spawn_blocking(move || -> Result<mib::LoadStats, String> {
            let stamps = {
                let res = inner.read().map_err(|e| e.to_string())?;
                res.file_stamps()
            };
            let plan = mib::Resolver::scan_and_parse(&stamps, &dirs, &files);
            let mut res = inner.write().map_err(|e| e.to_string())?;
            Ok(res.apply_load(plan))
        })
        .await
        .map_err(|e| format!("mib_load task failed: {}", e))??;

    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(MibLoadStatus {
        node_count: res.node_count(),
        fallback_mibs: res.fallback_mib_names().cloned().collect(),
        files_parsed: Some(stats.parsed),
        files_cached: Some(stats.cached),
    })
}

/// Path to the MIBs bundled with the app, if present.
///
/// Bundled apps find them under the Tauri resource dir (copied there by the
/// bundler); dev/source builds fall back to the checkout's `src-tauri/mibs`.
fn bundled_mibs_dir(app: &tauri::AppHandle) -> Option<String> {
    find_bundled_mibs_dir(app.path().resource_dir().ok().as_deref())
        .map(|dir| dir.to_string_lossy().to_string())
}

/// Picks the first existing MIB directory: the resource-dir copy (bundled
/// apps), then the checkout's `src-tauri/mibs` (dev/source builds).
fn find_bundled_mibs_dir(resource_dir: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = resource_dir {
        candidates.push(dir.join("mibs"));
    }
    candidates.push(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mibs"));
    candidates.into_iter().find(|dir| dir.is_dir())
}

/// True when the directory exists and contains at least one entry.
fn dir_has_entries(dir: &str) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_some())
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
    Ok(mib_status_of(&res))
}

/// Returns the hierarchical MIB tree for rendering in the UI.
///
/// Async (launch-beta/07): the tree walk runs on the app-owned runtime —
/// Tauri runs non-`async` commands on the main thread, and a full tree
/// build over 100k+ nodes would freeze the UI.
#[tauri::command]
async fn mib_tree(
    engine_state: tauri::State<'_, SnmpEngineState>,
    resolver: tauri::State<'_, MibResolverState>,
) -> Result<Vec<mib::TreeNode>, String> {
    resolver
        .read_with(&engine_state.runtime_handle(), "mib_tree", |res| {
            res.build_tree()
        })
        .await
}

/// Returns direct children of the given OID for lazy loading.
#[tauri::command]
async fn mib_children(
    engine_state: tauri::State<'_, SnmpEngineState>,
    resolver: tauri::State<'_, MibResolverState>,
    oid: String,
) -> Result<Vec<mib::TreeNode>, String> {
    resolver
        .read_with(&engine_state.runtime_handle(), "mib_children", move |res| {
            res.get_children(&oid)
        })
        .await
}

/// Returns all MIB nodes under the given OID — the subtree in tree order.
#[tauri::command]
fn mib_subtree(
    resolver: tauri::State<MibResolverState>,
    oid: String,
) -> Result<Vec<mib::TreeNode>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.get_subtree(&oid))
}

/// Searches for MIB nodes matching the given query (autocomplete).
#[tauri::command]
async fn mib_search(
    engine_state: tauri::State<'_, SnmpEngineState>,
    resolver: tauri::State<'_, MibResolverState>,
    query: String,
) -> Result<Vec<mib::MibNode>, String> {
    resolver
        .read_with(&engine_state.runtime_handle(), "mib_search", move |res| {
            res.search(&query)
        })
        .await
}

/// Unloads the modules that the given file paths resolve to, in a single
/// write-lock (one index swap, one status) instead of N separate calls.
#[tauri::command]
fn mib_unload_many(
    resolver: tauri::State<MibResolverState>,
    file_paths: Vec<String>,
) -> Result<MibLoadStatus, String> {
    let mut res = resolver.inner.write().map_err(|e| e.to_string())?;
    res.unload_mibs_by_paths(&file_paths);
    Ok(mib_status_of(&res))
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

/// Returns parsed INDEX/AUGMENTS metadata for a TABLE node, if any.
#[tauri::command]
fn mib_table_info(
    resolver: tauri::State<MibResolverState>,
    table_oid: String,
) -> Result<Option<mib::TableInfo>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.get_table_info(&table_oid).cloned())
}

/// Returns full inspector details for an OID (longest-prefix resolved), or
/// `None` when the OID matches no loaded MIB node.
#[tauri::command]
fn mib_node_details(
    resolver: tauri::State<MibResolverState>,
    oid: String,
) -> Result<Option<mib::NodeDetails>, String> {
    let res = resolver.inner.read().map_err(|e| e.to_string())?;
    Ok(res.node_details(&oid))
}

/// Returns all OID → name pairs for frontend resolution.
#[tauri::command]
async fn mib_oid_name_map(
    engine_state: tauri::State<'_, SnmpEngineState>,
    resolver: tauri::State<'_, MibResolverState>,
) -> Result<Vec<(String, String)>, String> {
    resolver
        .read_with(&engine_state.runtime_handle(), "mib_oid_name_map", |res| {
            res.oid_name_map()
        })
        .await
}

// ── SNMP Commands ────────────────────────────────────────────────────────────

/// Tests connectivity to a Target (async — runs off the main thread).
#[tauri::command]
async fn snmp_connect(
    engine_state: tauri::State<'_, SnmpEngineState>,
    params: SnmpCommandParams,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params)?;
    let oids: Vec<String> = vec!["1.3.6.1.2.1.1.9.1.5.0".to_string()];

    // Run on the app-owned runtime (8MB worker stacks) to avoid tokio worker
    // stack overflow inside snmp2's connection code (which can recurse deeply).
    let engine = engine_state.engine.clone();
    let join_handle = engine_state
        .runtime_handle()
        .spawn(async move { engine.get(&target, &oids).await });
    let abort_on_timeout = join_handle.abort_handle();

    // Await completion with a timeout. On timeout the task is aborted —
    // dropping the JoinHandle alone left it running for the full retry
    // ladder (launch-beta/08).
    let joined = tokio::time::timeout(std::time::Duration::from_secs(10), join_handle).await;
    match joined {
        Ok(Ok(rs)) => rs,
        Ok(Err(join_err)) => Err(format!("Task panicked: {}", join_err)),
        Err(_timeout) => {
            abort_on_timeout.abort();
            Err("Connection timed out (10s)".to_string())
        }
    }
}

/// Executes a Get operation for the given OIDs.
///
/// Cancellable (launch-beta/08): Stop/Escape aborts the task and the engine
/// checks the shared token at every retry boundary.
#[tauri::command]
async fn snmp_get(
    engine_state: tauri::State<'_, SnmpEngineState>,
    resolver: tauri::State<'_, MibResolverState>,
    cancel_token: tauri::State<'_, WalkCancelToken>,
    params: SnmpCommandParams,
    oids: Vec<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params)?;
    // Scalar MIB nodes are queried at their `.0` instance; subtree, table and
    // row nodes (and OIDs that already carry an instance suffix) pass through.
    let oids: Vec<String> = oids
        .iter()
        .map(|o| scalar_instance_oid(&resolver.inner, o))
        .collect();
    let engine = engine_state.engine.clone();
    let cancel = (*cancel_token).inner();
    engine_state
        .run_cancellable("Get", &cancel_token, async move {
            engine
                .get_with_cancel(&target, &oids, Some(cancel.as_ref()))
                .await
        })
        .await?
}

/// Returns the OID to query for a Get: appends `.0` when the OID exactly
/// matches a scalar MIB node (e.g. `sysDescr` -> `1.3.6.1.2.1.1.1.0`).
fn scalar_instance_oid(resolver: &RwLock<mib::Resolver>, oid: &str) -> String {
    let Ok(guard) = resolver.read() else {
        return oid.to_string();
    };
    if let Some(node) = guard.resolve(oid) {
        if node.oid == oid
            && !matches!(
                node.syntax_type,
                mib::SyntaxType::ObjectIdentifier
                    | mib::SyntaxType::Table
                    | mib::SyntaxType::TableRow
            )
        {
            return format!("{oid}.0");
        }
    }
    oid.to_string()
}

/// Executes a GetNext operation for the given OIDs.
///
/// Cancellable (launch-beta/08): the engine checks the shared token between
/// the sequential OIDs and during retries, returning the bindings collected
/// so far with a `cancelled` warning.
#[tauri::command]
async fn snmp_get_next(
    engine_state: tauri::State<'_, SnmpEngineState>,
    cancel_token: tauri::State<'_, WalkCancelToken>,
    params: SnmpCommandParams,
    oids: Vec<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params)?;
    let engine = engine_state.engine.clone();
    let cancel = (*cancel_token).inner();
    engine_state
        .run_cancellable("GetNext", &cancel_token, async move {
            engine
                .get_next_with_cancel(&target, &oids, Some(cancel.as_ref()))
                .await
        })
        .await?
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
    let target = build_target(&params)?;
    let engine = engine_state.engine.clone();
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
    cancel_token.set_handle(handle.abort_handle());
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
    let target = build_target(&params)?;
    if matches!(target.version, snmp::Version::V1) {
        return Err("BulkWalk is not supported in SNMPv1 — use Walk instead".to_string());
    }
    let engine = engine_state.engine.clone();
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
    cancel_token.set_handle(handle.abort_handle());
    Ok(())
}

/// Cancels an in-progress walk.
#[tauri::command]
fn snmp_cancel_walk(cancel_token: tauri::State<'_, WalkCancelToken>) {
    cancel_token.cancel();
}

/// Executes a Set operation to write a value at the given OID.
///
/// Cancellable (launch-beta/08): Stop/Escape aborts the task and the engine
/// checks the shared token at every retry boundary — before the request is
/// ever sent on the first attempt.
#[tauri::command]
async fn snmp_set(
    engine_state: tauri::State<'_, SnmpEngineState>,
    resolver: tauri::State<'_, MibResolverState>,
    cancel_token: tauri::State<'_, WalkCancelToken>,
    params: SnmpCommandParams,
    oid: String,
    value_type: String,
    value: serde_json::Value,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params)?;
    let set_value = parse_set_value(&value_type, &value)?;
    // Scalar MIB nodes are written at their `.0` instance — the frontend applies
    // the same fixup, this is defense in depth (G7).
    let oid = scalar_instance_oid(&resolver.inner, &oid);
    let engine = engine_state.engine.clone();
    let cancel = (*cancel_token).inner();
    engine_state
        .run_cancellable("Set", &cancel_token, async move {
            engine
                .set_with_cancel(&target, &oid, set_value, Some(cancel.as_ref()))
                .await
        })
        .await?
}

/// Retrieves a table as a pivoted grid (streaming via channels).
///
/// One BulkWalk of the whole table subtree; `column_oids` is the display
/// selection only. Progress and the final grid arrive on the channels, so
/// Stop/Esc work like for Walk.
#[tauri::command]
async fn snmp_get_table(
    engine_state: tauri::State<'_, SnmpEngineState>,
    resolver: tauri::State<'_, MibResolverState>,
    cancel_token: tauri::State<'_, WalkCancelToken>,
    params: SnmpCommandParams,
    table_oid: String,
    column_oids: Vec<String>,
    progress_channel: tauri::ipc::Channel,
    complete_channel: tauri::ipc::Channel,
) -> Result<(), String> {
    cancel_token.reset();
    let target = build_target(&params)?;
    // Command-boundary guard (launch-beta/06): a Get Table on an OID that is
    // not a known TABLE node walks the entire subtree — a high subtree like
    // 1.3.6 collects the whole device MIB. The UI blocks this path; log it
    // loudly for every other caller, and let the binding cap bound the
    // damage.
    let known_table = resolver
        .inner
        .read()
        .map(|g| g.get_table_info(&table_oid).is_some())
        .unwrap_or(false);
    if !known_table {
        warn!("Get Table on '{table_oid}': not a known TABLE node — walking the full subtree");
    }
    let index_columns = mib_index_specs(&resolver.inner, &table_oid);
    let engine = engine_state.engine.clone();
    let cancel = (*cancel_token).inner();
    let sender = Arc::new(ChannelTableSender {
        progress: progress_channel,
        complete: complete_channel,
    });
    let handle = engine.get_table_streaming(
        &engine_state.runtime_handle(),
        &target,
        &table_oid,
        column_oids,
        index_columns,
        sender,
        Some(cancel),
    );
    cancel_token.set_handle(handle.abort_handle());
    Ok(())
}

// ── File System Commands ─────────────────────────────────────────────────────

/// Fallback suggested filename when nothing usable survives sanitization.
const DEFAULT_EXPORT_FILENAME: &str = "scout-export";

/// Reduces a caller-supplied export name to a bare, safe filename for the
/// native save dialog (launch-beta/09).
///
/// Export names are built from MIB node names — attacker-controlled text from
/// user-loaded MIB files — so every path component, control character and
/// leading dot is stripped: the dialog must never be prefilled with a target
/// outside the directory the user picks.
fn sanitize_filename(name: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>();
    let trimmed = base.trim().trim_start_matches('.').trim().to_string();
    if trimmed.is_empty() {
        DEFAULT_EXPORT_FILENAME.to_string()
    } else {
        trimmed.chars().take(100).collect()
    }
}

/// Saves `content` to a path the **user** chooses: the native save dialog
/// picks the destination and the write is scoped to exactly that path
/// (launch-beta/09).
///
/// This replaces the old `fs_write_file(path, content)` command, which wrote
/// to any path the webview asked for — turning any frontend XSS into a local
/// file write as the user (e.g. `~/.bashrc`). The webview no longer names a
/// path at all; it passes content and a suggested filename, and the dialog is
/// the only source of the destination. Returns the saved path, or `None` when
/// the user cancels.
#[tauri::command]
fn dialog_save_file(
    app: tauri::AppHandle,
    default_filename: String,
    content: String,
) -> Result<Option<String>, String> {
    let window = app
        .get_webview_window("main")
        .expect("main window not found");
    let path = rfd::FileDialog::new()
        .set_parent(&window)
        .set_file_name(sanitize_filename(&default_filename))
        .save_file();
    let Some(path) = path else { return Ok(None) };
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    Ok(Some(path.to_string_lossy().to_string()))
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

/// Opens a native multi-file picker dialog.
#[tauri::command]
fn dialog_open_files(app: tauri::AppHandle) -> Result<Option<Vec<String>>, String> {
    let window = app
        .get_webview_window("main")
        .expect("main window not found");
    let paths = rfd::FileDialog::new().set_parent(&window).pick_files();
    Ok(paths.map(|ps| {
        ps.into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    }))
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
    /// Per-request timeout in seconds; `None` uses the engine default
    /// (launch-beta/28).
    #[serde(default)]
    timeout_secs: Option<u64>,
}

/// Builds a Target from command parameters. Validates v3 security settings up
/// front so misconfigurations (e.g. an auth protocol with an empty passphrase)
/// fail immediately with a clear message instead of deep inside the SNMP stack.
fn build_target(params: &SnmpCommandParams) -> Result<snmp::Target, String> {
    let community = params
        .community
        .clone()
        .unwrap_or_else(|| "public".to_string());

    let apply_timeout = |t: snmp::Target| match params.timeout_secs {
        Some(secs) => t.with_timeout_secs(secs),
        None => t,
    };

    match params.version.to_lowercase().as_str() {
        "v1" => Ok(apply_timeout(snmp::Target::v1(
            &params.host,
            params.port,
            community,
        ))),
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
            security.resolve()?;
            Ok(apply_timeout(snmp::Target::v3(
                &params.host,
                params.port,
                security,
            )))
        }
        _ => Ok(apply_timeout(snmp::Target::v2c(
            &params.host,
            params.port,
            community,
        ))),
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
            // A string carries its UTF-8 bytes; a number array carries raw
            // bytes (hex-mode input and BITS encodings).
            if let Some(s) = value.as_str() {
                return Ok(snmp::SetValue::OctetString(s.as_bytes().to_vec()));
            }
            let bytes = value
                .as_array()
                .ok_or_else(|| "String or byte-array value expected".to_string())?
                .iter()
                .map(|b| {
                    b.as_u64()
                        .filter(|v| *v <= u8::MAX as u64)
                        .map(|v| v as u8)
                        .ok_or_else(|| "Byte values must be 0..=255".to_string())
                })
                .collect::<Result<Vec<u8>, String>>()?;
            Ok(snmp::SetValue::OctetString(bytes))
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

/// Status response for MIB loading operations. The `files_*` stats are only
/// present after a directory load pass (not after unload/status queries).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MibLoadStatus {
    /// Total number of indexed MIB nodes.
    node_count: usize,
    /// Names of MIB modules loaded via regex fallback.
    fallback_mibs: Vec<String>,
    /// Files (re)parsed by the load pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    files_parsed: Option<usize>,
    /// Files served from the parse cache by the load pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    files_cached: Option<usize>,
}

/// Builds a status response without load-pass stats.
fn mib_status_of(res: &mib::Resolver) -> MibLoadStatus {
    MibLoadStatus {
        node_count: res.node_count(),
        fallback_mibs: res.fallback_mib_names().cloned().collect(),
        files_parsed: None,
        files_cached: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_banner_names_version_and_platform() {
        let line = startup_banner();
        assert!(
            line.contains(env!("CARGO_PKG_VERSION")),
            "banner must carry the build version: {line}"
        );
        assert!(
            line.contains(std::env::consts::OS),
            "banner must carry the platform: {line}"
        );
    }

    #[test]
    fn bundled_mibs_dir_falls_back_to_checkout() {
        let dir = find_bundled_mibs_dir(None).expect("checkout mibs dir should exist");
        assert!(dir.join("IF-MIB.txt").is_file());
    }

    #[test]
    fn bundled_mibs_dir_prefers_resource_dir() {
        let tmp = std::env::temp_dir().join(format!("scout-mibs-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("mibs")).expect("create temp mibs dir");
        std::fs::write(
            tmp.join("mibs/TEST-MIB"),
            "TEST-MIB DEFINITIONS ::= BEGIN\nEND\n",
        )
        .expect("write test MIB");
        let dir = find_bundled_mibs_dir(Some(&tmp)).expect("resource mibs dir should win");
        assert_eq!(dir, tmp.join("mibs"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn bundled_mibs_dir_none_when_missing() {
        let tmp = std::env::temp_dir().join(format!("scout-mibs-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");
        // The checkout fallback always exists in-repo, so only assert the
        // resource-dir candidate is not picked when it lacks a mibs/ subdir.
        let dir = find_bundled_mibs_dir(Some(&tmp));
        assert!(dir.as_deref() != Some(tmp.join("mibs").as_path()));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn dir_has_entries_detects_content() {
        let tmp = std::env::temp_dir().join(format!("scout-mibs-entries-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");
        assert!(!dir_has_entries(&tmp.display().to_string()));
        std::fs::write(tmp.join("x.txt"), "x").expect("write file");
        assert!(dir_has_entries(&tmp.display().to_string()));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── Save-dialog filename sanitization (launch-beta/09) ────────────────
    //
    // Export filenames are built from MIB node names, which come from
    // user-loaded (attacker-controlled) MIB files. The name is only a dialog
    // *suggestion*, so it must never carry a path: a node named
    // "../../../../.bashrc" must not prefill the save dialog with a target
    // outside the user's chosen directory.

    #[test]
    fn sanitize_filename_strips_path_components() {
        assert_eq!(sanitize_filename("../../../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename(r"..\..\windows\x.bat"), "x.bat");
        assert_eq!(
            sanitize_filename("/absolute/path/sysDescr.json"),
            "sysDescr.json"
        );
    }

    #[test]
    fn sanitize_filename_strips_hidden_and_empty_names() {
        assert_eq!(sanitize_filename(".bashrc"), "bashrc");
        assert_eq!(sanitize_filename("...//.."), "scout-export");
        assert_eq!(sanitize_filename(""), "scout-export");
        assert_eq!(sanitize_filename("   "), "scout-export");
    }

    #[test]
    fn sanitize_filename_removes_control_chars() {
        assert_eq!(sanitize_filename("a\0b.txt"), "ab.txt");
        assert_eq!(sanitize_filename("line1\nline2.csv"), "line1line2.csv");
    }

    #[test]
    fn sanitize_filename_caps_length_and_keeps_normal_names() {
        assert_eq!(sanitize_filename("sysDescr.tsv"), "sysDescr.tsv");
        assert_eq!(sanitize_filename("ōsçråñ-mïb.tsv"), "ōsçråñ-mïb.tsv");
        let long = "a".repeat(400);
        let out = sanitize_filename(&long);
        assert!(
            out.chars().count() <= 100,
            "too long: {}",
            out.chars().count()
        );
        assert!(out.starts_with('a'));
    }
}
