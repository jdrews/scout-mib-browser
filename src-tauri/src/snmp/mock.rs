//! Mock SNMP server for testing without real network devices.
//!
//! Implements a minimal UDP-based SNMP responder that handles Get, GetNext,
//! and Set requests with configurable responses. Useful for unit tests
//! that need to verify the engine's behavior end-to-end.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Mock SNMP server that listens on a UDP port and responds to requests.
pub struct MockSnmpServer {
    /// Address the server is bound to.
    pub addr: SocketAddr,
    /// Internal state protected by a mutex for thread safety.
    inner: Arc<Mutex<MockServerInner>>,
}

struct MockServerInner {
    /// Mapped OIDs to their values (as raw BER-encoded bytes).
    data: HashMap<String, Vec<u8>>,
    /// Community string the server accepts.
    community: Vec<u8>,
    /// Number of requests received.
    request_count: u64,
}

impl MockSnmpServer {
    /// Starts a mock SNMP server on the given port with default data.
    pub fn new(port: u16) -> Self {
        let socket = UdpSocket::bind(format!("127.0.0.1:{}", port))
            .expect("Failed to bind UDP socket for mock server");
        socket.set_nonblocking(false).ok();

        let addr = socket.local_addr().unwrap();
        let inner = Arc::new(Mutex::new(MockServerInner {
            data: Self::default_mib_data(),
            community: b"public".to_vec(),
            request_count: 0,
        }));

        // Spawn a background thread to handle requests.
        let inner_clone = Arc::clone(&inner);
        thread::spawn(move || {
            Self::serve(socket, inner_clone);
        });

        Self { addr, inner }
    }

    /// Sets the community string the server accepts.
    pub fn set_community(&self, community: &str) {
        self.inner.lock().unwrap().community = community.as_bytes().to_vec();
    }

    /// Adds or updates a value at the given OID.
    pub fn set_value(&self, oid: &str, ber_bytes: Vec<u8>) {
        self.inner
            .lock()
            .unwrap()
            .data
            .insert(oid.to_string(), ber_bytes);
    }

    /// Returns the number of requests received so far.
    pub fn request_count(&self) -> u64 {
        self.inner.lock().unwrap().request_count
    }

    /// Default MIB data for testing — simulates a basic SNMP device.
    fn default_mib_data() -> HashMap<String, Vec<u8>> {
        let mut data = HashMap::new();

        // sysDescr (1.3.6.1.2.1.1.1.0) = "Linux router"
        data.insert(
            "1.3.6.1.2.1.1.1.0".to_string(),
            Self::ber_octet_string(b"Linux router"),
        );

        // sysObjectID (1.3.6.1.2.1.1.2.0) = 1.3.6.1.4.1.9.1.122
        data.insert(
            "1.3.6.1.2.1.1.2.0".to_string(),
            Self::ber_oid(&[1, 3, 6, 1, 4, 1, 9, 1, 122]),
        );

        // sysUpTime (1.3.6.1.2.1.1.3.0) = 1234567 timeticks
        data.insert(
            "1.3.6.1.2.1.1.3.0".to_string(),
            Self::ber_timeticks(1_234_567),
        );

        // sysName (1.3.6.1.2.1.1.5.0) = "test-router"
        data.insert(
            "1.3.6.1.2.1.1.5.0".to_string(),
            Self::ber_octet_string(b"test-router"),
        );

        // ifNumber (1.3.6.1.2.1.1.2.0) = 4 interfaces
        data.insert("1.3.6.1.2.1.2.1.0".to_string(), Self::ber_integer(4));

        // ifDescr entries for walk testing
        data.insert(
            "1.3.6.1.2.1.2.2.1.2.1".to_string(),
            Self::ber_octet_string(b"eth0"),
        );
        data.insert(
            "1.3.6.1.2.1.2.2.1.2.2".to_string(),
            Self::ber_octet_string(b"eth1"),
        );
        data.insert(
            "1.3.6.1.2.1.2.2.1.2.3".to_string(),
            Self::ber_octet_string(b"lo"),
        );

        // ifType entries
        data.insert(
            "1.3.6.1.2.1.2.2.1.3.1".to_string(),
            Self::ber_integer(6), // ethernetCsmacd
        );
        data.insert("1.3.6.1.2.1.2.2.1.3.2".to_string(), Self::ber_integer(6));
        data.insert(
            "1.3.6.1.2.1.2.2.1.3.3".to_string(),
            Self::ber_integer(24), // softwareLoopback
        );

        data
    }

    /// Background serve loop — reads UDP packets and sends responses.
    fn serve(socket: UdpSocket, inner: Arc<Mutex<MockServerInner>>) {
        let mut buf = [0u8; 65536];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, client_addr)) => {
                    let response = Self::handle_request(&buf[..len], &inner);
                    let _ = socket.send_to(&response, client_addr);
                }
                Err(e) => {
                    // Non-fatal — server may be dropped.
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        break;
                    }
                }
            }
        }
    }

    /// Handles an incoming SNMP request and returns a response PDU.
    fn handle_request(request: &[u8], inner: &Arc<Mutex<MockServerInner>>) -> Vec<u8> {
        let mut state = inner.lock().unwrap();
        state.request_count += 1;

        // Parse the community string from the request (simplified).
        if !Self::community_matches(request, &state.community) {
            return Self::build_error_response(request);
        }

        // Extract OIDs from the request varbinds.
        let oids = Self::extract_request_oids(request);
        if oids.is_empty() {
            return Self::build_error_response(request);
        }

        // Determine message type (Get, GetNext, Set).
        let msg_type = Self::extract_message_type(request);

        match msg_type {
            MessageType::Get => Self::build_get_response(request, &oids, &state.data),
            MessageType::GetNext => Self::build_getnext_response(request, &oids, &state.data),
            MessageType::Set => Self::build_set_response(request, &oids, &mut state.data),
            _ => Self::build_error_response(request),
        }
    }

    // ── BER helpers ────────────────────────────────────────────────────────

    /// Encodes a value as BER INTEGER.
    fn ber_integer(val: i32) -> Vec<u8> {
        let mut result = vec![0x02, 0x04]; // tag, length
        result.push(((val >> 24) & 0xFF) as u8);
        result.push(((val >> 16) & 0xFF) as u8);
        result.push(((val >> 8) & 0xFF) as u8);
        result.push((val & 0xFF) as u8);
        result
    }

    /// Encodes a value as BER OCTET STRING.
    fn ber_octet_string(data: &[u8]) -> Vec<u8> {
        let mut result = vec![0x04, data.len() as u8]; // tag, length
        result.extend_from_slice(data);
        result
    }

    /// Encodes a value as BER OBJECT IDENTIFIER.
    fn ber_oid(components: &[u64]) -> Vec<u8> {
        let mut encoded = Vec::new();
        if components.len() >= 2 {
            encoded.push((components[0] * 40 + components[1]) as u8);
        }
        for &c in &components[2..] {
            if c < 128 {
                encoded.push(c as u8);
            } else {
                let mut val = c;
                let mut bytes = Vec::new();
                while val > 0 {
                    bytes.push((val & 0x7F) as u8);
                    val >>= 7;
                }
                for (i, b) in bytes.iter().enumerate().rev() {
                    encoded.push(if i == bytes.len() - 1 { *b } else { *b | 0x80 });
                }
            }
        }
        let mut result = vec![0x06, encoded.len() as u8]; // tag, length
        result.extend_from_slice(&encoded);
        result
    }

    /// Encodes a value as BER TIMETICKS.
    fn ber_timeticks(val: u32) -> Vec<u8> {
        let mut result = vec![0x43, 0x04]; // tag, length
        result.push(((val >> 24) & 0xFF) as u8);
        result.push(((val >> 16) & 0xFF) as u8);
        result.push(((val >> 8) & 0xFF) as u8);
        result.push((val & 0xFF) as u8);
        result
    }

    // ── SNMP message parsing (simplified) ──────────────────────────────────

    /// Checks if the community string in the request matches.
    fn community_matches(_request: &[u8], _community: &[u8]) -> bool {
        // Simplified — always accept for testing.
        true
    }

    /// Extracts OIDs from a request's varbind list.
    fn extract_request_oids(request: &[u8]) -> Vec<String> {
        let mut oids = Vec::new();

        // Find the varbind sequence (look for 0x30 after PDU header).
        if let Some(start) = Self::find_varbind_start(request) {
            let mut pos = start;
            while pos + 4 < request.len() {
                // Each varbind is a SEQUENCE (0x30).
                if request[pos] != 0x30 {
                    break;
                }
                let vb_len = request[pos + 1] as usize;
                pos += 2;

                // Inside the varbind, find the OID (0x06).
                while pos < start + vb_len && pos < request.len() {
                    if request[pos] == 0x06 {
                        let oid_len = request[pos + 1] as usize;
                        let oid_bytes = &request[pos + 2..pos + 2 + oid_len];
                        oids.push(Self::decode_oid_string(oid_bytes));
                        break;
                    }
                    pos += 1;
                }

                pos += vb_len;
            }
        }

        oids
    }

    /// Finds the start of the varbind list in a request.
    fn find_varbind_start(request: &[u8]) -> Option<usize> {
        // SNMP PDU structure: version(3) + community(varies) + PDU type + req_id(5) + err_status(3) + err_index(3) + varbinds
        if request.len() < 10 {
            return None;
        }

        // Skip version (3 bytes).
        let mut pos = 3;

        // Skip community string.
        if pos >= request.len() {
            return None;
        }
        let comm_len = request[pos + 1] as usize;
        pos += 2 + comm_len;

        // PDU type byte (0xA0=Get, 0xA1=GetNext, 0xA3=Set).
        if pos >= request.len() {
            return None;
        }
        pos += 1;

        // Skip PDU length.
        if pos >= request.len() {
            return None;
        }
        let pdu_len = request[pos] as usize;
        pos += 1;

        // Skip req_id (5 bytes: tag + len + 4 bytes).
        pos += 5;
        // Skip err_status (3 bytes).
        pos += 3;
        // Skip err_index (3 bytes).
        pos += 3;

        if pos < request.len() {
            Some(pos)
        } else {
            None
        }
    }

    /// Decodes BER OID bytes to dotted-decimal string.
    fn decode_oid_string(bytes: &[u8]) -> String {
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

    /// Extracts the SNMP message type from a request.
    fn extract_message_type(request: &[u8]) -> MessageType {
        // Find PDU type byte (after version + community).
        if request.len() < 5 {
            return MessageType::Unknown;
        }

        let mut pos = 3; // Skip version.
        if pos >= request.len() {
            return MessageType::Unknown;
        }
        let comm_len = request[pos + 1] as usize;
        pos += 2 + comm_len;

        if pos >= request.len() {
            return MessageType::Unknown;
        }

        match request[pos] {
            0xA0 => MessageType::Get,
            0xA1 => MessageType::GetNext,
            0xA3 => MessageType::Set,
            _ => MessageType::Unknown,
        }
    }

    // ── Response builders ──────────────────────────────────────────────────

    /// Builds a Get response with values from the data store.
    fn build_get_response(
        request: &[u8],
        oids: &[String],
        data: &HashMap<String, Vec<u8>>,
    ) -> Vec<u8> {
        let mut varbinds = Vec::new();

        for oid in oids {
            if let Some(value) = data.get(oid) {
                // Build varbind: SEQUENCE + OID + value.
                let oid_ber = Self::ber_oid_from_string(oid);
                let vb_data = [&oid_ber, value].concat();
                varbinds.push(vec![0x30, vb_data.len() as u8]);
                varbinds.push(vb_data);
            } else {
                // NoSuchInstance.
                varbinds.push(vec![0x30, 0x09]);
                let oid_ber = Self::ber_oid_from_string(oid);
                varbinds.push(oid_ber);
                varbinds.push(vec![0x81, 0x00]); // NoSuchInstance
            }
        }

        let vb_list: Vec<u8> = varbinds.concat();
        Self::build_response_pdu(request, &vb_list)
    }

    /// Builds a GetNext response with the next OID after each requested OID.
    fn build_getnext_response(
        request: &[u8],
        oids: &[String],
        data: &HashMap<String, Vec<u8>>,
    ) -> Vec<u8> {
        let mut varbinds = Vec::new();

        for oid in oids {
            // Find the next OID after the requested one.
            if let Some(next) = Self::find_next_oid(oid, data) {
                let (next_oid, value) = next;
                let oid_ber = Self::ber_oid_from_string(&next_oid);
                let vb_data = [&oid_ber, value].concat();
                varbinds.push(vec![0x30, vb_data.len() as u8]);
                varbinds.push(vb_data);
            } else {
                // EndOfMibView.
                varbinds.push(vec![0x30, 0x09]);
                let oid_ber = Self::ber_oid_from_string(oid);
                varbinds.push(oid_ber);
                varbinds.push(vec![0x80, 0x00]); // EndOfMibView
            }
        }

        let vb_list: Vec<u8> = varbinds.concat();
        Self::build_response_pdu(request, &vb_list)
    }

    /// Builds a Set response echoing back the set values.
    fn build_set_response(
        request: &[u8],
        oids: &[String],
        data: &mut HashMap<String, Vec<u8>>,
    ) -> Vec<u8> {
        let mut varbinds = Vec::new();

        for oid in oids {
            // For Set, we'd extract the value from the request and store it.
            // Simplified: just echo back with success.
            let value = data
                .get(oid)
                .cloned()
                .unwrap_or_else(|| Self::ber_integer(0));
            let oid_ber = Self::ber_oid_from_string(oid);
            let vb_data = [&oid_ber, &value].concat();
            varbinds.push(vec![0x30, vb_data.len() as u8]);
            varbinds.push(vb_data);
        }

        let vb_list: Vec<u8> = varbinds.concat();
        Self::build_response_pdu(request, &vb_list)
    }

    /// Builds an error response.
    fn build_error_response(request: &[u8]) -> Vec<u8> {
        // Return empty varbind list with error status.
        let vb_list = vec![0x30, 0x00]; // Empty SEQUENCE
        Self::build_response_pdu(request, &vb_list)
    }

    /// Builds the response PDU wrapper around varbind data.
    fn build_response_pdu(_request: &[u8], vb_list: &[u8]) -> Vec<u8> {
        let mut pdu = Vec::new();

        // SNMPv2c version.
        pdu.extend_from_slice(&[0x02, 0x01, 0x01]); // INTEGER, len=1, value=1 (V2C)

        // Community string "public".
        pdu.push(0x04); // OCTET STRING tag
        pdu.push(b"public".len() as u8);
        pdu.extend_from_slice(b"public");

        // Response PDU type.
        pdu.push(0xA1); // Response

        // PDU length (req_id + err_status + err_index + varbinds).
        let pdu_len = 5 + 3 + 3 + vb_list.len();
        pdu.push(pdu_len as u8);

        // Request ID (echo from request, simplified to fixed value).
        pdu.extend_from_slice(&[0x02, 0x04, 0x00, 0x00, 0xDE, 0xAD]);

        // Error status = 0 (noError).
        pdu.extend_from_slice(&[0x02, 0x01, 0x00]);

        // Error index = 0.
        pdu.extend_from_slice(&[0x02, 0x01, 0x00]);

        // Varbind list.
        pdu.extend_from_slice(vb_list);

        // Wrap in outer SEQUENCE.
        let inner_len = pdu.len();
        let mut result = vec![0x30, inner_len as u8];
        result.extend_from_slice(&pdu);

        result
    }

    /// Encodes an OID string to BER bytes.
    fn ber_oid_from_string(oid_str: &str) -> Vec<u8> {
        let components: Vec<u64> = oid_str.split('.').filter_map(|s| s.parse().ok()).collect();
        Self::ber_oid(&components)
    }

    /// Finds the next OID in sorted order after the given OID.
    fn find_next_oid<'a>(
        current: &str,
        data: &'a HashMap<String, Vec<u8>>,
    ) -> Option<(String, &'a Vec<u8>)> {
        let mut candidates: Vec<_> = data.iter().filter(|(k, _)| k.as_str() > current).collect();
        candidates.sort_by_key(|(k, _)| *k);

        if let Some((oid, value)) = candidates.first() {
            Some((oid.clone(), value))
        } else {
            None
        }
    }
}

/// SNMP message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageType {
    Get,
    GetNext,
    Set,
    Unknown,
}

impl Drop for MockSnmpServer {
    fn drop(&mut self) {
        // The background thread will exit when the socket is dropped.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ber_integer_encoding() {
        let encoded = MockSnmpServer::ber_integer(42);
        assert_eq!(encoded, vec![0x02, 0x04, 0x00, 0x00, 0x00, 42]);
    }

    #[test]
    fn ber_octet_string_encoding() {
        let encoded = MockSnmpServer::ber_octet_string(b"hello");
        assert_eq!(encoded, vec![0x04, 5, b'h', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn ber_oid_encoding() {
        let encoded = MockSnmpServer::ber_oid(&[1, 3, 6, 1, 2, 1]);
        assert_eq!(encoded[0], 0x06); // OID tag
        assert_eq!(encoded[1], 5); // length
    }

    #[test]
    fn decode_oid_string_standard() {
        let bytes = [0x2b, 0x06, 0x01, 0x02, 0x01];
        assert_eq!(MockSnmpServer::decode_oid_string(&bytes), "1.3.6.1.2.1");
    }

    #[test]
    fn ber_timeticks_encoding() {
        let encoded = MockSnmpServer::ber_timeticks(1_234_567);
        assert_eq!(encoded[0], 0x43); // TIMETICKS tag
        assert_eq!(encoded[1], 4); // length
    }

    #[test]
    fn find_next_oid_basic() {
        let mut data = HashMap::new();
        data.insert("1.3.6.1.2.1.1.1.0".to_string(), vec![1]);
        data.insert("1.3.6.1.2.1.1.2.0".to_string(), vec![2]);
        data.insert("1.3.6.1.2.1.1.5.0".to_string(), vec![3]);

        let next = MockSnmpServer::find_next_oid("1.3.6.1.2.1.1.1.0", &data);
        assert!(next.is_some());
        assert_eq!(next.unwrap().0, "1.3.6.1.2.1.1.2.0");
    }

    #[test]
    fn find_next_oid_no_successor() {
        let mut data = HashMap::new();
        data.insert("1.3.6.1.2.1.1.1.0".to_string(), vec![1]);

        let next = MockSnmpServer::find_next_oid("9.9.9.9", &data);
        assert!(next.is_none());
    }

    #[test]
    fn mock_server_starts() {
        // Use a random high port to avoid conflicts.
        let server = MockSnmpServer::new(0);
        // Server should be bound to some address.
        assert!(!server.addr.ip().is_unspecified());
    }

    #[test]
    fn default_mib_data_has_entries() {
        let data = MockSnmpServer::default_mib_data();
        assert!(data.contains_key("1.3.6.1.2.1.1.1.0")); // sysDescr
        assert!(data.contains_key("1.3.6.1.2.1.1.5.0")); // sysName
        assert!(data.len() >= 8);
    }
}
