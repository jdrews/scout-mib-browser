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

    /// SNMP version string, e.g. `"v2c"` or `"v3"`.
    #[serde(
        default = "default_snmp_version",
        skip_serializing_if = "is_default_version"
    )]
    pub version: String,

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

    /// v3 authentication protocol (`"none"`, `"md5"`, `"sha1"`, etc.).
    #[serde(
        default = "default_v3_auth_protocol",
        skip_serializing_if = "is_default_v3_auth"
    )]
    pub v3_auth_protocol: String,

    /// v3 authentication passphrase.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub v3_auth_passphrase: String,

    /// v3 privacy protocol (`"none"`, `"des"`, `"aes128"`, etc.).
    #[serde(
        default = "default_v3_priv_protocol",
        skip_serializing_if = "is_default_v3_priv"
    )]
    pub v3_priv_protocol: String,

    /// v3 privacy passphrase.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub v3_priv_passphrase: String,

    /// v3 security level (`"noAuthNoPriv"`, `"authNoPriv"`, `"authPriv"`).
    #[serde(
        default = "default_v3_security_level",
        skip_serializing_if = "is_default_v3_sec_level"
    )]
    pub v3_security_level: String,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: default_snmp_port(),
            version: default_snmp_version(),
            community: default_community_string(),
            v3_username: String::new(),
            v3_auth_protocol: default_v3_auth_protocol(),
            v3_auth_passphrase: String::new(),
            v3_priv_protocol: default_v3_priv_protocol(),
            v3_priv_passphrase: String::new(),
            v3_security_level: default_v3_security_level(),
        }
    }
}

impl TargetConfig {
    /// Returns `true` if all fields hold their default values.
    pub(crate) fn is_default(&self) -> bool {
        self.host.is_empty()
            && self.port == DEFAULT_SNMP_PORT
            && self.version == default_snmp_version()
            && self.community == default_community_string()
            && self.v3_username.is_empty()
            && self.v3_auth_protocol == default_v3_auth_protocol()
            && self.v3_auth_passphrase.is_empty()
            && self.v3_priv_protocol == default_v3_priv_protocol()
            && self.v3_priv_passphrase.is_empty()
            && self.v3_security_level == default_v3_security_level()
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
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            mib_tree_visible: default_true(),
            results_pane_visible: default_true(),
            splitter_horizontal: default_splitter_horizontal(),
            splitter_vertical: default_splitter_vertical(),
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
    }
}

// ── Default value helpers ────────────────────────────────────────────────────

fn default_snmp_port() -> u16 {
    DEFAULT_SNMP_PORT
}

fn default_mib_directories() -> Vec<String> {
    vec![String::from("/usr/share/snmp/mibs")]
}

fn default_snmp_version() -> String {
    String::from("v2c")
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

/// Returns `true` when the version string equals `"v2c"`.
fn is_default_version(v: &str) -> bool {
    v == "v2c"
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

// ── SNMPv3 defaults ────────────────────────────────────────────────────────

fn default_v3_auth_protocol() -> String {
    String::from("none")
}

fn default_v3_priv_protocol() -> String {
    String::from("none")
}

fn default_v3_security_level() -> String {
    String::from("noAuthNoPriv")
}

fn is_default_v3_auth(v: &str) -> bool {
    v == "none"
}

fn is_default_v3_priv(v: &str) -> bool {
    v == "none"
}

fn is_default_v3_sec_level(v: &str) -> bool {
    v == "noAuthNoPriv"
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
pub fn save_config(cfg: &AppConfig) -> Result<(), std::io::Error> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml_str = toml::to_string_pretty(cfg).expect("serialize config");
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
                        cfg.target.version = s.to_string();
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
                        cfg.target.v3_auth_protocol = s.to_string();
                    }
                }
                "v3_auth_passphrase" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.v3_auth_passphrase = s.to_string();
                    }
                }
                "v3_priv_protocol" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.v3_priv_protocol = s.to_string();
                    }
                }
                "v3_priv_passphrase" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.v3_priv_passphrase = s.to_string();
                    }
                }
                "v3_security_level" => {
                    if let Some(s) = value.as_str() {
                        cfg.target.v3_security_level = s.to_string();
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
        assert_eq!(cfg.target.version, "v2c");
        assert_eq!(cfg.target.community, "public");
        assert!(cfg.ui.mib_tree_visible);
        assert!(cfg.ui.results_pane_visible);
        assert!((cfg.ui.splitter_horizontal - 0.3).abs() < f64::EPSILON);
        assert!((cfg.ui.splitter_vertical - 0.5).abs() < f64::EPSILON);
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
        assert_eq!(loaded.target.version, "v2c");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn env_var_override_cascade() {
        // Set env vars that override defaults.
        env::set_var("SCOUT_TARGET__HOST", "10.0.0.1");
        env::set_var("SCOUT_TARGET__PORT", "2161");
        env::set_var("SCOUT_UI__SPLITTER_HORIZONTAL", "0.75");

        let cfg = build_config().unwrap();

        assert_eq!(cfg.target.host, "10.0.0.1");
        assert_eq!(cfg.target.port, 2161);
        assert!((cfg.ui.splitter_horizontal - 0.75).abs() < f64::EPSILON);

        // Unset to avoid polluting other tests.
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
        assert_eq!(cfg.target.version, "v2c");

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
