use std::time::Duration;

use tracing::warn;

use super::types::*;

/// Maximum number of retry attempts on timeout.
const MAX_RETRIES: u32 = 3;

/// Exponential backoff delays: 1s, 2s, 4s.
fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_secs(2_u64.pow(attempt))
}

/// Converts a snmp2 `Value` into our tolerant `SnmpValue`, handling unexpected types gracefully.
pub fn value_to_snmp_value(v: snmp2::Value<'_>) -> (SnmpValue, bool) {
    match v {
        snmp2::Value::Integer(i) => (SnmpValue::Integer(i), false),
        snmp2::Value::Unsigned32(u) => (SnmpValue::Unsigned(u), false),
        snmp2::Value::Counter32(c) => (SnmpValue::Counter32(c), false),
        snmp2::Value::Counter64(c) => (SnmpValue::Counter64(c), false),
        snmp2::Value::OctetString(bytes) => (SnmpValue::OctetString(bytes.to_vec()), false),
        snmp2::Value::ObjectIdentifier(oid) => {
            (SnmpValue::ObjectIdentifier(oid.to_string()), false)
        }
        snmp2::Value::IpAddress(ip) => (
            SnmpValue::IpAddress(format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])),
            false,
        ),
        snmp2::Value::Timeticks(t) => (SnmpValue::TimeTicks(t), false),
        snmp2::Value::Opaque(bytes) => {
            warn!("Received Opaque ASN.1 type, storing as raw");
            (
                SnmpValue::Raw {
                    type_code: 0x44,
                    data: bytes.to_vec(),
                },
                true,
            )
        }
        snmp2::Value::Null => (SnmpValue::Null, false),
        snmp2::Value::Boolean(b) => (SnmpValue::TruthValue(b), false),
        // These are special sentinel values — not actual data.
        snmp2::Value::EndOfMibView | snmp2::Value::NoSuchObject | snmp2::Value::NoSuchInstance => {
            (SnmpValue::Null, false)
        }
        // PDU-level constructs that shouldn't appear in varbind values.
        snmp2::Value::Sequence(_) | snmp2::Value::Set(_) | snmp2::Value::Constructed(_, _) => {
            warn!("Received unexpected constructed ASN.1 type in value");
            (SnmpValue::Null, true)
        }
        // Request/response types — should not appear in response varbinds.
        snmp2::Value::GetRequest(_)
        | snmp2::Value::GetNextRequest(_)
        | snmp2::Value::GetBulkRequest(_)
        | snmp2::Value::Response(_)
        | snmp2::Value::SetRequest(_)
        | snmp2::Value::InformRequest(_)
        | snmp2::Value::Trap(_)
        | snmp2::Value::Report(_) => {
            warn!("Received PDU-level value in varbind — this is unexpected");
            (SnmpValue::Null, true)
        }
    }
}

/// Checks if a snmp2 error indicates a network/timeout issue (retryable).
pub fn is_retryable_error(err: &snmp2::Error) -> bool {
    matches!(err, snmp2::Error::Send | snmp2::Error::Receive)
}

/// Checks if a Value signals normal walk termination.
pub fn is_walk_termination_value(v: &snmp2::Value<'_>) -> bool {
    matches!(
        v,
        snmp2::Value::EndOfMibView | snmp2::Value::NoSuchObject | snmp2::Value::NoSuchInstance
    )
}

/// Executes an async operation with retry logic for network errors.
/// Returns `(result, retries)` where `retries` is the number of successful retries.
pub async fn with_retry<F, Fut, T>(mut operation: F) -> (Result<T, snmp2::Error>, u32)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, snmp2::Error>>,
{
    let mut last_err = None;

    for attempt in 0..=MAX_RETRIES {
        match operation().await {
            Ok(val) => return (Ok(val), attempt),
            Err(e) => {
                if is_retryable_error(&e) && attempt < MAX_RETRIES {
                    let delay = backoff_delay(attempt);
                    warn!(
                        "Network error on attempt {}/{} — retrying in {:?}",
                        attempt + 1,
                        MAX_RETRIES + 1,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                } else {
                    last_err = Some(e);
                    break;
                }
            }
        }
    }

    let err = last_err.unwrap_or(snmp2::Error::Receive);
    (Err(err), MAX_RETRIES)
}

/// Converts a snmp2 error into our warning format.
pub fn error_to_warning(err: &snmp2::Error, oid: Option<String>) -> SnmpWarning {
    let (kind, message) = match err {
        snmp2::Error::AsnParse => ("asn-parse", "ASN.1 parsing error in response"),
        snmp2::Error::AsnInvalidLen => ("asn-invalid-len", "ASN.1 invalid length in response"),
        snmp2::Error::AsnWrongType => ("asn-wrong-type", "ASN.1 wrong type in response"),
        snmp2::Error::AsnUnsupportedType => {
            ("asn-unsupported-type", "ASN.1 unsupported type in response")
        }
        snmp2::Error::AsnEof => ("asn-eof", "Unexpected end of SNMP response"),
        snmp2::Error::AsnIntOverflow => ("asn-int-overflow", "ASN.1 integer overflow in response"),
        snmp2::Error::UnsupportedVersion => (
            "unsupported-version",
            "Target uses unsupported SNMP version",
        ),
        snmp2::Error::RequestIdMismatch => ("request-id-mismatch", "SNMP request ID mismatch"),
        snmp2::Error::CommunityMismatch => {
            ("community-mismatch", "Community string rejected by Target")
        }
        snmp2::Error::ValueOutOfRange => ("value-out-of-range", "SNMP value out of range"),
        snmp2::Error::BufferOverflow => ("buffer-overflow", "SNMP response exceeds buffer size"),
        snmp2::Error::Send => ("send-error", "Failed to send SNMP request to Target"),
        snmp2::Error::Receive => (
            "receive-error",
            "No response received from Target (timeout or network error)",
        ),
        #[cfg(feature = "snmp2/v3")]
        snmp2::Error::AuthFailure(_) => ("auth-failure", "SNMPv3 authentication failed"),
        #[cfg(feature = "snmp2/v3")]
        snmp2::Error::Crypto(e) => {
            return SnmpWarning {
                kind: "crypto-error".to_string(),
                message: format!("Cryptographic error: {}", e),
                oid,
            }
        }
        #[cfg(feature = "snmp2/v3")]
        snmp2::Error::AuthUpdated => (
            "auth-updated",
            "SNMPv3 security context updated — retry may succeed",
        ),
        snmp2::Error::Mib(s) => {
            return SnmpWarning {
                kind: "mib-error".to_string(),
                message: format!("MIB error: {}", s),
                oid,
            }
        }
        // Catch-all for any future variants.
        _ => ("unknown-error", "Unknown SNMP error"),
    };

    SnmpWarning {
        kind: kind.to_string(),
        message: message.to_string(),
        oid,
    }
}

/// Creates a VariableBinding from an OID and snmp2 Value with tolerance handling.
pub fn binding_from_snmp(oid: String, value: snmp2::Value<'_>) -> VariableBinding {
    let (snmp_val, warning) = value_to_snmp_value(value);
    VariableBinding {
        oid,
        value: snmp_val,
        warning,
    }
}

/// Attempts to decode raw BER bytes into a SnmpValue.
pub fn try_decode_ber(bytes: &[u8]) -> (SnmpValue, bool) {
    if bytes.is_empty() {
        return (SnmpValue::Null, false);
    }

    let tag = bytes[0];

    // Parse BER length byte(s).
    let (data_start, data_len) = match bytes.get(1) {
        None => return (SnmpValue::Null, true),
        Some(&len_byte) if len_byte & 0x80 == 0 => (2, len_byte as usize),
        Some(&len_byte) => {
            let num_len_bytes = (len_byte & 0x7F) as usize;
            if bytes.len() < 2 + num_len_bytes {
                return (SnmpValue::Null, true);
            }
            let mut total_len: usize = 0;
            for i in 0..num_len_bytes {
                total_len = (total_len << 8) | (bytes[2 + i] as usize);
            }
            (2 + num_len_bytes, total_len)
        }
    };

    if bytes.len() < data_start + data_len {
        return (SnmpValue::Null, true);
    }
    let data = &bytes[data_start..data_start + data_len];

    match tag {
        0x02 => {
            let val = parse_integer(data);
            (SnmpValue::Integer(val), false)
        }
        0x04 => (SnmpValue::OctetString(data.to_vec()), false),
        0x06 => {
            let oid = parse_oid_bytes(data);
            (SnmpValue::ObjectIdentifier(oid), false)
        }
        0x40 => {
            if data.len() == 4 {
                let ip = format!("{}.{}.{}.{}", data[0], data[1], data[2], data[3]);
                (SnmpValue::IpAddress(ip), false)
            } else {
                warn!("Malformed IPADDRESS: expected 4 bytes, got {}", data.len());
                (
                    SnmpValue::Raw {
                        type_code: tag,
                        data: bytes.to_vec(),
                    },
                    true,
                )
            }
        }
        0x43 => {
            let val = parse_u32(data);
            (SnmpValue::TimeTicks(val), false)
        }
        0x44 => {
            warn!("Received opaque type in raw BER decode");
            (
                SnmpValue::Raw {
                    type_code: tag,
                    data: bytes.to_vec(),
                },
                true,
            )
        }
        0x05 if data.is_empty() => (SnmpValue::Null, false),
        0x01 => {
            let val = !data.is_empty() && data[data.len() - 1] != 0;
            (SnmpValue::TruthValue(val), false)
        }
        _ => {
            warn!("Unexpected ASN.1 type code 0x{:02x}, storing as raw", tag);
            (
                SnmpValue::Raw {
                    type_code: tag,
                    data: bytes.to_vec(),
                },
                true,
            )
        }
    }
}

/// Parses a BER-encoded integer into i64.
fn parse_integer(bytes: &[u8]) -> i64 {
    if bytes.is_empty() {
        return 0;
    }
    let mut result: i64 = 0;
    for &b in bytes {
        result = (result << 8) | (b as i64);
    }
    result
}

/// Parses a BER-encoded unsigned integer into u32.
fn parse_u32(bytes: &[u8]) -> u32 {
    if bytes.is_empty() {
        return 0;
    }
    let mut result: u32 = 0;
    for &b in bytes {
        result = (result << 8) | (b as u32);
    }
    result
}

/// Parses BER-encoded OID bytes into dotted-decimal string.
fn parse_oid_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "0.0".to_string();
    }

    let mut parts = Vec::new();
    let first = bytes[0];
    parts.push((first / 40).to_string());
    parts.push((first % 40).to_string());

    let mut idx = 1;
    while idx < bytes.len() {
        let mut val: u32 = 0;
        loop {
            let b = bytes[idx];
            val = (val << 7) | ((b & 0x7F) as u32);
            idx += 1;
            if b & 0x80 == 0 || idx >= bytes.len() {
                break;
            }
        }
        parts.push(val.to_string());
    }

    parts.join(".")
}

/// Creates a VariableBinding from raw BER bytes with tolerance handling.
pub fn binding_from_raw_ber(oid: String, ber_bytes: &[u8]) -> VariableBinding {
    let (snmp_val, warning) = try_decode_ber(ber_bytes);
    VariableBinding {
        oid,
        value: snmp_val,
        warning,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_integer_basic() {
        assert_eq!(parse_integer(&[0, 1]), 1);
        assert_eq!(parse_integer(&[0, 0, 0, 42]), 42);
        assert_eq!(parse_integer(&[]), 0);
    }

    #[test]
    fn parse_u32_basic() {
        assert_eq!(parse_u32(&[0, 1]), 1);
        assert_eq!(parse_u32(&[0, 0, 0, 255]), 255);
        assert_eq!(parse_u32(&[]), 0);
    }

    #[test]
    fn parse_oid_bytes_standard() {
        let bytes = [0x2b, 0x06, 0x01, 0x02, 0x01];
        assert_eq!(parse_oid_bytes(&bytes), "1.3.6.1.2.1");
    }

    #[test]
    fn parse_oid_bytes_empty() {
        assert_eq!(parse_oid_bytes(&[]), "0.0");
    }

    #[test]
    fn try_decode_ber_integer() {
        let (val, warning) = try_decode_ber(&[0x02, 0x01, 42]);
        assert!(!warning);
        assert_eq!(val, SnmpValue::Integer(42));
    }

    #[test]
    fn try_decode_ber_octet_string() {
        let bytes = b"hello";
        let ber = vec![0x04, 5, b'h', b'e', b'l', b'l', b'o'];
        let (val, warning) = try_decode_ber(&ber);
        assert!(!warning);
        assert_eq!(val, SnmpValue::OctetString(bytes.to_vec()));
    }

    #[test]
    fn try_decode_ber_unknown_type() {
        let (val, warning) = try_decode_ber(&[0xFF, 0x01, 42]);
        assert!(warning);
        match val {
            SnmpValue::Raw { type_code, .. } => assert_eq!(type_code, 0xFF),
            _ => panic!("expected Raw"),
        }
    }

    #[test]
    fn try_decode_ber_ip_address() {
        let (val, warning) = try_decode_ber(&[0x40, 0x04, 192, 168, 1, 1]);
        assert!(!warning);
        match val {
            SnmpValue::IpAddress(ip) => assert_eq!(ip, "192.168.1.1"),
            _ => panic!("expected IpAddress"),
        }
    }

    #[test]
    fn try_decode_ber_malformed_ip() {
        let (val, warning) = try_decode_ber(&[0x40, 0x02, 192, 168]);
        assert!(warning);
        match val {
            SnmpValue::Raw { type_code, .. } => assert_eq!(type_code, 0x40),
            _ => panic!("expected Raw"),
        }
    }

    #[test]
    fn backoff_delays_correct() {
        assert_eq!(backoff_delay(0), Duration::from_secs(1));
        assert_eq!(backoff_delay(1), Duration::from_secs(2));
        assert_eq!(backoff_delay(2), Duration::from_secs(4));
    }

    #[test]
    fn snmp_value_display_integer() {
        let v = SnmpValue::Integer(-42);
        assert_eq!(v.display(), "-42");
    }

    #[test]
    fn snmp_value_display_octet_string_utf8() {
        let v = SnmpValue::OctetString(b"hello".to_vec());
        assert_eq!(v.display(), "\"hello\"");
    }

    #[test]
    fn snmp_value_display_raw() {
        let v = SnmpValue::Raw {
            type_code: 0xAB,
            data: vec![1, 2, 3],
        };
        assert_eq!(v.display(), "<raw type=0xab data=0x010203>");
    }

    #[test]
    fn result_set_merge() {
        let mut rs = ResultSet::new();
        rs.bindings.push(VariableBinding {
            oid: "1.3.6.1".to_string(),
            value: SnmpValue::Integer(1),
            warning: false,
        });

        let other = ResultSet {
            bindings: vec![VariableBinding {
                oid: "1.3.6.2".to_string(),
                value: SnmpValue::Integer(2),
                warning: true,
            }],
            warnings: vec![],
            partial: true,
            retries: 2,
        };

        rs.merge(other);
        assert_eq!(rs.bindings.len(), 2);
        assert!(rs.partial);
        assert_eq!(rs.retries, 2);
    }

    #[test]
    fn result_set_empty() {
        let rs = ResultSet::new();
        assert!(rs.is_empty());
    }

    #[test]
    fn is_walk_termination_value_correct() {
        assert!(is_walk_termination_value(&snmp2::Value::EndOfMibView));
        assert!(is_walk_termination_value(&snmp2::Value::NoSuchInstance));
        assert!(is_walk_termination_value(&snmp2::Value::NoSuchObject));
        assert!(!is_walk_termination_value(&snmp2::Value::Null));
    }

    #[test]
    fn is_retryable_error_correct() {
        assert!(is_retryable_error(&snmp2::Error::Send));
        assert!(is_retryable_error(&snmp2::Error::Receive));
        assert!(!is_retryable_error(&snmp2::Error::AsnParse));
    }

    #[test]
    fn version_conversion_roundtrip() {
        assert_eq!(Version::from(snmp2::Version::V1), Version::V1);
        assert_eq!(Version::from(snmp2::Version::V2C), Version::V2c);
        assert_eq!(Version::from(snmp2::Version::V3), Version::V3);

        assert_eq!(snmp2::Version::from(Version::V1), snmp2::Version::V1);
        assert_eq!(snmp2::Version::from(Version::V2c), snmp2::Version::V2C);
        assert_eq!(snmp2::Version::from(Version::V3), snmp2::Version::V3);
    }

    #[test]
    fn value_to_snmp_value_integer() {
        let (v, w) = value_to_snmp_value(snmp2::Value::Integer(42));
        assert!(!w);
        assert_eq!(v, SnmpValue::Integer(42));
    }

    #[test]
    fn value_to_snmp_value_end_of_mib_view() {
        let (v, w) = value_to_snmp_value(snmp2::Value::EndOfMibView);
        assert!(!w);
        assert_eq!(v, SnmpValue::Null);
    }

    #[test]
    fn oid_parse_from_string() {
        let oid: snmp2::Oid = "1.3.6.1.2.1.1.1".parse().unwrap();
        assert_eq!(oid.to_string(), "1.3.6.1.2.1.1.1");
    }

    #[test]
    fn oid_from_u64_slice() {
        let oid = snmp2::Oid::from(&[1, 3, 6, 1, 2, 1, 1, 1]).unwrap();
        assert_eq!(oid.to_string(), "1.3.6.1.2.1.1.1");
    }
}
