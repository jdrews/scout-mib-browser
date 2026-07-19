use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use tracing::{info, warn};

use super::tolerant::*;
use super::types::*;

/// Batch size for walk streaming.
const WALK_BATCH_SIZE: usize = 100;

/// Callback type for receiving walk/bulk_walk batch progress.
pub type WalkBatchCallback = dyn Fn(Vec<VariableBinding>) + Send + Sync;

/// Core SNMP engine that executes operations against a Target with error tolerance.
pub struct SnmpEngine {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl SnmpEngine {
    /// Creates a new engine backed by a dedicated tokio runtime.
    pub fn new() -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    /// Executes a Get operation for the given OIDs against the Target.
    pub fn get(&self, target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        self.runtime.block_on(self.do_get(target, oids))
    }

    /// Executes a GetNext operation for the given OIDs against the Target.
    pub fn get_next(&self, target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        self.runtime.block_on(self.do_get_next(target, oids))
    }

    /// Executes a Walk operation and returns all results.
    pub fn walk(&self, target: &Target, root_oid: &str) -> Result<ResultSet, String> {
        self.runtime.block_on(self.do_walk(target, root_oid, None))
    }

    /// Executes a Walk operation with batch callback for progress reporting.
    pub fn walk_with_callback(
        &self,
        target: &Target,
        root_oid: &str,
        callback: Arc<WalkBatchCallback>,
    ) -> Result<ResultSet, String> {
        self.runtime
            .block_on(self.do_walk(target, root_oid, Some(callback)))
    }

    /// Executes a BulkWalk operation and returns all results.
    pub fn bulk_walk(&self, target: &Target, root_oid: &str) -> Result<ResultSet, String> {
        self.runtime
            .block_on(self.do_bulk_walk(target, root_oid, None))
    }

    /// Executes a BulkWalk with batch callback for progress reporting.
    pub fn bulk_walk_with_callback(
        &self,
        target: &Target,
        root_oid: &str,
        callback: Arc<WalkBatchCallback>,
    ) -> Result<ResultSet, String> {
        self.runtime
            .block_on(self.do_bulk_walk(target, root_oid, Some(callback)))
    }

    /// Executes a Set operation to write a value at the given OID.
    pub fn set(&self, target: &Target, oid: &str, value: SetValue) -> Result<ResultSet, String> {
        self.runtime.block_on(self.do_set(target, oid, value))
    }

    // ── Async implementations ────────────────────────────────────────────────

    /// Extracts owned VariableBindings from a Pdu (consumes the iterator).
    fn extract_bindings(pdu: snmp2::Pdu<'_>) -> Vec<VariableBinding> {
        pdu.varbinds
            .map(|(o, v)| binding_from_snmp(o.to_string(), v))
            .collect()
    }

    async fn do_get(&self, target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        let target = target.clone();
        let oids_owned = oids.to_vec();

        // Parse OIDs once outside the retry loop.
        let snmp_oids: Vec<Arc<snmp2::Oid<'static>>> = oids_owned
            .iter()
            .map(|o| -> Result<Arc<snmp2::Oid<'static>>, String> {
                Ok(Arc::new(
                    o.parse()
                        .map_err(|e| format!("Invalid OID '{}': {:?}", o, e))?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Retry: each attempt creates a fresh session and extracts owned data.
        let (result, retries) = with_retry(|| {
            let t = target.clone();
            let oids: Vec<Arc<snmp2::Oid<'static>>> = snmp_oids.clone();
            async move {
                let mut session = Self::connect(&t).await.map_err(|_| snmp2::Error::Receive)?;

                let pdu = if oids.len() == 1 {
                    session.get(oids[0].as_ref()).await?
                } else {
                    let refs: Vec<&snmp2::Oid> = oids.iter().map(|a| a.as_ref()).collect();
                    session.get_many(&refs).await?
                };

                Ok::<Vec<VariableBinding>, snmp2::Error>(Self::extract_bindings(pdu))
            }
        })
        .await;

        match result {
            Ok(bindings) => {
                let mut rs = ResultSet::new();
                rs.retries = retries;
                rs.bindings = bindings;
                Ok(rs)
            }
            Err(e) => {
                warn!("Get failed after {} retries: {:?}", retries, e);
                let mut rs = ResultSet::new();
                rs.retries = retries;
                rs.partial = true;
                rs.warnings.push(error_to_warning(&e, None));
                Ok(rs)
            }
        }
    }

    async fn do_get_next(&self, target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        let mut rs = ResultSet::new();

        for oid_str in oids {
            let target = target.clone();
            let oid = oid_str.clone();

            let (result, retries) = with_retry(|| {
                let t = target.clone();
                let o = oid.clone();
                async move {
                    let mut session = Self::connect(&t).await.map_err(|_| snmp2::Error::Receive)?;
                    let parsed: snmp2::Oid = o.parse().map_err(|_| snmp2::Error::AsnParse)?;
                    let pdu = session.getnext(&parsed).await?;
                    Ok::<Vec<VariableBinding>, snmp2::Error>(Self::extract_bindings(pdu))
                }
            })
            .await;
            rs.retries = rs.retries.max(retries);

            match result {
                Ok(bindings) => rs.bindings.extend(bindings),
                Err(e) => {
                    warn!(
                        "GetNext failed for {} after {} retries: {:?}",
                        oid_str, retries, e
                    );
                    rs.partial = true;
                    rs.warnings
                        .push(error_to_warning(&e, Some(oid_str.clone())));
                }
            }
        }

        Ok(rs)
    }

    async fn do_walk(
        &self,
        target: &Target,
        root_oid: &str,
        callback: Option<Arc<WalkBatchCallback>>,
    ) -> Result<ResultSet, String> {
        let mut session = Self::connect(target).await?;
        let mut rs = ResultSet::new();
        let mut batch = Vec::new();

        let root: snmp2::Oid = root_oid
            .parse()
            .map_err(|e| format!("Invalid root OID: {:?}", e))?;
        let mut current_oid = root;

        loop {
            match session.getnext(&current_oid).await {
                Ok(pdu) => {
                    let mut got_data = false;
                    for (o, v) in pdu.varbinds {
                        if is_walk_termination_value(&v) {
                            info!("Walk terminated: {:?}", v);
                            Self::drain_batch(&mut batch, callback.as_deref(), &mut rs);
                            return Ok(rs);
                        }

                        let oid_str = o.to_string();
                        if !Self::is_subtree_of(&oid_str, root_oid) {
                            info!(
                                "Walk passed root {} (got {}), terminating",
                                root_oid, oid_str
                            );
                            Self::drain_batch(&mut batch, callback.as_deref(), &mut rs);
                            return Ok(rs);
                        }

                        batch.push(binding_from_snmp(oid_str, v));
                        got_data = true;
                    }

                    if !got_data {
                        break;
                    }

                    // Update current_oid from the last binding.
                    if let Some(last) = batch.last() {
                        match snmp2::Oid::from_str(&last.oid) {
                            Ok(new_oid) => current_oid = new_oid,
                            Err(_) => break,
                        }
                    } else {
                        break;
                    }

                    // Drain batch if full.
                    if batch.len() >= WALK_BATCH_SIZE {
                        Self::drain_batch(&mut batch, callback.as_deref(), &mut rs);
                    }
                }
                Err(e) => {
                    warn!("Walk getnext failed: {:?}", e);
                    rs.partial = true;
                    rs.warnings
                        .push(error_to_warning(&e, Some(root_oid.to_string())));

                    // Retry with reconnection for network errors.
                    if is_retryable_error(&e) {
                        match Self::connect(target).await {
                            Ok(new_session) => session = new_session,
                            Err(conn_err) => {
                                warn!("Reconnection failed: {}", conn_err);
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        Self::drain_batch(&mut batch, callback.as_deref(), &mut rs);
        Ok(rs)
    }

    async fn do_bulk_walk(
        &self,
        target: &Target,
        root_oid: &str,
        callback: Option<Arc<WalkBatchCallback>>,
    ) -> Result<ResultSet, String> {
        if matches!(target.version, Version::V1) {
            return Err("BulkWalk is not supported in SNMPv1 — use Walk instead".to_string());
        }

        let mut session = Self::connect(target).await?;
        let mut rs = ResultSet::new();
        let mut batch = Vec::new();

        let root: snmp2::Oid = root_oid
            .parse()
            .map_err(|e| format!("Invalid root OID: {:?}", e))?;
        let mut current_oid = root;

        const MAX_REPETITIONS: u32 = 50;

        loop {
            let ref_oids: Vec<&snmp2::Oid> = vec![&current_oid];

            match session.getbulk(&ref_oids, 0, MAX_REPETITIONS).await {
                Ok(pdu) => {
                    let mut got_data = false;
                    for (o, v) in pdu.varbinds {
                        if is_walk_termination_value(&v) {
                            info!("BulkWalk terminated: {:?}", v);
                            Self::drain_batch(&mut batch, callback.as_deref(), &mut rs);
                            return Ok(rs);
                        }

                        let oid_str = o.to_string();
                        if !Self::is_subtree_of(&oid_str, root_oid) {
                            info!(
                                "BulkWalk passed root {} (got {}), terminating",
                                root_oid, oid_str
                            );
                            Self::drain_batch(&mut batch, callback.as_deref(), &mut rs);
                            return Ok(rs);
                        }

                        batch.push(binding_from_snmp(oid_str, v));
                        got_data = true;
                    }

                    if !got_data {
                        break;
                    }

                    // Update current_oid from the last binding.
                    if let Some(last) = batch.last() {
                        match snmp2::Oid::from_str(&last.oid) {
                            Ok(new_oid) => current_oid = new_oid,
                            Err(_) => break,
                        }
                    } else {
                        break;
                    }

                    // Drain batch if full.
                    if batch.len() >= WALK_BATCH_SIZE {
                        Self::drain_batch(&mut batch, callback.as_deref(), &mut rs);
                    }
                }
                Err(e) => {
                    warn!("BulkWalk getbulk failed: {:?}", e);
                    rs.partial = true;
                    rs.warnings
                        .push(error_to_warning(&e, Some(root_oid.to_string())));

                    if is_retryable_error(&e) {
                        match Self::connect(target).await {
                            Ok(new_session) => session = new_session,
                            Err(conn_err) => {
                                warn!("Reconnection failed: {}", conn_err);
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        Self::drain_batch(&mut batch, callback.as_deref(), &mut rs);
        Ok(rs)
    }

    async fn do_set(
        &self,
        target: &Target,
        oid_str: &str,
        value: SetValue,
    ) -> Result<ResultSet, String> {
        let target = target.clone();
        let oid = oid_str.to_string();
        let value_owned = value;

        // Parse OID once.
        let parsed_oid: Arc<snmp2::Oid<'static>> = Arc::new(
            oid.parse()
                .map_err(|e| format!("Invalid OID '{}': {:?}", &oid, e))?,
        );

        // Retry: each attempt creates a fresh session and value.
        let (result, retries) = with_retry(|| {
            let t = target.clone();
            let o = Arc::clone(&parsed_oid);
            let v = value_owned.clone();
            async move {
                let mut session = Self::connect(&t).await.map_err(|_| snmp2::Error::Receive)?;
                let snmp_value = Self::set_value_to_snmp(v);
                let pdu = session.set(&[(&o, snmp_value)]).await?;
                Ok::<Vec<VariableBinding>, snmp2::Error>(Self::extract_bindings(pdu))
            }
        })
        .await;

        match result {
            Ok(bindings) => {
                let mut rs = ResultSet::new();
                rs.retries = retries;
                rs.bindings = bindings;
                Ok(rs)
            }
            Err(e) => {
                warn!("Set failed after {} retries: {:?}", retries, e);
                let mut rs = ResultSet::new();
                rs.retries = retries;
                rs.partial = true;
                rs.warnings
                    .push(error_to_warning(&e, Some(oid_str.to_string())));
                Ok(rs)
            }
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Establishes a connection to the Target and returns an AsyncSession.
    async fn connect(target: &Target) -> Result<snmp2::AsyncSession, String> {
        let addr: SocketAddr = target
            .addr()
            .parse()
            .map_err(|e| format!("Invalid Target address {}: {}", target.addr(), e))?;

        info!(
            "Connecting to Target {} (version={:?})",
            target.addr(),
            target.version,
        );

        match target.version {
            Version::V1 => snmp2::AsyncSession::new_v1(addr, target.community.as_bytes(), 0)
                .await
                .map_err(|e| format!("Failed to connect (v1) to {}: {}", target.addr(), e)),
            Version::V2c => snmp2::AsyncSession::new_v2c(addr, target.community.as_bytes(), 0)
                .await
                .map_err(|e| format!("Failed to connect (v2c) to {}: {}", target.addr(), e)),
            Version::V3 => {
                #[cfg(feature = "snmp2/v3")]
                {
                    let security = target
                        .security
                        .as_ref()
                        .ok_or_else(|| "SNMPv3 requires security configuration".to_string())?;

                    let mut sec = snmp2::v3::Security::default();
                    sec.username = security.username.clone().into_bytes();

                    if !security.auth_passphrase.is_empty() {
                        sec.authentication_password = security.auth_passphrase.clone().into_bytes();
                    }
                    if !security.priv_passphrase.is_empty() {
                        sec.privacy_password = security.priv_passphrase.clone().into_bytes();
                    }

                    snmp2::AsyncSession::new_v3(addr, 0, sec)
                        .await
                        .map_err(|e| format!("Failed to connect (v3) to {}: {}", target.addr(), e))
                }
                #[cfg(not(feature = "snmp2/v3"))]
                {
                    Err("SNMPv3 support requires the 'v3' feature of snmp2".to_string())
                }
            }
        }
    }

    /// Checks if `oid` is within the subtree rooted at `root`.
    fn is_subtree_of(oid: &str, root: &str) -> bool {
        oid == root || oid.starts_with(&format!("{}.", root))
    }

    /// Drains a batch — either invokes callback or appends to result set.
    fn drain_batch(
        batch: &mut Vec<VariableBinding>,
        callback: Option<&WalkBatchCallback>,
        rs: &mut ResultSet,
    ) {
        let items = std::mem::take(batch);
        if let Some(cb) = callback {
            cb(items);
        } else {
            rs.bindings.extend(items);
        }
    }

    /// Converts our SetValue enum to a snmp2 Value.
    fn set_value_to_snmp(value: SetValue) -> snmp2::Value<'static> {
        match value {
            SetValue::Integer(v) => snmp2::Value::Integer(v),
            SetValue::OctetString(bytes) => {
                snmp2::Value::OctetString(Box::leak(bytes.into_boxed_slice()))
            }
            SetValue::Unsigned32(v) => snmp2::Value::Unsigned32(v),
            SetValue::Counter32(v) => snmp2::Value::Counter32(v),
            SetValue::Counter64(v) => snmp2::Value::Counter64(v),
            SetValue::IpAddress(ip) => {
                let parts: Vec<u8> = ip.split('.').filter_map(|p| p.parse().ok()).collect();
                if parts.len() == 4 {
                    snmp2::Value::IpAddress([parts[0], parts[1], parts[2], parts[3]])
                } else {
                    snmp2::Value::OctetString(Box::leak(ip.into_bytes().into_boxed_slice()))
                }
            }
            SetValue::TimeTicks(v) => snmp2::Value::Timeticks(v),
            SetValue::ObjectIdentifier(oid_str) => snmp2::Value::ObjectIdentifier(
                snmp2::Oid::from_str(&oid_str)
                    .unwrap_or_else(|_| snmp2::Oid::from(&[0u64]).unwrap()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_subtree_of_exact_match() {
        assert!(SnmpEngine::is_subtree_of("1.3.6.1.2.1", "1.3.6.1.2.1"));
    }

    #[test]
    fn is_subtree_of_child() {
        assert!(SnmpEngine::is_subtree_of("1.3.6.1.2.1.1.1", "1.3.6.1.2.1"));
    }

    #[test]
    fn is_subtree_of_not_sibling() {
        assert!(!SnmpEngine::is_subtree_of("1.3.6.1.2.2", "1.3.6.1.2.1"));
    }

    #[test]
    fn is_subtree_of_different_tree() {
        assert!(!SnmpEngine::is_subtree_of("1.3.6.2.1", "1.3.6.1.2.1"));
    }

    #[test]
    fn target_v2c_default() {
        let t = Target::v2c("192.168.1.1", 161, "public");
        assert_eq!(t.host, "192.168.1.1");
        assert_eq!(t.port, 161);
        assert_eq!(t.version, Version::V2c);
        assert_eq!(t.community, "public");
    }

    #[test]
    fn target_v3_with_security() {
        let sec = SnmpV3SecurityConfig {
            username: "admin".to_string(),
            auth_protocol: AuthProtocol::Sha1,
            auth_passphrase: "authpass".to_string(),
            priv_protocol: PrivProtocol::Aes128,
            priv_passphrase: "privpass".to_string(),
        };
        let t = Target::v3("10.0.0.1", 161, sec);
        assert_eq!(t.version, Version::V3);
        assert!(t.community.is_empty());
        assert!(t.security.is_some());
    }

    #[test]
    fn target_addr_format() {
        let t = Target::v2c("example.com", 1161, "private");
        assert_eq!(t.addr(), "example.com:1161");
    }

    #[test]
    fn snmp_operation_serialization() {
        let op = SnmpOperation::Walk;
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(json, "\"walk\"");
    }

    #[test]
    fn version_serialization() {
        let v = Version::V2c;
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"v2c\"");
    }

    #[test]
    fn engine_new_succeeds() {
        let engine = SnmpEngine::new().expect("engine should create");
        drop(engine);
    }
}
