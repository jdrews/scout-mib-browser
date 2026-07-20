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
const WALK_BATCH_EVENT: &str = snmp::WALK_BATCH_EVENT;

fn main() {
    let snmp_state = SnmpEngineState::new().expect("failed to create SNMP engine");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
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
    params: SnmpCommandParams,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.get(&target, &["1.3.6.1.2.1.1.9.1.5.0".to_string()])
}

/// Executes a Get operation for the given OIDs.
#[tauri::command]
fn snmp_get(
    engine: tauri::State<SnmpEngineState>,
    params: SnmpCommandParams,
    oids: Vec<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.get(&target, &oids)
}

/// Executes a GetNext operation for the given OIDs.
#[tauri::command]
fn snmp_get_next(
    engine: tauri::State<SnmpEngineState>,
    params: SnmpCommandParams,
    oids: Vec<String>,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.get_next(&target, &oids)
}

/// Executes a Walk operation from the given root OID.
/// Returns immediately — results stream via Tauri events.
#[tauri::command]
fn snmp_walk(
    app: tauri::AppHandle,
    engine: tauri::State<SnmpEngineState>,
    params: SnmpCommandParams,
    root_oid: String,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.walk_spawn(app, &target, &root_oid);
    Ok(snmp::ResultSet::new())
}

/// Executes a BulkWalk operation from the given root OID.
/// Returns immediately — results stream via Tauri events.
#[tauri::command]
fn snmp_bulk_walk(
    app: tauri::AppHandle,
    engine: tauri::State<SnmpEngineState>,
    params: SnmpCommandParams,
    root_oid: String,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.bulk_walk_spawn(app, &target, &root_oid);
    Ok(snmp::ResultSet::new())
}

/// Executes a Set operation to write a value at the given OID.
#[tauri::command]
fn snmp_set(
    engine: tauri::State<SnmpEngineState>,
    params: SnmpCommandParams,
    oid: String,
    value_type: String,
    value: serde_json::Value,
) -> Result<snmp::ResultSet, String> {
    let target = build_target(&params);
    let set_value = parse_set_value(&value_type, &value)?;
    let engine = engine.inner.read().map_err(|e| e.to_string())?;
    engine.set(&target, &oid, set_value)
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
struct MibLoadStatus {
    /// Total number of indexed MIB nodes.
    node_count: usize,
    /// Names of MIB modules loaded via regex fallback.
    fallback_mibs: Vec<String>,
}
