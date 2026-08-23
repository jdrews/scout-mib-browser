use config::{Config, ConfigBuilder, Environment, File as ConfigFile};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

/// Default SNMP port for Target connections.
const DEFAULT_SNMP_PORT: u16 = 161;

/// Default config directory name under the user's config dir.
const CONFIG_DIR: &str = "scout";

/// Default config file name.
const CONFIG_FILE: &str = "config.toml";

/// Environment variable prefix for config overrides.
const ENV_PREFIX: &str = "SCOUT";

// ── Typed enums for SNMP configuration ───────────────────────────────────────

/// SNMP version for Target connections.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnmpVersion {
    V1,
    #[default]
    V2c,
    V3,
}

impl SnmpVersion {
    pub fn as_str(&self) -> &str {
        match self {
            SnmpVersion::V1 => "v1",
            SnmpVersion::V2c => "v2c",
            SnmpVersion::V3 => "v3",
        }
    }

    pub fn is_default(&self) -> bool {
        *self == SnmpVersion::default()
    }
}

/// Authentication protocol for SNMPv3 USM.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum V3AuthProtocol {
    #[default]
    None,
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

impl V3AuthProtocol {
    pub fn as_str(&self) -> &str {
        match self {
            V3AuthProtocol::None => "none",
            V3AuthProtocol::Md5 => "md5",
            V3AuthProtocol::Sha1 => "sha1",
            V3AuthProtocol::Sha224 => "sha224",
            V3AuthProtocol::Sha256 => "sha256",
            V3AuthProtocol::Sha384 => "sha384",
            V3AuthProtocol::Sha512 => "sha512",
        }
    }

    pub fn is_default(&self) -> bool {
        *self == V3AuthProtocol::default()
    }
}

/// Privacy (encryption) protocol for SNMPv3 USM.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum V3PrivProtocol {
    #[default]
    None,
    Des,
    Aes128,
    Aes192,
    Aes256,
}

impl V3PrivProtocol {
    pub fn as_str(&self) -> &str {
        match self {
            V3PrivProtocol::None => "none",
            V3PrivProtocol::Des => "des",
            V3PrivProtocol::Aes128 => "aes128",
            V3PrivProtocol::Aes192 => "aes192",
            V3PrivProtocol::Aes256 => "aes256",
        }
    }

    pub fn is_default(&self) -> bool {
        *self == V3PrivProtocol::default()
    }
}

/// SNMPv3 security level.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum V3SecurityLevel {
    #[default]
    NoAuthNoPrivacy,
    AuthNoPrivacy,
    AuthPrivacy,
}

impl V3SecurityLevel {
    pub fn as_str(&self) -> &str {
        match self {
            V3SecurityLevel::NoAuthNoPrivacy => "noAuthNoPriv",
            V3SecurityLevel::AuthNoPrivacy => "authNoPriv",
            V3SecurityLevel::AuthPrivacy => "authPriv",
        }
    }

    pub fn is_default(&self) -> bool {
        *self == V3SecurityLevel::default()
    }
}

// ── Config Schema ────────────────────────────────────────────────────────────

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// MIB-related settings.
    #[serde(default, skip_serializing_if = "MibConfig::is_default")]
    pub mib: MibConfig,

    /// Last-used Target connection settings.
    #[serde(default, skip_serializing_if = "TargetConfig::is_default")]
    pub target: TargetConfig,

    /// UI state persistence.
    #[serde(default, skip_serializing_if = "UiConfig::is_default")]
    pub ui: UiConfig,
}

impl AppConfig {
    /// Converts to a [`toml::Value`] that includes **all** fields (including defaults).
    /// This bypasses `skip_serializing_if` which would drop default values and corrupt
    /// the config on round-trip when only a subset of fields is written.
    pub(crate) fn to_toml_value(&self) -> toml::Value {
        let mut table = toml::map::Map::new();

        // MIB section — always include directories.
        let mut mib_table = toml::map::Map::new();
        mib_table.insert(
            "directories".to_string(),
            toml::Value::Array(
                self.mib
                    .directories
                    .iter()
                    .map(|d| toml::Value::String(d.clone()))
                    .collect(),
            ),
        );
        table.insert("mib".to_string(), toml::Value::Table(mib_table));

        // Target section — always include all fields. Credential values are
        // omitted entirely when `save_credentials` is off, so no secrets can
        // reach the file through any write path.
        let mut target_table = toml::map::Map::new();
        if !self.target.host.is_empty() {
            target_table.insert(
                "host".to_string(),
                toml::Value::String(self.target.host.clone()),
            );
        }
        target_table.insert(
            "port".to_string(),
            toml::Value::Integer(self.target.port as i64),
        );
        target_table.insert(
            "version".to_string(),
            toml::Value::String(self.target.version.as_str().to_string()),
        );
        if self.ui.save_credentials {
            target_table.insert(
                "community".to_string(),
                toml::Value::String(self.target.community.clone()),
            );
            if !self.target.v3_username.is_empty() {
                target_table.insert(
                    "v3_username".to_string(),
                    toml::Value::String(self.target.v3_username.clone()),
                );
            }
            if !self.target.v3_auth_passphrase.is_empty() {
                target_table.insert(
                    "v3_auth_passphrase".to_string(),
                    toml::Value::String(self.target.v3_auth_passphrase.clone()),
                );
            }
            if !self.target.v3_priv_passphrase.is_empty() {
                target_table.insert(
                    "v3_priv_passphrase".to_string(),
                    toml::Value::String(self.target.v3_priv_passphrase.clone()),
                );
            }
        }
        if !self.target.v3_auth_protocol.is_default() {
            target_table.insert(
                "v3_auth_protocol".to_string(),
                toml::Value::String(self.target.v3_auth_protocol.as_str().to_string()),
            );
        }
        if !self.target.v3_priv_protocol.is_default() {
            target_table.insert(
                "v3_priv_protocol".to_string(),
                toml::Value::String(self.target.v3_priv_protocol.as_str().to_string()),
            );
        }
        if !self.target.v3_security_level.is_default() {
            target_table.insert(
                "v3_security_level".to_string(),
                toml::Value::String(self.target.v3_security_level.as_str().to_string()),
            );
        }
        table.insert("target".to_string(), toml::Value::Table(target_table));

        // UI section — always include all fields.
        let mut ui_table = toml::map::Map::new();
        ui_table.insert(
            "mib_tree_visible".to_string(),
            toml::Value::Boolean(self.ui.mib_tree_visible),
        );
        ui_table.insert(
            "results_pane_visible".to_string(),
            toml::Value::Boolean(self.ui.results_pane_visible),
        );
        ui_table.insert(
            "splitter_horizontal".to_string(),
            toml::Value::Float(self.ui.splitter_horizontal),
        );
        ui_table.insert(
            "splitter_vertical".to_string(),
            toml::Value::Float(self.ui.splitter_vertical),
        );
        ui_table.insert(
            "save_credentials".to_string(),
            toml::Value::Boolean(self.ui.save_credentials),
        );
        table.insert("ui".to_string(), toml::Value::Table(ui_table));

        toml::Value::Table(table)
    }
}

/// Configuration for MIB file discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MibConfig {
    /// Directories to search for MIB files.
    #[serde(default = "default_mib_directories")]
    pub directories: Vec<String>,
}

impl Default for MibConfig {
    fn default() -> Self {
        Self {
            directories: default_mib_directories(),
        }
    }
}

impl MibConfig {
    /// Returns `true` if all fields hold their default values.
    pub(crate) fn is_default(&self) -> bool {
        self.directories == default_mib_directories()
    }
}

/// Last-used Target connection settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    /// Hostname or IP address of the last-queried Target.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,

    /// UDP port for SNMP on the Target (default 161).
    #[serde(default = "default_snmp_port", skip_serializing_if = "is_default_port")]
    pub port: u16,

    /// SNMP version.
    #[serde(default, skip_serializing_if = "SnmpVersion::is_default")]
    pub version: SnmpVersion,

    /// Community string for SNMPv1/v2c authentication.
    #[serde(
        default = "default_community_string",
        skip_serializing_if = "is_default_community"
    )]
    pub community: String,

    // ── SNMPv3 USM/VACM settings ────────────────────────────────────────
    /// v3 USM username.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub v3_username: String,

    /// v3 authentication protocol.
    #[serde(default, skip_serializing_if = "V3AuthProtocol::is_default")]
    pub v3_auth_protocol: V3AuthProtocol,

    /// v3 authentication passphrase.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub v3_auth_passphrase: String,

    /// v3 privacy protocol.
    #[serde(default, skip_serializing_if = "V3PrivProtocol::is_default")]
    pub v3_priv_protocol: V3PrivProtocol,

    /// v3 privacy passphrase.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub v3_priv_passphrase: String,

    /// v3 security level.
    #[serde(default, skip_serializing_if = "V3SecurityLevel::is_default")]
    pub v3_security_level: V3SecurityLevel,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: default_snmp_port(),
            version: SnmpVersion::default(),
            community: default_community_string(),
            v3_username: String::new(),
            v3_auth_protocol: V3AuthProtocol::default(),
            v3_auth_passphrase: String::new(),
            v3_priv_protocol: V3PrivProtocol::default(),
            v3_priv_passphrase: String::new(),
            v3_security_level: V3SecurityLevel::default(),
        }
    }
}

impl TargetConfig {
    /// Returns `true` if all fields hold their default values.
    pub(crate) fn is_default(&self) -> bool {
        self.host.is_empty()
            && self.port == DEFAULT_SNMP_PORT
            && self.version.is_default()
            && self.community == default_community_string()
            && self.v3_username.is_empty()
            && self.v3_auth_protocol.is_default()
            && self.v3_auth_passphrase.is_empty()
            && self.v3_priv_protocol.is_default()
            && self.v3_priv_passphrase.is_empty()
            && self.v3_security_level.is_default()
    }
}

/// UI layout and visibility state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Whether the MIB tree pane is visible.
    #[serde(default = "default_true", skip_serializing_if = "is_default_bool_true")]
    pub mib_tree_visible: bool,

    /// Whether the results pane is visible.
    #[serde(default = "default_true", skip_serializing_if = "is_default_bool_true")]
    pub results_pane_visible: bool,

    /// Horizontal splitter position as fraction (0.0–1.0).
    #[serde(
        default = "default_splitter_horizontal",
        skip_serializing_if = "is_default_splitter_h"
    )]
    pub splitter_horizontal: f64,

    /// Vertical splitter position as fraction (0.0–1.0).
    #[serde(
        default = "default_splitter_vertical",
        skip_serializing_if = "is_default_splitter_v"
    )]
    pub splitter_vertical: f64,

    /// Whether Target credentials (community string and V3 username/
    /// passphrases) are persisted to the config file. Host, port, version,
    /// and protocol choices are always saved.
    #[serde(default = "default_true", skip_serializing_if = "is_default_bool_true")]
    pub save_credentials: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            mib_tree_visible: default_true(),
            results_pane_visible: default_true(),
            splitter_horizontal: default_splitter_horizontal(),
            splitter_vertical: default_splitter_vertical(),
            save_credentials: default_true(),
        }
    }
}

impl UiConfig {
    /// Returns `true` if all fields hold their default values.
    pub(crate) fn is_default(&self) -> bool {
        self.mib_tree_visible
            && self.results_pane_visible
            && (self.splitter_horizontal - 0.3).abs() < f64::EPSILON
            && (self.splitter_vertical - 0.5).abs() < f64::EPSILON
            && self.save_credentials
    }
}

/// Removes credential values from a config in place: community string and all
/// V3 credential fields (username, auth passphrase, priv passphrase) reset to
/// their defaults. Host, port, version, and protocol choices are kept. Called
/// when the `save_credentials` toggle is turned off so no secrets remain on disk.
pub fn scrub_credentials(cfg: &mut AppConfig) {
    cfg.target.community = default_community_string();
    cfg.target.v3_username.clear();
    cfg.target.v3_auth_passphrase.clear();
    cfg.target.v3_priv_passphrase.clear();
}

// ── Default value helpers ────────────────────────────────────────────────────

fn default_snmp_port() -> u16 {
    DEFAULT_SNMP_PORT
}

fn default_mib_directories() -> Vec<String> {
    vec![String::from("/usr/share/snmp/mibs")]
}

fn default_community_string() -> String {
    String::from("public")
}

fn default_true() -> bool {
    true
}

fn default_splitter_horizontal() -> f64 {
    0.3
}

fn default_splitter_vertical() -> f64 {
    0.5
}

/// Returns `true` when the port equals the default SNMP port.
fn is_default_port(v: &u16) -> bool {
    *v == DEFAULT_SNMP_PORT
}

/// Returns `true` when the community string equals `"public"`.
fn is_default_community(v: &str) -> bool {
    v == "public"
}

/// Returns `true` when the boolean is `true` (the default).
fn is_default_bool_true(v: &bool) -> bool {
    *v
}

/// Returns `true` when the horizontal splitter is at its default position.
fn is_default_splitter_h(v: &f64) -> bool {
    (v - 0.3).abs() < f64::EPSILON
}

/// Returns `true` when the vertical splitter is at its default position.
fn is_default_splitter_v(v: &f64) -> bool {
    (v - 0.5).abs() < f64::EPSILON
}

// ── Config path resolution ───────────────────────────────────────────────────

/// Resolves the full path to `~/.config/scout/config.toml`.
pub fn config_path() -> PathBuf {
    let config_dir = dirs::config_local_dir()
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR);
    config_dir.join(CONFIG_FILE)
}

// ── Builder helpers ──────────────────────────────────────────────────────────

/// Applies the standard set of builder defaults to a config builder.
fn with_defaults(
    builder: ConfigBuilder<config::builder::DefaultState>,
) -> ConfigBuilder<config::builder::DefaultState> {
    builder
        .set_default("target.port", DEFAULT_SNMP_PORT)
        .unwrap()
        .set_default("target.version", "v2c")
        .unwrap()
        .set_default("target.community", "public")
        .unwrap()
        .set_default("ui.mib_tree_visible", true)
        .unwrap()
        .set_default("ui.results_pane_visible", true)
        .unwrap()
        .set_default("ui.splitter_horizontal", 0.3_f64)
        .unwrap()
        .set_default("ui.splitter_vertical", 0.5_f64)
        .unwrap()
        .set_default("ui.save_credentials", true)
        .unwrap()
}

// ── Builder ──────────────────────────────────────────────────────────────────

/// Builds an `AppConfig` from defaults, environment overrides, and the TOML file.
///
/// Cascade order (highest priority last):
/// 1. Struct defaults (`Default` trait / serde default functions)
/// 2. Environment variables prefixed with `SCOUT_`
/// 3. Values in `~/.config/scout/config.toml` (if it exists)
pub fn build_config() -> Result<AppConfig, config::ConfigError> {
    let path = config_path();

    let mut builder = with_defaults(Config::builder()).add_source(
        Environment::with_prefix(ENV_PREFIX)
            .prefix_separator("_")
            .separator("__"),
    );

    // Only add the file source if it exists — this creates the file on first run.
    if path.exists() {
        builder = builder.add_source(ConfigFile::from(path).required(false));
    }

    let config = builder.build()?;
    config.try_deserialize()
}

/// Creates the config directory and writes an empty TOML file if it does not exist.
///
/// The file is intentionally empty — all defaults are applied by [`build_config`]
/// at read time, so only non-default values need to be persisted.
pub fn ensure_config_file() -> Result<PathBuf, std::io::Error> {
    let path = config_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, "")?;
    }
    Ok(path)
}

/// Persists the current `AppConfig` back to the TOML file.
///
/// Uses a dedicated serializable wrapper that forces all fields to be written,
/// bypassing `skip_serializing_if` which would drop defaults and corrupt the
/// config on round-trip (e.g., writing only `[mib]` loses `[target]`).
pub fn save_config(cfg: &AppConfig) -> Result<(), std::io::Error> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml_str = toml::to_string_pretty(&cfg.to_toml_value()).expect("serialize config");
    std::fs::write(&path, toml_str)
}

// ── Tauri State wrapper ──────────────────────────────────────────────────────

/// Thread-safe handle stored in Tauri app state.
#[derive(Clone)]
pub struct ConfigHandle {
    /// Path to the config file on disk.
    pub path: PathBuf,
}

impl ConfigHandle {
    /// Reads the current configuration from disk (with defaults and env cascade).
    pub fn read(&self) -> Result<AppConfig, String> {
        build_config().map_err(|e| e.to_string())
    }

    /// Writes a new configuration to disk.
    pub fn write(&self, cfg: AppConfig) -> Result<(), String> {
        save_config(&cfg).map_err(|e| e.to_string())
    }
}

// ── Tauri Commands ───────────────────────────────────────────────────────────

/// Returns the full application configuration.
#[tauri::command]
pub fn config_read(handle: State<ConfigHandle>) -> Result<AppConfig, String> {
    handle.read()
}

/// Updates a specific field in the configuration and persists it.
///
/// `path` uses dot-separated keys (e.g., `"target.host"`).
/// `value` is a JSON-encoded value matching the expected type.
#[tauri::command]
pub fn config_write(
    handle: State<ConfigHandle>,
    path: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let mut cfg = handle.read()?;

    // Parse dot-separated path and update the matching field.
    if let Some(dot_pos) = path.find('.') {
        let section = &path[..dot_pos];
        let key = &path[dot_pos + 1..];

        match section {
            "mib" => match key {
                "directories" => {
                    if let Some(arr) = value.as_array() {
                        cfg.mib.directories = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                    }
                }
                _ => return Err(format!("unknown mib key: {}", key)),
            },
            "target" => match key {
                "host" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.host = s.to_string();
                    }
                }
                "port" => {
                    if let Some(n) = value.as_u64() {
                        cfg.target.port = n as u16;
                    }
                }
                "version" => {
                    if let Some(s) = value.as_str() {
                        match s.to_lowercase().as_str() {
                            "v1" => cfg.target.version = SnmpVersion::V1,
                            "v3" => cfg.target.version = SnmpVersion::V3,
                            _ => cfg.target.version = SnmpVersion::V2c,
                        }
                    }
                }
                "community" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.community = s.to_string();
                    }
                }
                "v3_username" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.v3_username = s.to_string();
                    }
                }
                "v3_auth_protocol" => {
                    if let Some(s) = value.as_str() {
                        match s.to_lowercase().as_str() {
                            "md5" => cfg.target.v3_auth_protocol = V3AuthProtocol::Md5,
                            "sha1" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha1,
                            "sha224" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha224,
                            "sha256" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha256,
                            "sha384" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha384,
                            "sha512" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha512,
                            _ => cfg.target.v3_auth_protocol = V3AuthProtocol::None,
                        }
                    }
                }
                "v3_auth_passphrase" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.v3_auth_passphrase = s.to_string();
                    }
                }
                "v3_priv_protocol" => {
                    if let Some(s) = value.as_str() {
                        match s.to_lowercase().as_str() {
                            "des" => cfg.target.v3_priv_protocol = V3PrivProtocol::Des,
                            "aes128" => cfg.target.v3_priv_protocol = V3PrivProtocol::Aes128,
                            "aes192" => cfg.target.v3_priv_protocol = V3PrivProtocol::Aes192,
                            "aes256" => cfg.target.v3_priv_protocol = V3PrivProtocol::Aes256,
                            _ => cfg.target.v3_priv_protocol = V3PrivProtocol::None,
                        }
                    }
                }
                "v3_priv_passphrase" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.v3_priv_passphrase = s.to_string();
                    }
                }
                "v3_security_level" => {
                    if let Some(s) = value.as_str() {
                        match s.to_lowercase().as_str() {
                            "authnopriv" => {
                                cfg.target.v3_security_level = V3SecurityLevel::AuthNoPrivacy
                            }
                            "authpriv" => {
                                cfg.target.v3_security_level = V3SecurityLevel::AuthPrivacy
                            }
                            _ => cfg.target.v3_security_level = V3SecurityLevel::NoAuthNoPrivacy,
                        }
                    }
                }
                _ => return Err(format!("unknown target key: {}", key)),
            },
            "ui" => match key {
                "mib_tree_visible" => {
                    if let Some(b) = value.as_bool() {
                        cfg.ui.mib_tree_visible = b;
                    }
                }
                "results_pane_visible" => {
                    if let Some(b) = value.as_bool() {
                        cfg.ui.results_pane_visible = b;
                    }
                }
                "splitter_horizontal" => {
                    if let Some(n) = value.as_f64() {
                        cfg.ui.splitter_horizontal = n;
                    }
                }
                "splitter_vertical" => {
                    if let Some(n) = value.as_f64() {
                        cfg.ui.splitter_vertical = n;
                    }
                }
                "save_credentials" => {
                    if let Some(b) = value.as_bool() {
                        cfg.ui.save_credentials = b;
                        // Turning the toggle off scrubs already-saved credential
                        // values from disk immediately, not just future writes.
                        if !b {
                            scrub_credentials(&mut cfg);
                        }
                    }
                }
                _ => return Err(format!("unknown ui key: {}", key)),
            },
            _ => return Err(format!("unknown section: {}", section)),
        }
    } else {
        return Err(format!(
            "path must be dot-separated (e.g., \"target.host\"): {}",
            path
        ));
    }

    handle.write(cfg)
}

/// Returns the config file path.
#[tauri::command]
pub fn config_get_path(handle: State<ConfigHandle>) -> String {
    handle.path.display().to_string()
}

/// Writes all Target connection settings at once and persists to disk.
///
/// Accepts a JSON object with any subset of `TargetConfig` fields. Only the
/// provided fields are updated; missing fields retain their current values.
#[tauri::command]
pub fn config_write_target(
    handle: State<ConfigHandle>,
    config: serde_json::Value,
) -> Result<(), String> {
    let mut cfg = handle.read()?;

    if let Some(obj) = config.as_object() {
        if let Some(v) = obj.get("host").and_then(|v| v.as_str()) {
            cfg.target.host = v.to_string();
        }
        if let Some(v) = obj.get("port").and_then(|v| v.as_u64()) {
            cfg.target.port = v as u16;
        }
        if let Some(v) = obj.get("version").and_then(|v| v.as_str()) {
            match v.to_lowercase().as_str() {
                "v1" => cfg.target.version = SnmpVersion::V1,
                "v3" => cfg.target.version = SnmpVersion::V3,
                _ => cfg.target.version = SnmpVersion::V2c,
            }
        }
        // Credential fields are only applied when saving is enabled; with the
        // opt-out on, typed credentials stay in memory for the session but
        // never reach the config file.
        if cfg.ui.save_credentials {
            if let Some(v) = obj.get("community").and_then(|v| v.as_str()) {
                cfg.target.community = v.to_string();
            }
            if let Some(v) = obj.get("v3_username").and_then(|v| v.as_str()) {
                cfg.target.v3_username = v.to_string();
            }
        }
        if let Some(v) = obj.get("v3_auth_protocol").and_then(|v| v.as_str()) {
            match v.to_lowercase().as_str() {
                "md5" => cfg.target.v3_auth_protocol = V3AuthProtocol::Md5,
                "sha1" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha1,
                "sha224" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha224,
                "sha256" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha256,
                "sha384" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha384,
                "sha512" => cfg.target.v3_auth_protocol = V3AuthProtocol::Sha512,
                _ => cfg.target.v3_auth_protocol = V3AuthProtocol::None,
            }
        }
        if cfg.ui.save_credentials {
            if let Some(v) = obj.get("v3_auth_passphrase").and_then(|v| v.as_str()) {
                cfg.target.v3_auth_passphrase = v.to_string();
            }
        }
        if let Some(v) = obj.get("v3_priv_protocol").and_then(|v| v.as_str()) {
            match v.to_lowercase().as_str() {
                "des" => cfg.target.v3_priv_protocol = V3PrivProtocol::Des,
                "aes128" => cfg.target.v3_priv_protocol = V3PrivProtocol::Aes128,
                "aes192" => cfg.target.v3_priv_protocol = V3PrivProtocol::Aes192,
                "aes256" => cfg.target.v3_priv_protocol = V3PrivProtocol::Aes256,
                _ => cfg.target.v3_priv_protocol = V3PrivProtocol::None,
            }
        }
        if cfg.ui.save_credentials {
            if let Some(v) = obj.get("v3_priv_passphrase").and_then(|v| v.as_str()) {
                cfg.target.v3_priv_passphrase = v.to_string();
            }
        }
    }

    handle.write(cfg)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Helper to build config from a temporary directory.
    fn test_config_path(dir: &std::path::Path) -> PathBuf {
        dir.join(CONFIG_FILE)
    }

    #[test]
    fn defaults_are_correct() {
        let cfg = AppConfig::default();

        assert_eq!(
            cfg.mib.directories,
            vec!["/usr/share/snmp/mibs".to_string()]
        );
        assert_eq!(cfg.target.host, "");
        assert_eq!(cfg.target.port, 161);
        assert_eq!(cfg.target.version, SnmpVersion::V2c);
        assert_eq!(cfg.target.community, "public");
        assert!(cfg.ui.mib_tree_visible);
        assert!(cfg.ui.results_pane_visible);
        assert!((cfg.ui.splitter_horizontal - 0.3).abs() < f64::EPSILON);
        assert!((cfg.ui.splitter_vertical - 0.5).abs() < f64::EPSILON);
        assert!(cfg.ui.save_credentials);
    }

    #[test]
    fn save_credentials_off_keeps_credential_fields_out_of_toml() {
        let mut cfg = AppConfig::default();
        cfg.target.host = "10.0.0.5".to_string();
        cfg.target.port = 1161;
        cfg.target.version = SnmpVersion::V3;
        cfg.target.community = "s3cret-community".to_string();
        cfg.target.v3_username = "admin".to_string();
        cfg.target.v3_auth_protocol = V3AuthProtocol::Sha256;
        cfg.target.v3_auth_passphrase = "auth-pass".to_string();
        cfg.target.v3_priv_protocol = V3PrivProtocol::Aes128;
        cfg.target.v3_priv_passphrase = "priv-pass".to_string();
        cfg.ui.save_credentials = false;

        let value = cfg.to_toml_value();
        let toml_str = toml::to_string_pretty(&value).unwrap();

        // No credential values reach the file.
        assert!(!toml_str.contains("s3cret-community"));
        assert!(!toml_str.contains("admin"));
        assert!(!toml_str.contains("auth-pass"));
        assert!(!toml_str.contains("priv-pass"));
        let target = value.get("target").unwrap().as_table().unwrap();
        assert!(target.get("community").is_none());
        assert!(target.get("v3_username").is_none());
        assert!(target.get("v3_auth_passphrase").is_none());
        assert!(target.get("v3_priv_passphrase").is_none());

        // Host, port, version, and (non-secret) protocol choices still persist.
        assert_eq!(target.get("host").unwrap().as_str(), Some("10.0.0.5"));
        assert_eq!(target.get("port").unwrap().as_integer(), Some(1161));
        assert_eq!(target.get("version").unwrap().as_str(), Some("v3"));
        assert_eq!(
            target.get("v3_auth_protocol").unwrap().as_str(),
            Some("sha256")
        );
        assert_eq!(
            target.get("v3_priv_protocol").unwrap().as_str(),
            Some("aes128")
        );

        // The toggle's own state is persisted so the opt-out survives restarts.
        let ui = value.get("ui").unwrap().as_table().unwrap();
        assert_eq!(ui.get("save_credentials").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn save_credentials_on_round_trips_credentials() {
        let mut cfg = AppConfig::default();
        cfg.target.host = "10.0.0.5".to_string();
        cfg.target.community = "s3cret-community".to_string();
        cfg.target.v3_username = "admin".to_string();
        cfg.target.v3_auth_passphrase = "auth-pass".to_string();

        let value = cfg.to_toml_value();
        let toml_str = toml::to_string_pretty(&value).unwrap();
        assert!(toml_str.contains("s3cret-community"));
        assert!(toml_str.contains("admin"));
        assert!(toml_str.contains("auth-pass"));

        // Read back with defaults; credentials survive the round trip.
        let loaded: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.target.community, "s3cret-community");
        assert_eq!(loaded.target.v3_username, "admin");
        assert_eq!(loaded.target.v3_auth_passphrase, "auth-pass");
        assert!(loaded.ui.save_credentials);
    }

    #[test]
    fn scrub_on_disable_removes_saved_credentials_from_disk() {
        // Simulate a config file that already holds credentials.
        let mut cfg = AppConfig::default();
        cfg.target.host = "10.0.0.5".to_string();
        cfg.target.community = "s3cret-community".to_string();
        cfg.target.v3_username = "admin".to_string();
        cfg.target.v3_auth_passphrase = "auth-pass".to_string();
        cfg.target.v3_priv_passphrase = "priv-pass".to_string();

        // Turning the toggle off scrubs immediately (what config_write does).
        cfg.ui.save_credentials = false;
        scrub_credentials(&mut cfg);
        let toml_str = toml::to_string_pretty(&cfg.to_toml_value()).unwrap();

        assert!(!toml_str.contains("s3cret-community"));
        assert!(!toml_str.contains("admin"));
        assert!(!toml_str.contains("auth-pass"));
        assert!(!toml_str.contains("priv-pass"));

        // Non-credential settings survive the scrub.
        assert!(toml_str.contains("10.0.0.5"));
    }

    #[test]
    fn round_trip_persistence() {
        let tmp = std::env::temp_dir().join("scout_config_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = test_config_path(&tmp);

        let mut cfg = AppConfig::default();
        cfg.target.host = "192.168.1.1".to_string();
        cfg.target.port = 1161;
        cfg.ui.splitter_horizontal = 0.45;

        // Write using toml directly (simulating save_config).
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        std::fs::write(&path, &toml_str).unwrap();

        // Read back via config crate with defaults for missing fields.
        let loaded: AppConfig = with_defaults(Config::builder())
            .add_source(ConfigFile::from(path.clone()).required(false))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(loaded.target.host, "192.168.1.1");
        assert_eq!(loaded.target.port, 1161);
        assert!((loaded.ui.splitter_horizontal - 0.45).abs() < f64::EPSILON);
        // Defaults preserved for unchanged fields.
        assert_eq!(loaded.target.version, SnmpVersion::V2c);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn env_var_override_cascade() {
        // Hermetic (Linux): point the config dir at an empty temp location so a
        // real ~/.config/scout/config.toml cannot shadow the env vars.
        let tmp = std::env::temp_dir().join("scout_env_override_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        env::set_var("XDG_CONFIG_HOME", &tmp);

        // Set env vars that override defaults.
        env::set_var("SCOUT_TARGET__HOST", "10.0.0.1");
        env::set_var("SCOUT_TARGET__PORT", "2161");
        env::set_var("SCOUT_UI__SPLITTER_HORIZONTAL", "0.75");

        let cfg = build_config().unwrap();

        assert_eq!(cfg.target.host, "10.0.0.1");
        assert_eq!(cfg.target.port, 2161);
        assert!((cfg.ui.splitter_horizontal - 0.75).abs() < f64::EPSILON);

        // Unset to avoid polluting other tests.
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("SCOUT_TARGET__HOST");
        env::remove_var("SCOUT_TARGET__PORT");
        env::remove_var("SCOUT_UI__SPLITTER_HORIZONTAL");
    }

    #[test]
    fn ensure_config_file_creates_directory_and_file() {
        let tmp = std::env::temp_dir().join("scout_ensure_test");
        let _ = std::fs::remove_dir_all(&tmp);

        // Verify the directory creation and file write logic.
        let path = tmp.join(CONFIG_DIR).join(CONFIG_FILE);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "").unwrap();

        assert!(path.exists());

        // Empty file + builder defaults = full config.
        let cfg: AppConfig = with_defaults(Config::builder())
            .add_source(ConfigFile::from(path).required(false))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(cfg.target.port, 161);
        assert_eq!(cfg.target.version, SnmpVersion::V2c);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn config_write_updates_field() {
        let tmp = std::env::temp_dir().join("scout_write_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = test_config_path(&tmp);

        // Start with empty file (defaults applied by builder).
        std::fs::write(&path, "").unwrap();

        // Read with defaults.
        let mut updated: AppConfig = with_defaults(Config::builder())
            .add_source(ConfigFile::from(path.clone()).required(false))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        updated.target.host = "new-host.example.com".to_string();
        std::fs::write(&path, toml::to_string_pretty(&updated).unwrap()).unwrap();

        // Read back.
        let reloaded: AppConfig = with_defaults(Config::builder())
            .add_source(ConfigFile::from(path.clone()).required(false))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(reloaded.target.host, "new-host.example.com");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
