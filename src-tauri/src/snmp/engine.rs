use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tracing::{info, warn};

use super::table::*;
use super::tolerant::*;
use super::types::*;

/// Batch size for walk streaming.

/// Walk mode — determines which SNMP fetch operation is used.
enum WalkMode {
    GetNext,
    GetBulk,
}

impl WalkMode {
    fn label(&self) -> &'static str {
        match self {
            WalkMode::GetNext => "Walk",
            WalkMode::GetBulk => "BulkWalk",
        }
    }
}

/// Core SNMP engine that executes operations against a Target with error tolerance.
#[derive(Clone)]
pub struct SnmpEngine {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl SnmpEngine {
    /// Creates a new engine backed by a dedicated multi-threaded tokio runtime.
    pub fn new() -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_stack_size(4 * 1024 * 1024) // 4 MB for deep JSON serialization during large walks
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    /// Executes a Get operation for the given OIDs against the Target.
    pub fn get(&self, target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        self.runtime.block_on(Self::do_get(target, oids))
    }

    /// Executes a GetNext operation for the given OIDs against the Target.
    pub fn get_next(&self, target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        self.runtime.block_on(Self::do_get_next(target, oids))
    }

    /// Blocking helper for table walks (walks all columns then assembles grid).
    fn bulk_walk_blocking(&self, target: &Target, root_oid: &str) -> Result<ResultSet, String> {
        self.runtime.block_on(Self::do_walk_loop(
            WalkMode::GetBulk,
            target,
            root_oid,
            None,
            None,
        ))
    }

    /// Executes a streaming Walk operation with cancellation support. Returns the JoinHandle for aborting.
    pub fn walk_streaming(
        &self,
        target: &Target,
        root_oid: &str,
        batch_channel: Option<tauri::ipc::Channel>,
        complete_channel: Option<tauri::ipc::Channel>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> Result<tokio::task::JoinHandle<()>, String> {
        let mode = WalkMode::GetNext;
        let target = target.clone();
        let root_oid = root_oid.to_string();
        let batch_ch = batch_channel;
        let complete_ch = complete_channel;
        let cancel = cancel_token.map(|a| a.clone());

        let handle = self.runtime.spawn(async move {
            let result = Self::do_walk_loop(
                mode,
                &target,
                &root_oid,
                batch_ch.as_ref(),
                cancel.as_deref(),
            )
            .await;
            match result {
                Ok(rs) => {
                    if let Some(ch) = complete_ch {
                        let _ = ch.send(serde_json::to_string(&rs).unwrap_or_default().into());
                    }
                }
                Err(e) => {
                    warn!("Walk streaming error: {}", e);
                    if let Some(ch) = complete_ch {
                        let rs = ResultSet {
                            bindings: Vec::new(),
                            partial: true,
                            warnings: vec![SnmpWarning {
                                kind: "error".to_string(),
                                message: e.clone(),
                                oid: None,
                            }],
                            retries: 0,
                        };
                        let _ = ch.send(serde_json::to_string(&rs).unwrap_or_default().into());
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Executes a streaming BulkWalk operation with cancellation support. Returns the JoinHandle for aborting.
    pub fn bulk_walk_streaming(
        &self,
        target: &Target,
        root_oid: &str,
        batch_channel: Option<tauri::ipc::Channel>,
        complete_channel: Option<tauri::ipc::Channel>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> Result<tokio::task::JoinHandle<()>, String> {
        let mode = WalkMode::GetBulk;
        let target = target.clone();
        let root_oid = root_oid.to_string();
        let batch_ch = batch_channel;
        let complete_ch = complete_channel;
        let cancel = cancel_token.map(|a| a.clone());

        let handle = self.runtime.spawn(async move {
            let result = Self::do_walk_loop(
                mode,
                &target,
                &root_oid,
                batch_ch.as_ref(),
                cancel.as_deref(),
            )
            .await;
            match result {
                Ok(rs) => {
                    if let Some(ch) = complete_ch {
                        let _ = ch.send(serde_json::to_string(&rs).unwrap_or_default().into());
                    }
                }
                Err(e) => {
                    warn!("BulkWalk streaming error: {}", e);
                    if let Some(ch) = complete_ch {
                        let rs = ResultSet {
                            bindings: Vec::new(),
                            partial: true,
                            warnings: vec![SnmpWarning {
                                kind: "error".to_string(),
                                message: e.clone(),
                                oid: None,
                            }],
                            retries: 0,
                        };
                        let _ = ch.send(serde_json::to_string(&rs).unwrap_or_default().into());
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Executes a Set operation to write a value at the given OID.
    pub fn set(&self, target: &Target, oid: &str, value: SetValue) -> Result<ResultSet, String> {
        self.runtime.block_on(self.do_set(target, oid, value))
    }

    /// Walks all columns of a table and assembles results into a grid.
    ///
    /// Takes the table's root OID and column OIDs, BulkWalks each column,
    /// then merges results by instance suffix into rows. Returns a [`TableResult`]
    /// with best-effort merge for inconsistent data.
    pub fn walk_table(
        &self,
        target: &Target,
        table_oid: &str,
        column_oids: &[String],
    ) -> Result<TableResult, String> {
        info!(
            "WalkTable started on {} for {} ({} columns)",
            target.addr(),
            table_oid,
            column_oids.len()
        );
        if column_oids.is_empty() {
            return Ok(TableResult {
                table_oid: table_oid.to_string(),
                columns: Vec::new(),
                rows: Vec::new(),
                total_rows: 0,
                missing_cells: 0,
                warnings: Vec::new(),
                partial: false,
            });
        }

        let mut column_results: HashMap<String, Vec<VariableBinding>> = HashMap::new();
        let mut all_warnings: Vec<SnmpWarning> = Vec::new();
        let mut any_partial = false;

        for col_oid in column_oids {
            match self.bulk_walk_blocking(target, col_oid) {
                Ok(rs) => {
                    column_results.insert(col_oid.clone(), rs.bindings);
                    all_warnings.extend(rs.warnings);
                    any_partial |= rs.partial;
                }
                Err(e) => {
                    warn!("Table walk failed for column {}: {}", col_oid, e);
                    all_warnings.push(SnmpWarning {
                        kind: "column-walk-error".to_string(),
                        message: format!("Failed to walk column {}: {}", col_oid, e),
                        oid: Some(col_oid.clone()),
                    });
                    any_partial = true;
                }
            }
        }

        let mut grid = assemble_table_grid(table_oid.to_string(), column_results);
        grid.warnings.extend(all_warnings);
        grid.partial = any_partial;
        info!(
            "WalkTable completed on {}: {} rows, {} missing cells",
            target.addr(),
            grid.total_rows,
            grid.missing_cells
        );
        Ok(grid)
    }

    // ── Async implementations ────────────────────────────────────────────────

    /// Extracts owned VariableBindings from a Pdu (consumes the iterator).
    fn extract_bindings(pdu: snmp2::Pdu<'_>) -> Vec<VariableBinding> {
        pdu.varbinds
            .map(|(o, v)| binding_from_snmp(o.to_string(), v))
            .collect()
    }

    /// Async Get operation — public so it can be called directly from Tauri's runtime.
    pub async fn do_get(target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        info!("Get started on {} for {} OID(s)", target.addr(), oids.len());
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

        // Inline retry loop to avoid large generic future types.
        let mut last_err = None;
        for attempt in 0..=MAX_RETRIES {
            let t = target.clone();
            let oids: Vec<Arc<snmp2::Oid<'static>>> = snmp_oids.clone();
            let result: Result<Vec<VariableBinding>, snmp2::Error> = (async {
                let mut session = Self::connect(&t).await.map_err(|_| snmp2::Error::Receive)?;

                let pdu = if oids.len() == 1 {
                    session.get(oids[0].as_ref()).await?
                } else {
                    let refs: Vec<&snmp2::Oid> = oids.iter().map(|a| a.as_ref()).collect();
                    session.get_many(&refs).await?
                };

                Ok(Self::extract_bindings(pdu))
            })
            .await;

            match result {
                Ok(bindings) => {
                    info!(
                        "Get completed on {}: {} binding(s)",
                        target.addr(),
                        bindings.len()
                    );
                    let mut rs = ResultSet::new();
                    rs.retries = attempt;
                    rs.bindings = bindings;
                    return Ok(rs);
                }
                Err(e) => {
                    if is_retryable_error(&e) && attempt < MAX_RETRIES {
                        let delay = backoff_delay(attempt);
                        warn!(
                            "Get network error on attempt {}/{} — retrying in {:?}",
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

        let e = last_err.unwrap_or(snmp2::Error::Receive);
        warn!("Get failed after {} retries: {:?}", MAX_RETRIES, e);
        Err(format!("{:?}", e))
    }

    async fn do_get_next(target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        info!(
            "GetNext started on {} for {} OID(s)",
            target.addr(),
            oids.len()
        );
        let mut rs = ResultSet::new();

        for oid_str in oids {
            let target = target.clone();
            let oid = oid_str.clone();

            // Inline retry loop.
            let mut last_err = None;
            for attempt in 0..=MAX_RETRIES {
                let t = target.clone();
                let o = oid.clone();
                let result: Result<Vec<VariableBinding>, snmp2::Error> = (async {
                    let mut session = Self::connect(&t).await.map_err(|_| snmp2::Error::Receive)?;
                    let parsed: snmp2::Oid = o.parse().map_err(|_| snmp2::Error::AsnParse)?;
                    let pdu = session.getnext(&parsed).await?;
                    Ok(Self::extract_bindings(pdu))
                })
                .await;

                match result {
                    Ok(bindings) => {
                        rs.retries = rs.retries.max(attempt);
                        rs.bindings.extend(bindings);
                        break;
                    }
                    Err(e) => {
                        if is_retryable_error(&e) && attempt < MAX_RETRIES {
                            let delay = backoff_delay(attempt);
                            warn!(
                                "GetNext network error on attempt {}/{} — retrying in {:?}",
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

            if let Some(e) = last_err {
                warn!(
                    "GetNext failed for {} after {} retries: {:?}",
                    oid_str, MAX_RETRIES, e
                );
                rs.partial = true;
                rs.warnings
                    .push(error_to_warning(&e, Some(oid_str.clone())));
            }
        }

        info!(
            "GetNext completed on {}: {} binding(s)",
            target.addr(),
            rs.bindings.len()
        );
        Ok(rs)
    }

    /// Shared walk loop used by both Walk and BulkWalk.
    async fn do_walk_loop(
        mode: WalkMode,
        target: &Target,
        root_oid: &str,
        channel: Option<&tauri::ipc::Channel>,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<ResultSet, String> {
        let op_name = mode.label();
        info!("{} started on {} from {}", op_name, target.addr(), root_oid);
        let mut session = Self::connect(target).await?;
        let mut rs = ResultSet::new();

        let normalized = if !root_oid.contains('.') {
            format!("{}.0", root_oid)
        } else {
            root_oid.to_string()
        };
        let root: snmp2::Oid = normalized
            .parse()
            .map_err(|e| format!("Invalid root OID: {:?}", e))?;
        let mut current_oid = root;
        let mut retry_count: u32 = 0;

        loop {
            if cancel_token.map_or(false, |t| t.load(Ordering::Acquire)) {
                info!("{} cancelled by user", op_name);
                rs.partial = true;
                return Ok(rs);
            }

            let pdu_result = match mode {
                WalkMode::GetNext => session.getnext(&current_oid).await,
                WalkMode::GetBulk => session.getbulk(&[&current_oid], 0, 50).await,
            };

            match pdu_result {
                Ok(pdu) => {
                    let mut received_varbinds = false;
                    for (o, v) in pdu.varbinds {
                        if is_walk_termination_value(&v) {
                            info!("{} terminated: {:?}", op_name, v);
                            return Ok(rs);
                        }

                        let oid_str = o.to_string();
                        if !Self::is_subtree_of(&oid_str, root_oid) {
                            info!(
                                "{} passed root {} (got {}), terminating",
                                op_name, root_oid, oid_str
                            );
                            return Ok(rs);
                        }

                        let binding = binding_from_snmp(oid_str.clone(), v);
                        if let Some(ch) = channel {
                            match serde_json::to_string(&binding) {
                                Ok(json) => {
                                    if let Err(e) = ch.send(json.into()) {
                                        warn!("{} channel send failed: {:?}", op_name, e);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("{} serialization failed: {:?}", op_name, e);
                                    break;
                                }
                            }
                        } else {
                            rs.bindings.push(binding);
                        }
                        received_varbinds = true;

                        match snmp2::Oid::from_str(&oid_str) {
                            Ok(new_oid) => current_oid = new_oid,
                            Err(_) => break,
                        }
                    }

                    if !received_varbinds {
                        break;
                    }
                }
                Err(e) => {
                    warn!("{} fetch failed: {:?}", op_name, e);
                    rs.partial = true;
                    rs.warnings
                        .push(error_to_warning(&e, Some(root_oid.to_string())));

                    if is_retryable_error(&e) && retry_count < MAX_RETRIES {
                        let delay = backoff_delay(retry_count);
                        warn!("{} network error — retrying in {:?}", op_name, delay);
                        tokio::time::sleep(delay).await;

                        if cancel_token.map_or(false, |t| t.load(Ordering::Acquire)) {
                            info!("{} cancelled by user during retry", op_name);
                            rs.partial = true;
                            return Ok(rs);
                        }
                        retry_count += 1;

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

        rs.retries = retry_count;
        info!(
            "{} completed on {}: {} binding(s)",
            op_name,
            target.addr(),
            rs.bindings.len()
        );
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
        info!("Set started on {} for {}", target.addr(), oid_str);

        // Parse OID once.
        let parsed_oid: Arc<snmp2::Oid<'static>> = Arc::new(
            oid.parse()
                .map_err(|e| format!("Invalid OID '{}': {:?}", &oid, e))?,
        );

        // Inline retry loop.
        let mut last_err = None;
        for attempt in 0..=MAX_RETRIES {
            let t = target.clone();
            let o = Arc::clone(&parsed_oid);
            let v = value_owned.clone();
            let result: Result<Vec<VariableBinding>, snmp2::Error> = (async {
                let mut session = Self::connect(&t).await.map_err(|_| snmp2::Error::Receive)?;
                let snmp_value = Self::set_value_to_snmp(v);
                let pdu = session.set(&[(&o, snmp_value)]).await?;
                Ok(Self::extract_bindings(pdu))
            })
            .await;

            match result {
                Ok(bindings) => {
                    info!(
                        "Set completed on {}: {} binding(s)",
                        target.addr(),
                        bindings.len()
                    );
                    let mut rs = ResultSet::new();
                    rs.retries = attempt;
                    rs.bindings = bindings;
                    return Ok(rs);
                }
                Err(e) => {
                    if is_retryable_error(&e) && attempt < MAX_RETRIES {
                        let delay = backoff_delay(attempt);
                        warn!(
                            "Set network error on attempt {}/{} — retrying in {:?}",
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

        let e = last_err.unwrap_or(snmp2::Error::Receive);
        warn!("Set failed after {} retries: {:?}", MAX_RETRIES, e);
        let mut rs = ResultSet::new();
        rs.retries = MAX_RETRIES;
        rs.partial = true;
        rs.warnings
            .push(error_to_warning(&e, Some(oid_str.to_string())));
        Ok(rs)
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
                let security = target
                    .security
                    .as_ref()
                    .ok_or_else(|| "SNMPv3 requires security configuration".to_string())?;

                let sec = snmp2::v3::Security::new(
                    security.username.as_bytes(),
                    security.auth_passphrase.as_bytes(),
                );

                snmp2::AsyncSession::new_v3(addr, 0, sec)
                    .await
                    .map_err(|e| format!("Failed to connect (v3) to {}: {}", target.addr(), e))
            }
        }
    }

    /// Checks if `oid` is within the subtree rooted at `root`.
    fn is_subtree_of(oid: &str, root: &str) -> bool {
        oid == root || oid.starts_with(&format!("{}.", root))
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
