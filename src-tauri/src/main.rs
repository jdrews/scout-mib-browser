mod config;
mod mib;
mod snmp;

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

/// Thread-safe handle to the SNMP engine stored in Tauri app state.
#[derive(Clone)]
pub struct SnmpEngineState {
    inner: Arc<RwLock<snmp::SnmpEngine>>,
}

impl SnmpEngineState {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            inner: Arc::new(RwLock::new(snmp::SnmpEngine::new()?)),
        })
    }
}

/// Tauri event name for walk batch emission.
const WALK_BATCH_EVENT: &str = "snmp-walk-batch";

fn main() {
    let snmp_state = SnmpEngineState::new().expect("failed to create SNMP engine");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            config::ensure_config_file().expect("failed to create config file");

            let path = config::config_path();
            app.manage(config::ConfigHandle { path });

            app.manage(MibResolverState::new());
            app.manage(snmp_state.clone());

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
            snmp_connect,
            snmp_get,
            snmp_get_next,
            snmp_walk,
            snmp_bulk_walk,
            snmp_set,
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

// ── SNMP Commands ────────────────────────────────────────────────────────────

/// Tests connectivity to a Target.
#[tauri::command]
fn snmp_connect(
    engine: tauri::State<SnmpEngineState>,
    host: String,
    port: u16,
    version: String,
    community: Option<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&host, port, &version, community);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.get(&target, &["1.3.6.1.2.1.1.9.1.5.0".to_string()])
}

/// Executes a Get operation for the given OIDs.
#[tauri::command]
fn snmp_get(
    engine: tauri::State<SnmpEngineState>,
    host: String,
    port: u16,
    version: String,
    community: Option<String>,
    oids: Vec<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&host, port, &version, community);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.get(&target, &oids)
}

/// Executes a GetNext operation for the given OIDs.
#[tauri::command]
fn snmp_get_next(
    engine: tauri::State<SnmpEngineState>,
    host: String,
    port: u16,
    version: String,
    community: Option<String>,
    oids: Vec<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&host, port, &version, community);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.get_next(&target, &oids)
}

/// Executes a Walk operation from the given root OID.
#[tauri::command]
fn snmp_walk(
    _app: tauri::AppHandle,
    engine: tauri::State<SnmpEngineState>,
    host: String,
    port: u16,
    version: String,
    community: Option<String>,
    root_oid: String,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&host, port, &version, community);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;

    // Create callback that emits batches to the frontend.
    let app_handle = _app.clone();
    let callback: Arc<snmp::WalkBatchCallback> =
        Arc::new(move |bindings: Vec<snmp::VariableBinding>| {
            let _ = app_handle.emit_to(
                "main",
                WALK_BATCH_EVENT,
                serde_json::json!({ "bindings": bindings }),
            );
        });

    engine.walk_with_callback(&target, &root_oid, callback)
}

/// Executes a BulkWalk operation from the given root OID.
#[tauri::command]
fn snmp_bulk_walk(
    _app: tauri::AppHandle,
    engine: tauri::State<SnmpEngineState>,
    host: String,
    port: u16,
    version: String,
    community: Option<String>,
    root_oid: String,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&host, port, &version, community);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;

    let app_handle = _app.clone();
    let callback: Arc<snmp::WalkBatchCallback> =
        Arc::new(move |bindings: Vec<snmp::VariableBinding>| {
            let _ = app_handle.emit_to(
                "main",
                WALK_BATCH_EVENT,
                serde_json::json!({ "bindings": bindings }),
            );
        });

    engine.bulk_walk_with_callback(&target, &root_oid, callback)
}

/// Executes a Set operation to write a value at the given OID.
#[tauri::command]
fn snmp_set(
    engine: tauri::State<SnmpEngineState>,
    host: String,
    port: u16,
    version: String,
    community: Option<String>,
    oid: String,
    value_type: String,
    value: serde_json::Value,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&host, port, &version, community);
    let set_value = parse_set_value(&value_type, &value)?;
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.set(&target, &oid, set_value)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Builds a Target from command parameters.
fn build_target(host: &str, port: u16, version: &str, community: Option<String>) -> snmp::Target {
    let community = community.unwrap_or_else(|| "public".to_string());

    match version.to_lowercase().as_str() {
        "v1" => snmp::Target::v1(host, port, community),
        "v3" => snmp::Target::v3(
            host,
            port,
            snmp::SnmpV3SecurityConfig {
                username: String::new(),
                auth_protocol: snmp::AuthProtocol::None,
                auth_passphrase: String::new(),
                priv_protocol: snmp::PrivProtocol::None,
                priv_passphrase: String::new(),
                security_level: snmp::SecurityLevelTag::NoAuthNoPriv,
            },
        ),
        _ => snmp::Target::v2c(host, port, community),
    }
}

/// Parses a JSON value into a SetValue based on the type string.
fn parse_set_value(value_type: &str, value: &serde_json::Value) -> Result<snmp::SetValue, String> {
    match value_type.to_lowercase().as_str() {
        "integer" | "integer32" => {
            let v = value
                .as_i64()
                .ok_or_else(|| "Integer value expected".to_string())?;
            Ok(snmp::SetValue::Integer(v as i32))
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
            Ok(snmp::SetValue::Gauge32(v as u32))
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
struct MibLoadStatus {
    /// Total number of indexed MIB nodes.
    node_count: usize,
    /// Names of MIB modules loaded via regex fallback.
    fallback_mibs: Vec<String>,
}
