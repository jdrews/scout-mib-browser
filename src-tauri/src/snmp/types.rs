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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

/// SNMP operation mode — determines what kind of request is sent to the Target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SnmpOperation {
    /// Retrieves exact values for specified OIDs.
    Get,
    /// Retrieves the next OID(s) after the specified one(s).
    GetNext,
    /// Walks an entire subtree starting from the given OID.
    Walk,
    /// Bulk walk using GETBULK for efficiency (v2c/v3 only).
    BulkWalk,
    /// Sets a value at the specified OID.
    Set,
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

impl SnmpValue {
    /// Human-readable display string for the value.
    pub fn display(&self) -> String {
        match self {
            SnmpValue::Integer(v) => format!("{}", v),
            SnmpValue::Unsigned(v) => format!("{}", v),
            SnmpValue::Counter32(v) => format!("{} (counter32)", v),
            SnmpValue::Counter64(v) => format!("{} (counter64)", v),
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
            SnmpValue::TimeTicks(v) => format!("{} (timeticks)", v),
            SnmpValue::TruthValue(v) => {
                if *v {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            SnmpValue::Null => "NULL".to_string(),
            SnmpValue::Raw { type_code, data } => format!(
                "<raw type=0x{:02x} data=0x{}>",
                type_code,
                data.iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            ),
        }
    }

    /// Type label for display.
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

    /// Returns `true` if the result set contains no bindings and no warnings.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty() && self.warnings.is_empty()
    }

    /// Combines another result set into this one (used for batch accumulation).
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
