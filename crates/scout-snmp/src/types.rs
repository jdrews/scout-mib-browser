use serde::Serialize;

/// SNMP version supported by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Version {
    V1,
    V2c,
    V3,
}

impl From<snmp2::Version> for Version {
    fn from(v: snmp2::Version) -> Self {
        match v {
            snmp2::Version::V1 => Version::V1,
            snmp2::Version::V2C => Version::V2c,
            snmp2::Version::V3 => Version::V3,
        }
    }
}

impl From<Version> for snmp2::Version {
    fn from(v: Version) -> Self {
        match v {
            Version::V1 => snmp2::Version::V1,
            Version::V2c => snmp2::Version::V2C,
            Version::V3 => snmp2::Version::V3,
        }
    }
}

/// Authentication protocol for SNMPv3 USM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthProtocol {
    None,
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

/// Privacy (encryption) protocol for SNMPv3 USM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PrivProtocol {
    None,
    Des,
    Aes128,
    Aes192,
    Aes256,
}

/// SNMPv3 USM/VACM security configuration.
#[derive(Debug, Clone, Serialize)]
pub struct SnmpV3SecurityConfig {
    /// USM username for authentication.
    pub username: String,
    /// Authentication protocol.
    #[serde(skip_serializing_if = "is_auth_none")]
    pub auth_protocol: AuthProtocol,
    /// Authentication passphrase (empty if auth is None).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub auth_passphrase: String,
    /// Privacy protocol.
    #[serde(skip_serializing_if = "is_priv_none")]
    pub priv_protocol: PrivProtocol,
    /// Privacy passphrase (empty if privacy is None).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub priv_passphrase: String,
}

impl Default for SnmpV3SecurityConfig {
    fn default() -> Self {
        Self {
            username: String::new(),
            auth_protocol: AuthProtocol::None,
            auth_passphrase: String::new(),
            priv_protocol: PrivProtocol::None,
            priv_passphrase: String::new(),
        }
    }
}

fn is_auth_none(v: &AuthProtocol) -> bool {
    matches!(v, AuthProtocol::None)
}

fn is_priv_none(v: &PrivProtocol) -> bool {
    matches!(v, PrivProtocol::None)
}

/// Resolved SNMPv3 USM parameters for a session, derived from
/// [`SnmpV3SecurityConfig`]. Exposed so the protocol mapping can be unit-tested
/// and inspected independently of a live connection.
#[derive(Debug, Clone, PartialEq)]
pub struct V3SecurityPlan {
    /// Security level: noAuthNoPriv / authNoPriv / authPriv.
    pub auth: snmp2::v3::Auth,
    /// Authentication protocol (`None` for noAuthNoPriv).
    pub auth_protocol: Option<snmp2::v3::AuthProtocol>,
    /// Key-extension method for the AES-192/256 pairs whose auth hash is too
    /// short to yield the full privacy key (`None` when not required).
    pub key_extension: Option<snmp2::v3::KeyExtension>,
}

fn auth_protocol_to_snmp2(p: &AuthProtocol) -> Option<snmp2::v3::AuthProtocol> {
    match p {
        AuthProtocol::None => None,
        AuthProtocol::Md5 => Some(snmp2::v3::AuthProtocol::Md5),
        AuthProtocol::Sha1 => Some(snmp2::v3::AuthProtocol::Sha1),
        AuthProtocol::Sha224 => Some(snmp2::v3::AuthProtocol::Sha224),
        AuthProtocol::Sha256 => Some(snmp2::v3::AuthProtocol::Sha256),
        AuthProtocol::Sha384 => Some(snmp2::v3::AuthProtocol::Sha384),
        AuthProtocol::Sha512 => Some(snmp2::v3::AuthProtocol::Sha512),
    }
}

fn priv_protocol_to_cipher(p: &PrivProtocol) -> Option<snmp2::v3::Cipher> {
    match p {
        PrivProtocol::None => None,
        PrivProtocol::Des => Some(snmp2::v3::Cipher::Des),
        PrivProtocol::Aes128 => Some(snmp2::v3::Cipher::Aes128),
        PrivProtocol::Aes192 => Some(snmp2::v3::Cipher::Aes192),
        PrivProtocol::Aes256 => Some(snmp2::v3::Cipher::Aes256),
    }
}

impl SnmpV3SecurityConfig {
    /// Resolves this configuration into the concrete USM parameters a session
    /// needs. Pure (no I/O) so the full protocol matrix is unit-testable without
    /// a live agent. Returns an error for combinations that are invalid per
    /// RFC 3414 (privacy without authentication).
    pub fn resolve(&self) -> Result<V3SecurityPlan, String> {
        if self.priv_protocol != PrivProtocol::None && self.auth_protocol == AuthProtocol::None {
            return Err(
                "SNMPv3 privacy (encryption) requires an authentication protocol".to_string(),
            );
        }

        // snmp2's key derivation indexes into the passphrase unconditionally, so
        // an empty passphrase with a protocol selected would panic deep inside
        // the fork — reject it here with a clear error instead.
        if self.auth_protocol != AuthProtocol::None && self.auth_passphrase.is_empty() {
            return Err("SNMPv3 authentication requires a non-empty auth passphrase".to_string());
        }
        if self.priv_protocol != PrivProtocol::None && self.priv_passphrase.is_empty() {
            return Err(
                "SNMPv3 privacy (encryption) requires a non-empty priv passphrase".to_string(),
            );
        }

        let auth_protocol = auth_protocol_to_snmp2(&self.auth_protocol);

        let auth = match (&self.auth_protocol, &self.priv_protocol) {
            (AuthProtocol::None, PrivProtocol::None) => snmp2::v3::Auth::NoAuthNoPriv,
            (_, PrivProtocol::None) => snmp2::v3::Auth::AuthNoPriv,
            (_, priv_) => snmp2::v3::Auth::AuthPriv {
                cipher: priv_protocol_to_cipher(priv_)
                    .expect("privacy protocol is non-None in the AuthPriv branch"),
                privacy_password: self.priv_passphrase.clone().into_bytes(),
            },
        };

        let key_extension = match (&auth, &auth_protocol) {
            (snmp2::v3::Auth::AuthPriv { cipher, .. }, Some(ap)) => {
                if cipher.priv_key_needs_extension(ap) {
                    // Reeder matches the de-facto standard AES-192/256 OIDs used by
                    // pysnmp/snmpsim and most vendors (Cisco et al.).
                    Some(snmp2::v3::KeyExtension::Reeder)
                } else {
                    None
                }
            }
            _ => None,
        };

        Ok(V3SecurityPlan {
            auth,
            auth_protocol,
            key_extension,
        })
    }
}

/// The SNMP device being queried — its address and credentials combined.
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    /// Hostname or IP address.
    pub host: String,
    /// UDP port (default 161).
    pub port: u16,
    /// SNMP version.
    pub version: Version,
    /// Community string for v1/v2c authentication.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub community: String,
    /// v3 USM/VACM settings (only used when version is V3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<SnmpV3SecurityConfig>,
}

impl Target {
    /// Creates a new v2c Target with the given community string.
    pub fn v2c(host: impl Into<String>, port: u16, community: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port,
            version: Version::V2c,
            community: community.into(),
            security: None,
        }
    }

    /// Creates a new v1 Target.
    pub fn v1(host: impl Into<String>, port: u16, community: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port,
            version: Version::V1,
            community: community.into(),
            security: None,
        }
    }

    /// Creates a new v3 Target with USM settings.
    pub fn v3(host: impl Into<String>, port: u16, security: SnmpV3SecurityConfig) -> Self {
        Self {
            host: host.into(),
            port,
            version: Version::V3,
            community: String::new(),
            security: Some(security),
        }
    }

    /// Returns the socket address string for this Target.
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// A single SNMP data value that can be returned in a Variable Binding.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SnmpValue {
    Integer(i64),
    Unsigned(u32),
    Counter32(u32),
    Counter64(u64),
    OctetString(Vec<u8>),
    ObjectIdentifier(String),
    IpAddress(String),
    TimeTicks(u32),
    TruthValue(bool),
    Null,
    /// Raw value with ASN.1 type code for unexpected or unparseable types.
    Raw {
        type_code: u8,
        data: Vec<u8>,
    },
}

#[cfg(test)]
impl SnmpValue {
    pub fn display(&self) -> String {
        match self {
            SnmpValue::Integer(v) => format!("{}", v),
            SnmpValue::Unsigned(v) => format!("{}", v),
            SnmpValue::Counter32(v) => format!("{}", v),
            SnmpValue::Counter64(v) => format!("{}", v),
            SnmpValue::OctetString(bytes) => {
                if let Ok(s) = String::from_utf8(bytes.clone()) {
                    format!("\"{}\"", s)
                } else {
                    format!(
                        "0x{}",
                        bytes
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>()
                    )
                }
            }
            SnmpValue::ObjectIdentifier(oid) => oid.clone(),
            SnmpValue::IpAddress(ip) => ip.clone(),
            SnmpValue::TimeTicks(v) => format!("{}", v),
            SnmpValue::TruthValue(v) => {
                if *v {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            SnmpValue::Null => "NULL".to_string(),
            SnmpValue::Raw { data, .. } => data
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    pub fn type_label(&self) -> &'static str {
        match self {
            SnmpValue::Integer(_) => "INTEGER",
            SnmpValue::Unsigned(_) => "UNSIGNED32",
            SnmpValue::Counter32(_) => "COUNTER32",
            SnmpValue::Counter64(_) => "COUNTER64",
            SnmpValue::OctetString(_) => "OCTET STRING",
            SnmpValue::ObjectIdentifier(_) => "OBJECT IDENTIFIER",
            SnmpValue::IpAddress(_) => "IPADDRESS",
            SnmpValue::TimeTicks(_) => "TIMETICKS",
            SnmpValue::TruthValue(_) => "TRUTHVALUE",
            SnmpValue::Null => "NULL",
            SnmpValue::Raw { .. } => "RAW",
        }
    }
}

/// An OID paired with its live value returned from a Target.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableBinding {
    /// Dotted-decimal OID.
    pub oid: String,
    /// Live SNMP value.
    pub value: SnmpValue,
    /// Whether this binding contains a warning (e.g., partial decode).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub warning: bool,
}

/// A non-fatal issue encountered during an SNMP operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpWarning {
    /// Short label for the warning type.
    pub kind: String,
    /// Human-readable description.
    pub message: String,
    /// OID associated with this warning (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
}

/// Output of an Execution — Variable Bindings plus any warnings or errors.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultSet {
    /// Successfully decoded variable bindings.
    pub bindings: Vec<VariableBinding>,
    /// Non-fatal warnings collected during tolerance handling.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<SnmpWarning>,
    /// Whether the operation completed fully or returned partial results.
    pub partial: bool,
    /// Total number of retries attempted (0 means no retry was needed).
    #[serde(skip_serializing_if = "is_zero")]
    pub retries: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl Default for ResultSet {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultSet {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            warnings: Vec::new(),
            partial: false,
            retries: 0,
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty() && self.warnings.is_empty()
    }

    #[cfg(test)]
    pub fn merge(&mut self, other: ResultSet) {
        self.bindings.extend(other.bindings);
        self.warnings.extend(other.warnings);
        self.partial |= other.partial;
        self.retries = self.retries.max(other.retries);
    }
}

/// Value to write during a Set operation.
#[derive(Debug, Clone)]
pub enum SetValue {
    Integer(i64),
    OctetString(Vec<u8>),
    Unsigned32(u32),
    Counter32(u32),
    Counter64(u64),
    IpAddress(String),
    TimeTicks(u32),
    ObjectIdentifier(String),
}

#[cfg(test)]
mod v3_resolve_tests {
    use super::*;

    fn cfg(auth: AuthProtocol, priv_: PrivProtocol) -> SnmpV3SecurityConfig {
        SnmpV3SecurityConfig {
            username: "admin".to_string(),
            auth_protocol: auth,
            auth_passphrase: "authpass".to_string(),
            priv_protocol: priv_,
            priv_passphrase: "privpass".to_string(),
        }
    }

    // ── noAuthNoPriv ────────────────────────────────────────────────────────

    #[test]
    fn resolve_no_auth_no_priv() {
        let plan = cfg(AuthProtocol::None, PrivProtocol::None)
            .resolve()
            .unwrap();
        assert_eq!(plan.auth, snmp2::v3::Auth::NoAuthNoPriv);
        assert_eq!(plan.auth_protocol, None);
        assert_eq!(plan.key_extension, None);
    }

    // ── authNoPriv: every supported auth protocol ───────────────────────────

    #[test]
    fn resolve_auth_no_priv_md5() {
        let plan = cfg(AuthProtocol::Md5, PrivProtocol::None)
            .resolve()
            .unwrap();
        assert_eq!(plan.auth, snmp2::v3::Auth::AuthNoPriv);
        assert_eq!(plan.auth_protocol, Some(snmp2::v3::AuthProtocol::Md5));
        assert_eq!(plan.key_extension, None);
    }

    #[test]
    fn resolve_auth_no_priv_sha1() {
        let plan = cfg(AuthProtocol::Sha1, PrivProtocol::None)
            .resolve()
            .unwrap();
        assert_eq!(plan.auth, snmp2::v3::Auth::AuthNoPriv);
        assert_eq!(plan.auth_protocol, Some(snmp2::v3::AuthProtocol::Sha1));
    }

    #[test]
    fn resolve_auth_no_priv_sha224() {
        let plan = cfg(AuthProtocol::Sha224, PrivProtocol::None)
            .resolve()
            .unwrap();
        assert_eq!(plan.auth_protocol, Some(snmp2::v3::AuthProtocol::Sha224));
    }

    #[test]
    fn resolve_auth_no_priv_sha256() {
        let plan = cfg(AuthProtocol::Sha256, PrivProtocol::None)
            .resolve()
            .unwrap();
        assert_eq!(plan.auth_protocol, Some(snmp2::v3::AuthProtocol::Sha256));
    }

    #[test]
    fn resolve_auth_no_priv_sha384() {
        let plan = cfg(AuthProtocol::Sha384, PrivProtocol::None)
            .resolve()
            .unwrap();
        assert_eq!(plan.auth_protocol, Some(snmp2::v3::AuthProtocol::Sha384));
    }

    #[test]
    fn resolve_auth_no_priv_sha512() {
        let plan = cfg(AuthProtocol::Sha512, PrivProtocol::None)
            .resolve()
            .unwrap();
        assert_eq!(plan.auth_protocol, Some(snmp2::v3::AuthProtocol::Sha512));
    }

    // ── authPriv: full matrix (6 auth × 4 priv) ─────────────────────────────

    fn expect_auth_priv(
        auth: AuthProtocol,
        priv_: PrivProtocol,
        cipher: snmp2::v3::Cipher,
        ap: snmp2::v3::AuthProtocol,
        key_ext: Option<snmp2::v3::KeyExtension>,
    ) {
        let plan = cfg(auth, priv_).resolve().unwrap();
        assert_eq!(
            plan.auth,
            snmp2::v3::Auth::AuthPriv {
                cipher,
                privacy_password: b"privpass".to_vec(),
            },
            "auth={:?} priv={:?}",
            auth,
            priv_
        );
        assert_eq!(
            plan.auth_protocol,
            Some(ap),
            "auth={:?} priv={:?}",
            auth,
            priv_
        );
        assert_eq!(
            plan.key_extension, key_ext,
            "auth={:?} priv={:?}",
            auth, priv_
        );
    }

    // DES and AES-128 never need a key extension (hash output ≥ 16 bytes).
    #[test]
    fn resolve_auth_priv_des_never_extends() {
        let cases = [
            (AuthProtocol::Md5, snmp2::v3::AuthProtocol::Md5),
            (AuthProtocol::Sha1, snmp2::v3::AuthProtocol::Sha1),
            (AuthProtocol::Sha224, snmp2::v3::AuthProtocol::Sha224),
            (AuthProtocol::Sha256, snmp2::v3::AuthProtocol::Sha256),
            (AuthProtocol::Sha384, snmp2::v3::AuthProtocol::Sha384),
            (AuthProtocol::Sha512, snmp2::v3::AuthProtocol::Sha512),
        ];
        for (a, ap) in cases {
            expect_auth_priv(a, PrivProtocol::Des, snmp2::v3::Cipher::Des, ap, None);
        }
    }

    #[test]
    fn resolve_auth_priv_aes128_never_extends() {
        let cases = [
            (AuthProtocol::Md5, snmp2::v3::AuthProtocol::Md5),
            (AuthProtocol::Sha1, snmp2::v3::AuthProtocol::Sha1),
            (AuthProtocol::Sha224, snmp2::v3::AuthProtocol::Sha224),
            (AuthProtocol::Sha256, snmp2::v3::AuthProtocol::Sha256),
            (AuthProtocol::Sha384, snmp2::v3::AuthProtocol::Sha384),
            (AuthProtocol::Sha512, snmp2::v3::AuthProtocol::Sha512),
        ];
        for (a, ap) in cases {
            expect_auth_priv(a, PrivProtocol::Aes128, snmp2::v3::Cipher::Aes128, ap, None);
        }
    }

    // AES-192 needs extension only when the auth hash is < 24 bytes (MD5=16, SHA1=20).
    #[test]
    fn resolve_auth_priv_aes192_extension_table() {
        expect_auth_priv(
            AuthProtocol::Md5,
            PrivProtocol::Aes192,
            snmp2::v3::Cipher::Aes192,
            snmp2::v3::AuthProtocol::Md5,
            Some(snmp2::v3::KeyExtension::Reeder),
        );
        expect_auth_priv(
            AuthProtocol::Sha1,
            PrivProtocol::Aes192,
            snmp2::v3::Cipher::Aes192,
            snmp2::v3::AuthProtocol::Sha1,
            Some(snmp2::v3::KeyExtension::Reeder),
        );
        expect_auth_priv(
            AuthProtocol::Sha224,
            PrivProtocol::Aes192,
            snmp2::v3::Cipher::Aes192,
            snmp2::v3::AuthProtocol::Sha224,
            None,
        );
        expect_auth_priv(
            AuthProtocol::Sha256,
            PrivProtocol::Aes192,
            snmp2::v3::Cipher::Aes192,
            snmp2::v3::AuthProtocol::Sha256,
            None,
        );
        expect_auth_priv(
            AuthProtocol::Sha384,
            PrivProtocol::Aes192,
            snmp2::v3::Cipher::Aes192,
            snmp2::v3::AuthProtocol::Sha384,
            None,
        );
        expect_auth_priv(
            AuthProtocol::Sha512,
            PrivProtocol::Aes192,
            snmp2::v3::Cipher::Aes192,
            snmp2::v3::AuthProtocol::Sha512,
            None,
        );
    }

    // AES-256 needs extension when the auth hash is < 32 bytes (MD5, SHA1, SHA224).
    #[test]
    fn resolve_auth_priv_aes256_extension_table() {
        expect_auth_priv(
            AuthProtocol::Md5,
            PrivProtocol::Aes256,
            snmp2::v3::Cipher::Aes256,
            snmp2::v3::AuthProtocol::Md5,
            Some(snmp2::v3::KeyExtension::Reeder),
        );
        expect_auth_priv(
            AuthProtocol::Sha1,
            PrivProtocol::Aes256,
            snmp2::v3::Cipher::Aes256,
            snmp2::v3::AuthProtocol::Sha1,
            Some(snmp2::v3::KeyExtension::Reeder),
        );
        expect_auth_priv(
            AuthProtocol::Sha224,
            PrivProtocol::Aes256,
            snmp2::v3::Cipher::Aes256,
            snmp2::v3::AuthProtocol::Sha224,
            Some(snmp2::v3::KeyExtension::Reeder),
        );
        expect_auth_priv(
            AuthProtocol::Sha256,
            PrivProtocol::Aes256,
            snmp2::v3::Cipher::Aes256,
            snmp2::v3::AuthProtocol::Sha256,
            None,
        );
        expect_auth_priv(
            AuthProtocol::Sha384,
            PrivProtocol::Aes256,
            snmp2::v3::Cipher::Aes256,
            snmp2::v3::AuthProtocol::Sha384,
            None,
        );
        expect_auth_priv(
            AuthProtocol::Sha512,
            PrivProtocol::Aes256,
            snmp2::v3::Cipher::Aes256,
            snmp2::v3::AuthProtocol::Sha512,
            None,
        );
    }

    // ── Validation: privacy requires authentication ─────────────────────────

    #[test]
    fn resolve_priv_without_auth_is_rejected() {
        for priv_ in [
            PrivProtocol::Des,
            PrivProtocol::Aes128,
            PrivProtocol::Aes192,
            PrivProtocol::Aes256,
        ] {
            let err = cfg(AuthProtocol::None, priv_)
                .resolve()
                .expect_err("priv without auth must fail");
            assert!(
                err.contains("requires an authentication protocol"),
                "priv={:?}: unexpected error message: {}",
                priv_,
                err
            );
        }
    }

    // ── Validation: protocols require non-empty passphrases ─────────────────

    #[test]
    fn resolve_auth_with_empty_passphrase_is_rejected() {
        for auth in [
            AuthProtocol::Md5,
            AuthProtocol::Sha1,
            AuthProtocol::Sha224,
            AuthProtocol::Sha256,
            AuthProtocol::Sha384,
            AuthProtocol::Sha512,
        ] {
            let mut c = cfg(auth, PrivProtocol::None);
            c.auth_passphrase.clear();
            let err = c.resolve().expect_err("empty auth passphrase must fail");
            assert!(
                err.contains("non-empty auth passphrase"),
                "auth={:?}: unexpected error message: {}",
                auth,
                err
            );
        }
    }

    #[test]
    fn resolve_priv_with_empty_passphrase_is_rejected() {
        for priv_ in [
            PrivProtocol::Des,
            PrivProtocol::Aes128,
            PrivProtocol::Aes192,
            PrivProtocol::Aes256,
        ] {
            let mut c = cfg(AuthProtocol::Sha256, priv_);
            c.priv_passphrase.clear();
            let err = c.resolve().expect_err("empty priv passphrase must fail");
            assert!(
                err.contains("non-empty priv passphrase"),
                "priv={:?}: unexpected error message: {}",
                priv_,
                err
            );
        }
    }

    #[test]
    fn resolve_no_auth_empty_passphrase_is_allowed() {
        let mut c = cfg(AuthProtocol::None, PrivProtocol::None);
        c.auth_passphrase.clear();
        c.priv_passphrase.clear();
        assert!(c.resolve().is_ok());
    }

    // ── Passphrases are carried through to the AuthPriv cipher ──────────────

    #[test]
    fn resolve_auth_priv_carries_priv_passphrase() {
        let mut c = cfg(AuthProtocol::Sha256, PrivProtocol::Aes256);
        c.priv_passphrase = "s3cret-priv".to_string();
        let plan = c.resolve().unwrap();
        assert_eq!(
            plan.auth,
            snmp2::v3::Auth::AuthPriv {
                cipher: snmp2::v3::Cipher::Aes256,
                privacy_password: b"s3cret-priv".to_vec(),
            }
        );
    }
}
