//! In-process mock of an SNMP Target for testing without a real network.
//!
//! Implements a minimal UDP-based SNMPv2c responder that handles Get,
//! GetNext, GetBulk, and Set requests. Requests are parsed with a proper
//! BER TLV walk (multi-byte lengths, variable-width INTEGERs) so the mock
//! interoperates with real client encodings; responses echo the request ID
//! exactly as sent.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// PDU tags (RFC 3416 §3, constructed APPLICATION class):
// get-request [0], get-next-request [1], response [2], set-request [3],
// [4] obsolete, get-bulk-request [5].
const TAG_GET_REQUEST: u8 = 0xA0;
const TAG_GET_NEXT_REQUEST: u8 = 0xA1;
const TAG_RESPONSE: u8 = 0xA2;
const TAG_SET_REQUEST: u8 = 0xA3;
const TAG_GET_BULK_REQUEST: u8 = 0xA5;

// Exception value tags (RFC 3416 §3, primitive APPLICATION class):
// noSuchObject [0], noSuchInstance [1], endOfMibView [2].
const TAG_NO_SUCH_INSTANCE: u8 = 0x81;
const TAG_END_OF_MIB_VIEW: u8 = 0x82;

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
    /// Total requests received (one per SNMP datagram).
    request_count: usize,
    /// Number of distinct walk chains observed. A chain starts at the first
    /// request and continues while each requested OID is strictly greater
    /// than the previous one; a request below the previous one marks a new
    /// walk (e.g., the engine starting a second per-column walk).
    walk_chains: usize,
    /// The OID of the most recent request, as sub-identifiers.
    last_request_oid: Option<Vec<u64>>,
    /// Set on drop so the serve loop exits promptly.
    stop: AtomicBool,
}

impl MockSnmpServer {
    /// Starts a mock SNMP server on the given port with default data.
    /// Port 0 binds to an ephemeral port; see [`MockSnmpServer::addr`].
    pub fn new(port: u16) -> Self {
        let socket = UdpSocket::bind(format!("127.0.0.1:{}", port))
            .expect("Failed to bind UDP socket for mock server");
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .ok();

        let addr = socket.local_addr().unwrap();
        let inner = Arc::new(Mutex::new(MockServerInner {
            data: Self::default_mib_data(),
            request_count: 0,
            walk_chains: 0,
            last_request_oid: None,
            stop: AtomicBool::new(false),
        }));

        // Spawn a background thread to handle requests.
        let serve_socket = socket
            .try_clone()
            .expect("Failed to clone UDP socket for mock server");
        let inner_clone = Arc::clone(&inner);
        thread::spawn(move || {
            Self::serve(serve_socket, inner_clone);
        });

        Self { addr, inner }
    }

    /// Adds or updates a value at the given OID.
    pub fn set_value(&self, oid: &str, ber_bytes: Vec<u8>) {
        self.inner
            .lock()
            .unwrap()
            .data
            .insert(oid.to_string(), ber_bytes);
    }

    /// Total number of request datagrams received.
    pub fn request_count(&self) -> usize {
        self.inner.lock().unwrap().request_count
    }

    /// Number of distinct walk chains observed (see `walk_chains`).
    pub fn walk_chain_count(&self) -> usize {
        self.inner.lock().unwrap().walk_chains
    }

    /// Default MIB data for testing — simulates a basic SNMP Target.
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

        // ifNumber (1.3.6.1.2.1.2.1.0) = 4 interfaces
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
            if inner.lock().unwrap().stop.load(Ordering::Acquire) {
                break;
            }
            match socket.recv_from(&mut buf) {
                Ok((len, client_addr)) => {
                    let response = Self::handle_request(&buf[..len], &inner);
                    let _ = socket.send_to(&response, client_addr);
                }
                Err(e) => match e.kind() {
                    // Read timeout — loop back and re-check the stop flag.
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => continue,
                    // Socket closed or other fatal error — exit.
                    _ => break,
                },
            }
        }
    }

    /// Handles an incoming SNMP request and returns a response message.
    fn handle_request(request: &[u8], inner: &Arc<Mutex<MockServerInner>>) -> Vec<u8> {
        let mut state = inner.lock().unwrap();

        if let Some(parsed) = Self::parse_request(request) {
            // Track request volume and walk-chain structure so tests can
            // assert single-pass table retrieval (one chain, not one per column).
            state.request_count += 1;
            if let Some(oid) = parsed.oids.first().map(|(o, _)| o.clone()) {
                let parts = Self::oid_parts(&oid);
                match &state.last_request_oid {
                    None => state.walk_chains = 1,
                    Some(last) => {
                        if Self::oid_less_or_eq(&parts, last) {
                            state.walk_chains += 1;
                        }
                    }
                }
                state.last_request_oid = Some(parts);
            }

            return match parsed.msg_type {
                MessageType::Get => Self::build_get_response(&parsed, &state.data),
                MessageType::GetNext | MessageType::GetBulk => {
                    Self::build_getnext_response(&parsed, &state.data)
                }
                MessageType::Set => Self::build_set_response(&parsed, &mut state.data),
            };
        }

        Self::build_error_response(request)
    }

    // ── BER helpers ────────────────────────────────────────────────────────

    /// Encodes a value as BER INTEGER (minimal two's-complement form).
    pub fn ber_integer(val: i32) -> Vec<u8> {
        let mut bytes = val.to_be_bytes().to_vec();
        while bytes.len() > 1 {
            // Strip redundant sign-extension bytes (keep one).
            let redundant = (bytes[0] == 0 && (bytes[1] & 0x80) == 0)
                || (bytes[0] == 0xFF && (bytes[1] & 0x80) != 0);
            if !redundant {
                break;
            }
            bytes.remove(0);
        }
        let mut result = vec![0x02, bytes.len() as u8];
        result.extend_from_slice(&bytes);
        result
    }

    /// Encodes a value as BER OCTET STRING.
    pub fn ber_octet_string(data: &[u8]) -> Vec<u8> {
        let mut result = vec![0x04, data.len() as u8]; // tag, length
        result.extend_from_slice(data);
        result
    }

    /// Encodes a value as BER OBJECT IDENTIFIER.
    pub fn ber_oid(components: &[u64]) -> Vec<u8> {
        let mut encoded = Vec::new();
        if components.len() >= 2 {
            encoded.push((components[0] * 40 + components[1]) as u8);
        }
        for &c in &components[2..] {
            if c < 128 {
                encoded.push(c as u8);
            } else {
                // Base-128 groups, most significant first; every byte except
                // the last carries the continuation bit.
                let mut val = c;
                let mut bytes = Vec::new();
                while val > 0 {
                    bytes.push((val & 0x7F) as u8);
                    val >>= 7;
                }
                for (i, b) in bytes.iter().enumerate().rev() {
                    encoded.push(if i == 0 { *b } else { *b | 0x80 });
                }
            }
        }
        let mut result = vec![0x06, encoded.len() as u8]; // tag, length
        result.extend_from_slice(&encoded);
        result
    }

    /// Encodes a value as BER TIMETICKS.
    pub fn ber_timeticks(val: u32) -> Vec<u8> {
        let mut result = vec![0x43, 0x04]; // tag, length
        result.push(((val >> 24) & 0xFF) as u8);
        result.push(((val >> 16) & 0xFF) as u8);
        result.push(((val >> 8) & 0xFF) as u8);
        result.push((val & 0xFF) as u8);
        result
    }

    /// Encodes a BER length (single- or multi-byte form).
    fn ber_length(len: usize) -> Vec<u8> {
        if len < 0x80 {
            return vec![len as u8];
        }
        let mut bytes = Vec::new();
        let mut v = len;
        while v > 0 {
            bytes.push((v & 0xFF) as u8);
            v >>= 8;
        }
        bytes.reverse();
        let mut result = vec![0x80 | bytes.len() as u8];
        result.extend_from_slice(&bytes);
        result
    }

    // ── Request parsing ────────────────────────────────────────────────────

    /// Parses an SNMPv2c request message via a proper BER TLV walk.
    fn parse_request(request: &[u8]) -> Option<ParsedRequest> {
        let mut ber = Ber::new(request);

        // Outer SEQUENCE — cursor now points at its contents.
        let (tag, _start, end) = ber.tlv()?;
        if tag != 0x30 || end != request.len() {
            return None;
        }

        // version INTEGER.
        let (tag, _, v_end) = ber.tlv()?;
        if tag != 0x02 {
            return None;
        }
        ber.seek(v_end);

        // community OCTET STRING.
        let (tag, c_start, c_end) = ber.tlv()?;
        if tag != 0x04 {
            return None;
        }
        let community = request[c_start..c_end].to_vec();
        ber.seek(c_end);

        // PDU (constructed APPLICATION).
        let (pdu_tag, pdu_start, pdu_end) = ber.tlv()?;
        let msg_type = match pdu_tag {
            TAG_GET_REQUEST => MessageType::Get,
            TAG_GET_NEXT_REQUEST => MessageType::GetNext,
            TAG_SET_REQUEST => MessageType::Set,
            TAG_GET_BULK_REQUEST => MessageType::GetBulk,
            _ => return None,
        };

        let pdu = &request[pdu_start..pdu_end];
        let mut inner = Ber::new(pdu);

        // request-id INTEGER — keep the full TLV for echoing.
        let id_pos = inner.pos;
        let (tag, _, id_end) = inner.tlv()?;
        if tag != 0x02 {
            return None;
        }
        let request_id = pdu[id_pos..id_end].to_vec();
        inner.seek(id_end);

        // BulkPDU is { request-id, non-repeaters, max-repetitions, varbinds };
        // every other PDU is { request-id, error-status, error-index, varbinds }.
        for _ in 0..2 {
            let (tag, _, e) = inner.tlv()?;
            if tag != 0x02 {
                return None;
            }
            inner.seek(e);
        }

        // varbind list: SEQUENCE OF.
        let (tag, vb_start, vb_end) = inner.tlv()?;
        if tag != 0x30 {
            return None;
        }
        let vb_list = &pdu[vb_start..vb_end];
        let mut vbs = Ber::new(vb_list);

        let mut oids = Vec::new();
        while vbs.pos < vbs.bytes.len() {
            let (tag, s, e) = vbs.tlv()?;
            if tag != 0x30 {
                return None;
            }
            let vb = &vbs.bytes[s..e];
            let mut vb_ber = Ber::new(vb);

            // OID.
            let (tag, o_start, o_end) = vb_ber.tlv()?;
            if tag != 0x06 {
                return None;
            }
            let oid = Self::decode_oid_string(&vb[o_start..o_end]);

            // Value TLV — present only in Set requests.
            let value = if msg_type == MessageType::Set {
                vb_ber.seek(o_end);
                let value_pos = vb_ber.pos;
                let (_, _, v_end) = vb_ber.tlv()?;
                Some(vb[value_pos..v_end].to_vec())
            } else {
                None
            };

            oids.push((oid, value));
            vbs.seek(e);
        }

        if oids.is_empty() {
            return None;
        }

        Some(ParsedRequest {
            msg_type,
            request_id,
            community,
            oids,
        })
    }

    /// Decodes BER OID bytes to dotted-decimal string.
    pub fn decode_oid_string(bytes: &[u8]) -> String {
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

    // ── Response builders ──────────────────────────────────────────────────

    /// Builds a Get response with values from the data store.
    fn build_get_response(parsed: &ParsedRequest, data: &HashMap<String, Vec<u8>>) -> Vec<u8> {
        let mut varbinds = Vec::new();

        for (oid, _) in &parsed.oids {
            match data.get(oid) {
                Some(value) => {
                    let oid_ber = Self::ber_oid_from_string(oid);
                    let vb_data = [&oid_ber[..], value.as_slice()].concat();
                    varbinds.push(Self::tlv(0x30, &vb_data));
                }
                None => {
                    // NoSuchInstance.
                    let oid_ber = Self::ber_oid_from_string(oid);
                    let vb_data = [&oid_ber[..], &[TAG_NO_SUCH_INSTANCE, 0x00]].concat();
                    varbinds.push(Self::tlv(0x30, &vb_data));
                }
            }
        }

        Self::build_response_pdu(parsed, &varbinds.concat())
    }

    /// Builds a GetNext/GetBulk response with the next OID after each requested OID.
    fn build_getnext_response(parsed: &ParsedRequest, data: &HashMap<String, Vec<u8>>) -> Vec<u8> {
        let mut varbinds = Vec::new();

        for (oid, _) in &parsed.oids {
            match Self::find_next_oid(oid, data) {
                Some((next_oid, value)) => {
                    let oid_ber = Self::ber_oid_from_string(&next_oid);
                    let vb_data = [&oid_ber[..], value.as_slice()].concat();
                    varbinds.push(Self::tlv(0x30, &vb_data));
                }
                None => {
                    // EndOfMibView.
                    let oid_ber = Self::ber_oid_from_string(oid);
                    let vb_data = [&oid_ber[..], &[TAG_END_OF_MIB_VIEW, 0x00]].concat();
                    varbinds.push(Self::tlv(0x30, &vb_data));
                }
            }
        }

        Self::build_response_pdu(parsed, &varbinds.concat())
    }

    /// Builds a Set response echoing back the set values.
    fn build_set_response(parsed: &ParsedRequest, data: &mut HashMap<String, Vec<u8>>) -> Vec<u8> {
        let mut varbinds = Vec::new();

        for (oid, value) in &parsed.oids {
            // Store the raw value so subsequent Gets return it.
            if let Some(v) = value {
                data.insert(oid.clone(), v.clone());
            }
            let echoed = value.clone().unwrap_or_else(|| Self::ber_integer(0));
            let oid_ber = Self::ber_oid_from_string(oid);
            let vb_data = [&oid_ber[..], echoed.as_slice()].concat();
            varbinds.push(Self::tlv(0x30, &vb_data));
        }

        Self::build_response_pdu(parsed, &varbinds.concat())
    }

    /// Builds an error response (noError status, empty varbind list).
    fn build_error_response(request: &[u8]) -> Vec<u8> {
        // Best effort: echo what we can from a malformed request.
        let parsed = Self::parse_request(request);
        match parsed {
            Some(p) => Self::build_response_pdu(&p, b""),
            None => Vec::new(),
        }
    }

    /// Builds the response message wrapper around a varbind list.
    fn build_response_pdu(parsed: &ParsedRequest, vb_list: &[u8]) -> Vec<u8> {
        let mut pdu = Vec::new();
        pdu.extend_from_slice(&parsed.request_id); // echo request-id verbatim
        pdu.extend_from_slice(&[0x02, 0x01, 0x00]); // error status = noError
        pdu.extend_from_slice(&[0x02, 0x01, 0x00]); // error index = 0
        pdu.extend_from_slice(&Self::tlv(0x30, vb_list)); // VarBindList SEQUENCE

        let mut message = Vec::new();
        message.extend_from_slice(&[0x02, 0x01, 0x01]); // version = v2c
        message.push(0x04); // community OCTET STRING
        message.extend_from_slice(&Self::ber_length(parsed.community.len()));
        message.extend_from_slice(&parsed.community);
        message.push(TAG_RESPONSE);
        message.extend_from_slice(&Self::ber_length(pdu.len()));
        message.extend_from_slice(&pdu);

        let mut result = vec![0x30];
        result.extend_from_slice(&Self::ber_length(message.len()));
        result.extend_from_slice(&message);
        result
    }

    /// Wraps content bytes in a TLV with the given tag.
    fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
        let mut result = vec![tag];
        result.extend_from_slice(&Self::ber_length(content.len()));
        result.extend_from_slice(content);
        result
    }

    /// Encodes an OID string to BER bytes.
    fn ber_oid_from_string(oid_str: &str) -> Vec<u8> {
        let components: Vec<u64> = oid_str.split('.').filter_map(|s| s.parse().ok()).collect();
        Self::ber_oid(&components)
    }

    /// Finds the next OID in sorted order after the given OID.
    ///
    /// Ordering is numeric per sub-identifier (as on the wire), not
    /// lexicographic — index `2` precedes `10`.
    pub fn find_next_oid<'a>(
        current: &str,
        data: &'a HashMap<String, Vec<u8>>,
    ) -> Option<(String, &'a Vec<u8>)> {
        let current_parts = Self::oid_parts(current);

        let mut candidates: Vec<(&String, &Vec<u8>, Vec<u64>)> = data
            .iter()
            .map(|(k, v)| (k, v, Self::oid_parts(k)))
            .filter(|(_, _, parts)| !Self::oid_less_or_eq(parts, &current_parts))
            .collect();
        candidates.sort_by(|a, b| Self::cmp_oid(&a.2, &b.2));

        candidates
            .first()
            .map(|(oid, value, _)| ((*oid).clone(), *value))
    }

    /// Splits a dotted-decimal OID into sub-identifiers.
    pub fn oid_parts(oid: &str) -> Vec<u64> {
        oid.split('.').filter_map(|s| s.parse().ok()).collect()
    }

    /// Compares two OIDs numerically, per sub-identifier (shorter prefix first).
    fn cmp_oid(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
        for (x, y) in a.iter().zip(b.iter()) {
            match x.cmp(y) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        a.len().cmp(&b.len())
    }

    /// True when OID `a` is strictly less than or equal to OID `b`.
    fn oid_less_or_eq(a: &[u64], b: &[u64]) -> bool {
        Self::cmp_oid(a, b) != std::cmp::Ordering::Greater
    }
}

/// A parsed SNMP request.
struct ParsedRequest {
    msg_type: MessageType,
    /// Raw BER INTEGER TLV from the request, echoed verbatim in the response.
    request_id: Vec<u8>,
    /// Community string bytes from the request, echoed in the response.
    community: Vec<u8>,
    /// Requested OIDs; for Set, also carries the raw value TLV per varbind.
    oids: Vec<(String, Option<Vec<u8>>)>,
}

/// Minimal BER TLV cursor over a byte slice.
struct Ber<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Ber<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Reads one TLV's tag and length, leaving the cursor at the start of
    /// the content (so container contents can be walked). Returns
    /// (tag, content_start, content_end).
    fn tlv(&mut self) -> Option<(u8, usize, usize)> {
        if self.pos + 1 > self.bytes.len() {
            return None;
        }
        let tag = self.bytes[self.pos];
        self.pos += 1;
        let len = self.read_length()?;
        let start = self.pos;
        if start.checked_add(len)? > self.bytes.len() {
            return None;
        }
        Some((tag, start, start + len))
    }

    /// Moves the cursor to an absolute position.
    fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// Reads a BER length (single- or multi-byte form), advancing past it.
    fn read_length(&mut self) -> Option<usize> {
        let first = *self.bytes.get(self.pos)?;
        self.pos += 1;
        if first < 0x80 {
            return Some(first as usize);
        }
        let count = (first & 0x7F) as usize;
        if count == 0 || count > 4 {
            return None;
        }
        let mut len: usize = 0;
        for _ in 0..count {
            len = (len << 8) | (*self.bytes.get(self.pos)? as usize);
            self.pos += 1;
        }
        Some(len)
    }
}

/// SNMP message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageType {
    Get,
    GetNext,
    GetBulk,
    Set,
}

impl Drop for MockSnmpServer {
    fn drop(&mut self) {
        // Signal the serve loop to exit; it wakes within its read timeout.
        self.inner
            .lock()
            .unwrap()
            .stop
            .store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ber_integer_encoding() {
        assert_eq!(MockSnmpServer::ber_integer(42), vec![0x02, 0x01, 42]);
        assert_eq!(MockSnmpServer::ber_integer(0), vec![0x02, 0x01, 0]);
        // Negative values keep a leading zero sign byte.
        assert_eq!(MockSnmpServer::ber_integer(-1), vec![0x02, 0x01, 0xFF]);
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
    fn ber_oid_roundtrip_large_component() {
        // Sub-identifiers >= 128 use multi-byte base-128 groups; the
        // continuation bit must be on every byte except the last.
        for &c in &[128u64, 99_997, 0xFFFF_FFFF] {
            let encoded = MockSnmpServer::ber_oid(&[1, 3, 6, 1, c]);
            // Content starts after tag + length byte(s).
            let content_len = if encoded[1] < 0x80 {
                2
            } else {
                2 + (encoded[1] & 0x7F) as usize
            };
            assert_eq!(
                MockSnmpServer::decode_oid_string(&encoded[content_len..]),
                format!("1.3.6.1.{}", c),
                "roundtrip failed for {}",
                c
            );
        }
    }

    #[test]
    fn ber_length_encoding() {
        assert_eq!(MockSnmpServer::ber_length(0x7F), vec![0x7F]);
        assert_eq!(MockSnmpServer::ber_length(0x80), vec![0x81, 0x80]);
        assert_eq!(MockSnmpServer::ber_length(300), vec![0x82, 0x01, 0x2C]);
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
    fn find_next_oid_numeric_not_lexicographic() {
        // G3 support: with rows 2 and 10, the successor of row 1 is row 2 —
        // lexicographic string order would wrongly pick "10".
        let mut data = HashMap::new();
        data.insert("1.3.6.1.4.1.99997.1.2".to_string(), vec![2]);
        data.insert("1.3.6.1.4.1.99997.1.10".to_string(), vec![10]);

        let next = MockSnmpServer::find_next_oid("1.3.6.1.4.1.99997.1.1", &data);
        assert_eq!(next.unwrap().0, "1.3.6.1.4.1.99997.1.2");

        // And after row 9 comes row 10.
        let next = MockSnmpServer::find_next_oid("1.3.6.1.4.1.99997.1.9", &data);
        assert_eq!(next.unwrap().0, "1.3.6.1.4.1.99997.1.10");
    }

    /// Builds a minimal SNMPv2c GetRequest message for parser tests.
    fn build_get_request(req_id: &[u8], oid: &str) -> Vec<u8> {
        let oid_ber = MockSnmpServer::ber_oid_from_string(oid);
        let vb = MockSnmpServer::tlv(0x30, &oid_ber);
        let vb_list = MockSnmpServer::tlv(0x30, &vb);
        let mut pdu = Vec::new();
        pdu.extend_from_slice(req_id);
        pdu.extend_from_slice(&[0x02, 0x01, 0x00]);
        pdu.extend_from_slice(&[0x02, 0x01, 0x00]);
        pdu.extend_from_slice(&vb_list);

        let mut message = Vec::new();
        message.extend_from_slice(&[0x02, 0x01, 0x01]);
        message.extend_from_slice(&[0x04, b"public".len() as u8]);
        message.extend_from_slice(b"public");
        message.push(TAG_GET_REQUEST);
        message.extend_from_slice(&MockSnmpServer::ber_length(pdu.len()));
        message.extend_from_slice(&pdu);

        let mut result = vec![0x30];
        result.extend_from_slice(&MockSnmpServer::ber_length(message.len()));
        result.extend_from_slice(&message);
        result
    }

    #[test]
    fn parse_get_request() {
        // 1-byte request-id INTEGER (minimal BER).
        let req = build_get_request(&[0x02, 0x01, 0x2A], "1.3.6.1.2.1.1.1.0");
        let parsed = MockSnmpServer::parse_request(&req).expect("should parse");
        assert_eq!(parsed.msg_type, MessageType::Get);
        assert_eq!(parsed.request_id, vec![0x02, 0x01, 0x2A]);
        assert_eq!(parsed.community, b"public");
        assert_eq!(parsed.oids.len(), 1);
        assert_eq!(parsed.oids[0].0, "1.3.6.1.2.1.1.1.0");
        assert!(parsed.oids[0].1.is_none());
    }

    #[test]
    fn parse_real_snmp2_getbulk_request() {
        // Exact datagram captured from a real snmp2 client GetBulk request.
        // BulkPDU layout: request-id, non-repeaters, max-repetitions, varbinds.
        let h =
            "302702010104067075626c6963a51a020100020100020132300f300d06092b06010201020201020500";
        let req: Vec<u8> = (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect();
        let parsed = MockSnmpServer::parse_request(&req).expect("should parse");
        assert_eq!(parsed.msg_type, MessageType::GetBulk);
        assert_eq!(parsed.request_id, vec![0x02, 0x01, 0x00]);
        assert_eq!(parsed.oids[0].0, "1.3.6.1.2.1.2.2.1.2");
    }

    #[test]
    fn parse_real_snmp2_set_request() {
        // Exact datagram captured from a real snmp2 client Set request.
        let h = "302d02010104067075626c6963a3200201000201000201003015301306082b06010201010500040772656e616d6564";
        let req: Vec<u8> = (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect();
        let parsed = MockSnmpServer::parse_request(&req).expect("should parse");
        assert_eq!(parsed.msg_type, MessageType::Set);
        assert_eq!(parsed.oids[0].0, "1.3.6.1.2.1.1.5.0");
        // The captured value must be the full OCTET STRING TLV.
        let mut expected = vec![0x04, 0x07];
        expected.extend_from_slice(b"renamed");
        assert_eq!(parsed.oids[0].1, Some(expected));
    }

    #[test]
    fn parse_real_snmp2_get_request() {
        // Exact datagram captured from a real snmp2 client Get request.
        let h = "302602010104067075626c6963a019020100020100020100300e300c06082b060102010101000500";
        let req: Vec<u8> = (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect();
        let parsed = MockSnmpServer::parse_request(&req).expect("should parse");
        assert_eq!(parsed.msg_type, MessageType::Get);
        // Full request-id TLV must be captured for echoing.
        assert_eq!(parsed.request_id, vec![0x02, 0x01, 0x00]);
        assert_eq!(parsed.community, b"public");
        assert_eq!(parsed.oids[0].0, "1.3.6.1.2.1.1.1.0");
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
