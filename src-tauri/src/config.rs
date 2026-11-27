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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

        // MIB section — always include directories and explicit files.
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
        mib_table.insert(
            "files".to_string(),
            toml::Value::Array(
                self.mib
                    .files
                    .iter()
                    .map(|f| toml::Value::String(f.clone()))
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
        if let Some(secs) = self.target.timeout_secs {
            target_table.insert(
                "timeout_secs".to_string(),
                toml::Value::Integer(secs as i64),
            );
        }
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MibConfig {
    /// Directories to search for MIB files (live sources).
    #[serde(default = "default_mib_directories")]
    pub directories: Vec<String>,

    /// Explicit MIB files loaded alongside the directories (persistent
    /// sources; a deleted file drops out on the next load).
    #[serde(default)]
    pub files: Vec<String>,
}

impl Default for MibConfig {
    fn default() -> Self {
        Self {
            directories: default_mib_directories(),
            files: Vec::new(),
        }
    }
}

impl MibConfig {
    /// Returns `true` if all fields hold their default values.
    pub(crate) fn is_default(&self) -> bool {
        self.directories == default_mib_directories() && self.files.is_empty()
    }
}

/// Last-used Target connection settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// Per-request round-trip timeout in seconds. `None` applies the engine
    /// default (2s, launch-beta/28).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,

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
            timeout_secs: None,
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
            && self.timeout_secs.is_none()
            && self.v3_username.is_empty()
            && self.v3_auth_protocol.is_default()
            && self.v3_auth_passphrase.is_empty()
            && self.v3_priv_protocol.is_default()
            && self.v3_priv_passphrase.is_empty()
            && self.v3_security_level.is_default()
    }
}

/// UI layout and visibility state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// and protocol choices are always saved. Off by default: plaintext
    /// credentials on disk are opt-in (launch-beta/04).
    #[serde(
        default = "default_false",
        skip_serializing_if = "is_default_bool_false"
    )]
    pub save_credentials: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            mib_tree_visible: default_true(),
            results_pane_visible: default_true(),
            splitter_horizontal: default_splitter_horizontal(),
            splitter_vertical: default_splitter_vertical(),
            save_credentials: default_false(),
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
            && !self.save_credentials
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

fn default_false() -> bool {
    false
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

/// Returns `true` when the boolean is `false` (the default).
fn is_default_bool_false(v: &bool) -> bool {
    !*v
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
        .set_default("ui.save_credentials", false)
        .unwrap()
}

// ── Builder ──────────────────────────────────────────────────────────────────

/// Builds an `AppConfig` from defaults, environment overrides, and the TOML
/// file at an explicit path.
///
/// Cascade order (highest priority last):
/// 1. Struct defaults (`Default` trait / serde default functions)
/// 2. Environment variables prefixed with `SCOUT_`
/// 3. Values in the TOML file at `path` (if it exists)
fn build_config_at(path: &std::path::Path) -> Result<AppConfig, config::ConfigError> {
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

/// Loads the config, quarantining a corrupt file (launch-beta/05).
///
/// When the file at `path` cannot be parsed or deserialized — power-loss
/// truncation, hand-edit damage — it is renamed to
/// `config.toml.bak-<timestamp>` so the bad bytes survive for manual
/// recovery, and the app starts from defaults (plus env overrides). A
/// warning is logged, which lands in the System Log via the UI log layer.
///
/// Returns the config and, when a quarantine happened, the backup path.
/// Defaults are returned even if the quarantine rename fails, so read (and
/// therefore every write path built on it) is never permanently poisoned.
pub fn load_config_quarantining(path: &std::path::Path) -> (AppConfig, Option<PathBuf>) {
    match build_config_at(path) {
        Ok(cfg) => (cfg, None),
        Err(e) => {
            let backup = quarantine_file(path);
            match &backup {
                Some(b) => {
                    tracing::warn!(
                        "Config file {} is corrupt ({}) — moved it to {} and starting from \
                         defaults. Re-apply your settings; the old file is preserved for \
                         manual recovery.",
                        path.display(),
                        e,
                        b.display()
                    );
                    // The file source is gone now; rebuild so defaults and
                    // env overrides still apply.
                    let cfg = build_config_at(path).unwrap_or_default();
                    (cfg, backup)
                }
                None => {
                    tracing::warn!(
                        "Config file {} is corrupt ({}) — starting from defaults, but the \
                         corrupt file could not be moved aside for backup.",
                        path.display(),
                        e
                    );
                    (AppConfig::default(), None)
                }
            }
        }
    }
}

/// Renames `path` to a timestamped sibling (`config.toml.bak-<stamp>`),
/// preserving the corrupt bytes. Returns the backup path, or `None` when
/// the rename failed.
fn quarantine_file(path: &std::path::Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let stamp = chrono::Local::now().format("%Y%m%dT%H%M%S");
    // Second-resolution stamps can collide (two corrupt loads within one
    // second); suffix with a sequence until the name is free.
    for seq in 0..1000u32 {
        let name = if seq == 0 {
            format!("{file_name}.bak-{stamp}")
        } else {
            format!("{file_name}.bak-{stamp}-{seq}")
        };
        let candidate = path.with_file_name(name);
        if candidate.exists() {
            continue;
        }
        return match std::fs::rename(path, &candidate) {
            Ok(()) => Some(candidate),
            Err(_) => None,
        };
    }
    None
}

/// Creates the config directory and writes an empty TOML file if it does not exist.
///
/// The file is intentionally empty — all defaults are applied by
/// [`build_config_at`] at read time, so only non-default values need to be
/// persisted.
pub fn ensure_config_file() -> Result<PathBuf, std::io::Error> {
    ensure_config_file_at(&config_path())
}

/// [`ensure_config_file`] at an explicit path (test seam). The file is
/// created owner-only (0600, dir 0700 on unix) — the config can hold v3
/// passphrases, so it must never be world-readable (launch-beta/04).
pub fn ensure_config_file_at(path: &std::path::Path) -> Result<PathBuf, std::io::Error> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            create_config_dir(parent)?;
        }
        write_private(path, "")?;
    }
    Ok(path.to_path_buf())
}

/// Persists the current `AppConfig` back to the TOML file at `path`.
///
/// Atomic and owner-only: the TOML is written to a sibling temp file with
/// 0600 permissions, then renamed over the target. A crash mid-write can
/// only leave a stray temp file — never a truncated config (the ticket-05
/// brick) — and the rename guarantees the target is 0600 even when a prior
/// version left it world-readable.
///
/// Uses a dedicated serializable wrapper that forces all fields to be
/// written, bypassing `skip_serializing_if` which would drop defaults and
/// corrupt the config on round-trip (e.g., writing only `[mib]` loses
/// `[target]`).
pub fn save_config_to(path: &std::path::Path, cfg: &AppConfig) -> Result<(), std::io::Error> {
    let toml_str = toml::to_string_pretty(&cfg.to_toml_value()).expect("serialize config");
    if let Some(parent) = path.parent() {
        create_config_dir(parent)?;
    }
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| CONFIG_FILE.to_string());
    let tmp = path.with_file_name(format!("{}.tmp.{}", file_name, std::process::id()));
    write_private(&tmp, &toml_str)?;
    std::fs::rename(&tmp, path)
}

/// Creates the config directory with owner-only permissions on unix
/// (0700); existing directories are left as-is.
#[cfg(unix)]
fn create_config_dir(dir: &std::path::Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

/// Writes `contents` to `path` with owner-only permissions (0600),
/// creating or truncating the file. Permission is set explicitly after
/// write so pre-existing files are tightened, not just new ones.
#[cfg(unix)]
fn write_private(path: &std::path::Path, contents: &str) -> Result<(), std::io::Error> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())?;
    // Durability before rename: the data must be on disk when the atomic
    // swap makes the file visible.
    file.sync_all()?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Non-unix: plain write. Windows inherits the per-user `%APPDATA%` ACL
/// (owner-only by default); see SECURITY.md.
#[cfg(not(unix))]
fn create_config_dir(dir: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dir)
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, contents: &str) -> Result<(), std::io::Error> {
    std::fs::write(path, contents)
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
    ///
    /// Never fails on a corrupt file: the bad config is quarantined to a
    /// timestamped backup and defaults are returned (launch-beta/05), so
    /// every settings write built on this read keeps working.
    pub fn read(&self) -> Result<AppConfig, String> {
        let (cfg, _) = load_config_quarantining(&self.path);
        Ok(cfg)
    }

    /// Writes a new configuration to disk.
    pub fn write(&self, cfg: AppConfig) -> Result<(), String> {
        save_config_to(&self.path, &cfg).map_err(|e| e.to_string())
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
                "files" => {
                    if let Some(arr) = value.as_array() {
                        cfg.mib.files = arr
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
                "timeout_secs" => {
                    cfg.target.timeout_secs = value.as_u64();
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
        if let Some(v) = obj.get("timeout_secs").and_then(|v| v.as_u64()) {
            cfg.target.timeout_secs = Some(v);
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

    /// Serializes this module's tests. The config crate reads `SCOUT_*`
    /// env overrides process-wide, and `env_var_override_cascade` sets
    /// them; Rust runs tests as threads in one process, so a parallel
    /// config build can absorb a leaked override (CI race: SCOUT_TARGET__HOST
    /// leaked into the corrupt-config quarantine test, making "defaults"
    /// non-default). Poison is tolerated: a panicking test must not take
    /// the whole module down with it.
    static CONFIG_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn config_test_guard() -> std::sync::MutexGuard<'static, ()> {
        CONFIG_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Helper to build config from a temporary directory.
    fn test_config_path(dir: &std::path::Path) -> PathBuf {
        dir.join(CONFIG_FILE)
    }

    #[test]
    fn defaults_are_correct() {
        let _serial = config_test_guard();
        let cfg = AppConfig::default();

        assert_eq!(
            cfg.mib.directories,
            vec!["/usr/share/snmp/mibs".to_string()]
        );
        assert!(cfg.mib.files.is_empty());
        assert_eq!(cfg.target.host, "");
        assert_eq!(cfg.target.port, 161);
        assert_eq!(cfg.target.version, SnmpVersion::V2c);
        assert_eq!(cfg.target.community, "public");
        assert!(cfg.ui.mib_tree_visible);
        assert!(cfg.ui.results_pane_visible);
        assert!((cfg.ui.splitter_horizontal - 0.3).abs() < f64::EPSILON);
        assert!((cfg.ui.splitter_vertical - 0.5).abs() < f64::EPSILON);
        // Fresh installs do NOT persist credentials (launch-beta/04): a
        // multi-user jump box must not gain a plaintext passphrase file
        // unless the user opts in.
        assert!(!cfg.ui.save_credentials);
    }

    #[test]
    fn builder_defaults_do_not_save_credentials() {
        let _serial = config_test_guard();
        // The read cascade applies the same off-by-default (empty file =
        // fresh install).
        let tmp = std::env::temp_dir().join("scout_fresh_default_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);
        std::fs::write(&path, "").unwrap();

        let cfg: AppConfig = with_defaults(Config::builder())
            .add_source(ConfigFile::from(path).required(false))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();
        assert!(!cfg.ui.save_credentials);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn save_credentials_off_keeps_credential_fields_out_of_toml() {
        let _serial = config_test_guard();
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
        let _serial = config_test_guard();
        let mut cfg = AppConfig::default();
        cfg.target.host = "10.0.0.5".to_string();
        cfg.target.community = "s3cret-community".to_string();
        cfg.target.v3_username = "admin".to_string();
        cfg.target.v3_auth_passphrase = "auth-pass".to_string();
        // Credentials persist only for users who opted in.
        cfg.ui.save_credentials = true;

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
    fn mib_files_round_trip() {
        let _serial = config_test_guard();
        let mut cfg = AppConfig::default();
        cfg.mib.files = vec!["/home/user/mibs/EXTRA-MIB".to_string()];

        let value = cfg.to_toml_value();
        let toml_str = toml::to_string_pretty(&value).unwrap();
        assert!(toml_str.contains("EXTRA-MIB"));

        // Read back with defaults; explicit files survive the round trip.
        let loaded: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            loaded.mib.files,
            vec!["/home/user/mibs/EXTRA-MIB".to_string()]
        );
        assert_eq!(
            loaded.mib.directories,
            vec!["/usr/share/snmp/mibs".to_string()]
        );
    }

    #[test]
    fn mib_files_absent_in_toml_deserializes_to_empty() {
        let _serial = config_test_guard();
        // A pre-existing config file without a `files` key loads cleanly.
        let toml_str = toml::to_string_pretty(&AppConfig::default()).unwrap();
        let loaded: AppConfig = toml::from_str(&toml_str).unwrap();
        assert!(loaded.mib.files.is_empty());
    }

    #[test]
    fn scrub_on_disable_removes_saved_credentials_from_disk() {
        let _serial = config_test_guard();
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
        let _serial = config_test_guard();
        let tmp = std::env::temp_dir().join("scout_config_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = test_config_path(&tmp);

        let mut cfg = AppConfig::default();
        cfg.target.host = "192.168.1.1".to_string();
        cfg.target.port = 1161;
        cfg.ui.splitter_horizontal = 0.45;

        // Write using toml directly (simulating save_config_to).
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
        let _serial = config_test_guard();
        // Hermetic (Linux): point the file source at an empty temp location
        // so a real ~/.config/scout/config.toml cannot shadow the env vars.
        // set_var IS process-wide, so this test holds the module lock for
        // its whole set→build→remove window (see CONFIG_TEST_LOCK).
        let tmp = std::env::temp_dir().join("scout_env_override_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Set env vars that override defaults.
        env::set_var("SCOUT_TARGET__HOST", "10.0.0.1");
        env::set_var("SCOUT_TARGET__PORT", "2161");
        env::set_var("SCOUT_UI__SPLITTER_HORIZONTAL", "0.75");

        let cfg = build_config_at(&test_config_path(&tmp)).unwrap();

        assert_eq!(cfg.target.host, "10.0.0.1");
        assert_eq!(cfg.target.port, 2161);
        assert!((cfg.ui.splitter_horizontal - 0.75).abs() < f64::EPSILON);

        // Unset to avoid polluting other tests.
        env::remove_var("SCOUT_TARGET__HOST");
        env::remove_var("SCOUT_TARGET__PORT");
        env::remove_var("SCOUT_UI__SPLITTER_HORIZONTAL");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ensure_config_file_creates_directory_and_file() {
        let _serial = config_test_guard();
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
        let _serial = config_test_guard();
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

    // ── Credential hardening: permissions & atomic writes (launch-beta/04) ──

    #[cfg(unix)]
    fn mode_of(path: &std::path::Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn save_config_writes_owner_only_file() {
        let _serial = config_test_guard();
        // The config can hold v3 passphrases; on a multi-user jump box a
        // 0644 file makes them world-readable.
        let tmp = std::env::temp_dir().join("scout_perm_file_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);

        save_config_to(&path, &AppConfig::default()).unwrap();

        assert_eq!(mode_of(&path), 0o600, "config must be owner read/write");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(unix)]
    #[test]
    fn save_config_creates_owner_only_directory() {
        let _serial = config_test_guard();
        let tmp = std::env::temp_dir().join("scout_perm_dir_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let path = tmp.join(CONFIG_DIR).join(CONFIG_FILE);

        save_config_to(&path, &AppConfig::default()).unwrap();

        assert_eq!(
            mode_of(&tmp.join(CONFIG_DIR)),
            0o700,
            "config dir must be owner-only"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(unix)]
    #[test]
    fn save_config_tightens_preexisting_world_readable_file() {
        let _serial = config_test_guard();
        // Upgrades from older versions leave a 0644 config; the first save
        // after upgrade must tighten it.
        let tmp = std::env::temp_dir().join("scout_perm_tighten_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);

        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&path, "[target]\nhost = \"old.example.com\"\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(mode_of(&path), 0o644);

        save_config_to(&path, &AppConfig::default()).unwrap();

        assert_eq!(mode_of(&path), 0o600);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn save_config_leaves_no_temp_file_on_success() {
        let _serial = config_test_guard();
        // Atomic save: write a sibling temp file, then rename. A completed
        // save must not litter the config directory.
        let tmp = std::env::temp_dir().join("scout_atomic_success_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);

        save_config_to(&path, &AppConfig::default()).unwrap();

        let leftovers: Vec<_> = std::fs::read_dir(&tmp)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(unix)]
    #[test]
    fn failed_save_leaves_existing_config_untouched() {
        let _serial = config_test_guard();
        // A crash/full-disk mid-write must never leave a truncated config
        // (the ticket-05 brick). With the temp-file + rename scheme the
        // write failure happens off the real path, so the previous file
        // survives byte-identical.
        let tmp = std::env::temp_dir().join("scout_atomic_fail_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);

        save_config_to(&path, &AppConfig::default()).unwrap();
        let before = std::fs::read(&path).unwrap();

        // Simulate a mid-write failure: make the directory read-only so the
        // temp file cannot be created.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o500)).unwrap();
        let result = save_config_to(&path, &AppConfig::default());
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o700)).unwrap();

        assert!(result.is_err(), "saving into a read-only dir must fail");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "existing config must be untouched"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_config_file_creates_owner_only_file() {
        let _serial = config_test_guard();
        let tmp = std::env::temp_dir().join("scout_ensure_perm_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let path = tmp.join(CONFIG_DIR).join(CONFIG_FILE);

        ensure_config_file_at(&path).unwrap();

        assert!(path.exists());
        assert_eq!(mode_of(&path), 0o600);
        assert_eq!(mode_of(&tmp.join(CONFIG_DIR)), 0o700);

        // Idempotent: an existing file is left alone.
        ensure_config_file_at(&path).unwrap();
        assert_eq!(mode_of(&path), 0o600);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── Corrupt-config recovery (launch-beta/05) ───────────────────────────

    #[test]
    fn valid_config_is_not_quarantined() {
        let _serial = config_test_guard();
        let tmp = std::env::temp_dir().join("scout_recover_valid_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);

        let mut cfg = AppConfig::default();
        cfg.target.host = "agent.example.com".to_string();
        save_config_to(&path, &cfg).unwrap();

        let (loaded, quarantined) = load_config_quarantining(&path);
        assert!(
            quarantined.is_none(),
            "valid config must not be quarantined"
        );
        assert_eq!(loaded.target.host, "agent.example.com");
        assert!(path.exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn corrupt_config_is_quarantined_and_defaults_returned() {
        let _serial = config_test_guard();
        // A truncated/corrupt config (power loss, hand-edit) must not brick
        // the app: the bad file is preserved as a timestamped backup and the
        // app starts from defaults.
        let tmp = std::env::temp_dir().join("scout_recover_corrupt_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);

        let corrupt = "[target]\nhost = \"10.0.0.5\"\nport = 1161\nbroken = [[\n";
        std::fs::write(&path, corrupt).unwrap();

        let (loaded, quarantined) = load_config_quarantining(&path);

        // Startup proceeds from defaults.
        assert_eq!(loaded, AppConfig::default());

        // The corrupt bytes are preserved in a timestamped backup.
        let backup = quarantined.expect("corrupt config must be quarantined");
        assert!(backup.exists(), "backup must exist at {}", backup.display());
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            corrupt,
            "backup must preserve the corrupt bytes verbatim"
        );
        let backup_name = backup.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            backup_name.starts_with("config.toml.bak-"),
            "backup name must be config.toml.bak-<timestamp>, got {backup_name}"
        );

        // The bad file is gone from the real path so writes can proceed.
        assert!(!path.exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn type_error_config_is_quarantined_too() {
        let _serial = config_test_guard();
        // Valid TOML with the wrong types (e.g. port = "not-a-number")
        // deserializes to a ConfigError and bricks writes just like a parse
        // error — same recovery.
        let tmp = std::env::temp_dir().join("scout_recover_type_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);

        std::fs::write(&path, "[target]\nport = \"not-a-number\"\n").unwrap();

        let (loaded, quarantined) = load_config_quarantining(&path);
        assert_eq!(loaded, AppConfig::default());
        assert!(quarantined.is_some());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn settings_writes_recover_after_corrupt_config() {
        let _serial = config_test_guard();
        // The ticket's core scenario: corrupt TOML, app (re)starts, every
        // settings write works again — the read path is never permanently
        // poisoned.
        let tmp = std::env::temp_dir().join("scout_recover_write_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = test_config_path(&tmp);

        std::fs::write(&path, "this is not = valid toml [[[").unwrap();

        let handle = ConfigHandle { path: path.clone() };

        // Read recovers: defaults, no error.
        let cfg = handle
            .read()
            .expect("read must recover from corrupt config");
        assert_eq!(cfg, AppConfig::default());

        // Write works and round-trips.
        let mut cfg = cfg;
        cfg.target.host = "recovered.example.com".to_string();
        handle.write(cfg).expect("write must work after recovery");

        let reloaded = handle.read().unwrap();
        assert_eq!(reloaded.target.host, "recovered.example.com");

        // The corrupt bytes are still preserved next to the new config.
        let backups: Vec<_> = std::fs::read_dir(&tmp)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("config.toml.bak-")
            })
            .collect();
        assert_eq!(backups.len(), 1, "exactly one timestamped backup");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
