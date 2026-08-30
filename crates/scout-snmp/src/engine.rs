use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tracing::{info, warn};

use super::table::*;
use super::tolerant::*;
use super::types::*;

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

/// Receives streamed walk results from the engine.
///
/// Implemented by the app crate to bridge to Tauri IPC channels. The pure
/// crate never sees serialized JSON — the adapter owns serialization.
pub trait WalkBatchSender: Send + Sync {
    /// Sends one binding to the client. Returns `false` if the client is gone
    /// (channel closed or serialization failed), in which case the engine stops walking.
    fn send_binding(&self, binding: &VariableBinding) -> bool;

    /// Sends the final result set (success or error summary).
    fn send_complete(&self, result: &ResultSet);
}

/// Receives progress and the final grid from a streaming table retrieval.
///
/// Implemented by the app crate to bridge to Tauri IPC channels, exactly like
/// [`WalkBatchSender`].
pub trait TableRowSender: Send + Sync {
    /// Sends a progress update with the current binding count. Returns `false`
    /// if the client is gone (channel closed or serialization failed), in which
    /// case the engine stops walking.
    fn send_progress(&self, count: usize) -> bool;

    /// Sends the final pivoted grid (success or error summary).
    fn send_complete(&self, result: &TableResult);
}

/// How the walk loop consumes each binding it receives.
enum WalkSink<'a> {
    /// Stream bindings to a client without retaining them (Walk/BulkWalk).
    Stream(&'a dyn WalkBatchSender),
    /// Retain bindings in memory and report progress periodically (Get Table —
    /// rows only complete once the whole column-major walk has arrived).
    Collect {
        bindings: &'a mut Vec<VariableBinding>,
        progress: &'a mut (dyn FnMut(usize) -> bool + Send + 'a),
    },
}

/// Progress is reported every this many collected bindings.
const TABLE_PROGRESS_STEP: usize = 10;

/// Core SNMP engine that executes operations against a Target with error tolerance.
///
/// The engine is stateless; the tokio runtime it runs on is owned by the app crate.
#[derive(Clone, Default)]
pub struct SnmpEngine;

impl SnmpEngine {
    /// Creates a new engine.
    pub fn new() -> Self {
        Self
    }

    /// Executes a Get operation for the given OIDs against the Target.
    pub async fn get(&self, target: &Target, oids: &[String]) -> Result<ResultSet, String> {
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
            let result: Result<(Vec<VariableBinding>, Vec<SnmpWarning>), snmp2::Error> = (async {
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
                Ok((bindings, warnings)) => {
                    info!(
                        "Get completed on {}: {} binding(s)",
                        target.addr(),
                        bindings.len()
                    );
                    let mut rs = ResultSet::new();
                    rs.retries = attempt;
                    rs.bindings = bindings;
                    if !warnings.is_empty() {
                        rs.partial = true;
                    }
                    rs.warnings = warnings;
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

    /// Executes a GetNext operation for the given OIDs against the Target.
    pub async fn get_next(&self, target: &Target, oids: &[String]) -> Result<ResultSet, String> {
        info!(
            "GetNext started on {} for {} OID(s)",
            target.addr(),
            oids.len()
        );

        // Parse OIDs once outside the retry loop. A bare top-level arc (e.g.
        // "1" for iso) is normalized to "{oid}.0" — a single subidentifier is
        // not BER-encodable. Anything still unparseable is a client-side
        // error: no request is ever sent, so it must not be reported as an
        // ASN.1 response failure.
        let parsed_oids: Vec<(String, Arc<snmp2::Oid<'static>>)> = oids
            .iter()
            .map(|o| -> Result<(String, Arc<snmp2::Oid<'static>>), String> {
                let normalized = Self::normalize_bare_oid(o);
                let parsed: snmp2::Oid = normalized
                    .parse()
                    .map_err(|e| format!("Invalid OID '{}': {:?}", o, e))?;
                Ok((o.clone(), Arc::new(parsed)))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut rs = ResultSet::new();

        for (oid_str, parsed) in parsed_oids {
            let target = target.clone();

            // Inline retry loop.
            let mut last_err = None;
            for attempt in 0..=MAX_RETRIES {
                let t = target.clone();
                let o = Arc::clone(&parsed);
                let result: Result<(Vec<VariableBinding>, Vec<SnmpWarning>), snmp2::Error> =
                    (async {
                        let mut session =
                            Self::connect(&t).await.map_err(|_| snmp2::Error::Receive)?;
                        let pdu = session.getnext(o.as_ref()).await?;
                        Ok(Self::extract_bindings(pdu))
                    })
                    .await;

                match result {
                    Ok((bindings, warnings)) => {
                        rs.retries = rs.retries.max(attempt);
                        rs.bindings.extend(bindings);
                        if !warnings.is_empty() {
                            rs.partial = true;
                            rs.warnings.extend(warnings);
                        }
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

    /// Executes a streaming Walk operation with cancellation support. Returns the JoinHandle for aborting.
    pub fn walk_streaming(
        &self,
        runtime: &tokio::runtime::Handle,
        target: &Target,
        root_oid: &str,
        sender: Arc<dyn WalkBatchSender>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> tokio::task::JoinHandle<()> {
        self.spawn_walk(
            runtime,
            WalkMode::GetNext,
            target,
            root_oid,
            sender,
            cancel_token,
        )
    }

    /// Executes a streaming BulkWalk operation with cancellation support. Returns the JoinHandle for aborting.
    pub fn bulk_walk_streaming(
        &self,
        runtime: &tokio::runtime::Handle,
        target: &Target,
        root_oid: &str,
        sender: Arc<dyn WalkBatchSender>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> tokio::task::JoinHandle<()> {
        self.spawn_walk(
            runtime,
            WalkMode::GetBulk,
            target,
            root_oid,
            sender,
            cancel_token,
        )
    }

    /// Spawns a streaming walk of the given mode onto the provided runtime.
    fn spawn_walk(
        &self,
        runtime: &tokio::runtime::Handle,
        mode: WalkMode,
        target: &Target,
        root_oid: &str,
        sender: Arc<dyn WalkBatchSender>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> tokio::task::JoinHandle<()> {
        let op_name = mode.label();
        let target = target.clone();
        let root_oid = root_oid.to_string();

        runtime.spawn(async move {
            let mut sink = WalkSink::Stream(sender.as_ref());
            let result = Self::do_walk_loop(
                mode,
                &target,
                &root_oid,
                Some(&mut sink),
                cancel_token.as_deref(),
            )
            .await;
            match result {
                Ok(rs) => sender.send_complete(&rs),
                Err(e) => {
                    warn!("{} streaming error: {}", op_name, e);
                    sender.send_complete(&Self::error_result_set(e));
                }
            }
        })
    }

    /// Executes a Set operation to write a value at the given OID.
    pub async fn set(
        &self,
        target: &Target,
        oid: &str,
        value: SetValue,
    ) -> Result<ResultSet, String> {
        let target = target.clone();
        let oid = oid.to_string();
        let value_owned = value;
        info!("Set started on {} for {}", target.addr(), oid);

        // Parse OID once.
        let parsed_oid: Arc<snmp2::Oid<'static>> = Arc::new(
            oid.parse()
                .map_err(|e| format!("Invalid OID '{}': {:?}", oid, e))?,
        );

        // Inline retry loop.
        let mut last_err = None;
        for attempt in 0..=MAX_RETRIES {
            let t = target.clone();
            let o = Arc::clone(&parsed_oid);
            let v = value_owned.clone();
            let result: Result<(Vec<VariableBinding>, Vec<SnmpWarning>), snmp2::Error> = (async {
                let mut session = Self::connect(&t).await.map_err(|_| snmp2::Error::Receive)?;
                let snmp_value = Self::set_value_to_snmp(v);
                let pdu = session.set(&[(&o, snmp_value)]).await?;
                Ok(Self::extract_bindings(pdu))
            })
            .await;

            match result {
                Ok((bindings, warnings)) => {
                    info!(
                        "Set completed on {}: {} binding(s)",
                        target.addr(),
                        bindings.len()
                    );
                    let mut rs = ResultSet::new();
                    rs.retries = attempt;
                    rs.bindings = bindings;
                    if !warnings.is_empty() {
                        rs.partial = true;
                    }
                    rs.warnings = warnings;
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
        rs.warnings.push(error_to_warning(&e, Some(oid)));
        Ok(rs)
    }

    /// Executes a streaming Get Table operation with cancellation support.
    /// Returns the JoinHandle for aborting.
    ///
    /// One connection; one BulkWalk of the whole table subtree; bindings are
    /// pivoted onto rows only when they belong to a requested column (nested
    /// sub-table data is excluded). `column_oids` is the *display* selection —
    /// the walk fetches the entire subtree regardless.
    pub fn get_table_streaming(
        &self,
        runtime: &tokio::runtime::Handle,
        target: &Target,
        table_oid: &str,
        column_oids: Vec<String>,
        index_columns: Vec<IndexColumnSpec>,
        sender: Arc<dyn TableRowSender>,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> tokio::task::JoinHandle<()> {
        let target = target.clone();
        let table_oid = table_oid.to_string();
        runtime.spawn(async move {
            let result = Self::do_get_table(
                &target,
                &table_oid,
                &column_oids,
                &index_columns,
                sender.as_ref(),
                cancel_token.as_deref(),
            )
            .await;
            match result {
                Ok(grid) => sender.send_complete(&grid),
                Err(e) => {
                    warn!("GetTable streaming error: {}", e);
                    sender.send_complete(&Self::error_table_result(&table_oid, column_oids, e));
                }
            }
        })
    }

    /// Single-pass table retrieval: one BulkWalk of the table subtree, then
    /// pivot the collected bindings into a grid.
    async fn do_get_table(
        target: &Target,
        table_oid: &str,
        column_oids: &[String],
        index_columns: &[IndexColumnSpec],
        sender: &dyn TableRowSender,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<TableResult, String> {
        info!(
            "GetTable started on {} for {} ({} display columns)",
            target.addr(),
            table_oid,
            column_oids.len()
        );

        let mut bindings: Vec<VariableBinding> = Vec::new();
        let mut progress = |count: usize| sender.send_progress(count);
        let mut sink = WalkSink::Collect {
            bindings: &mut bindings,
            progress: &mut progress,
        };

        // One connection, one walk of the whole subtree (column-major order).
        let rs = Self::do_walk_loop(
            WalkMode::GetBulk,
            target,
            table_oid,
            Some(&mut sink),
            cancel_token,
        )
        .await?;

        let mut grid = assemble_table_walk(
            table_oid.to_string(),
            column_oids.to_vec(),
            bindings,
            index_columns,
        );
        grid.warnings.extend(rs.warnings);
        grid.partial = rs.partial || !grid.warnings.is_empty();
        info!(
            "GetTable completed on {}: {} rows, {} missing cells",
            target.addr(),
            grid.total_rows,
            grid.missing_cells
        );
        Ok(grid)
    }

    /// Builds a partial table result carrying a single error warning.
    fn error_table_result(table_oid: &str, column_oids: Vec<String>, error: String) -> TableResult {
        TableResult {
            table_oid: table_oid.to_string(),
            columns: column_oids,
            rows: Vec::new(),
            total_rows: 0,
            missing_cells: 0,
            warnings: vec![SnmpWarning {
                kind: "error".to_string(),
                message: error,
                oid: None,
            }],
            partial: true,
        }
    }

    // ── Async implementations ────────────────────────────────────────────────

    /// Extracts owned VariableBindings and exception warnings from a Pdu
    /// (consumes the iterator).
    fn extract_bindings(pdu: snmp2::Pdu<'_>) -> (Vec<VariableBinding>, Vec<SnmpWarning>) {
        let varbinds: Vec<(String, snmp2::Value)> =
            pdu.varbinds.map(|(o, v)| (o.to_string(), v)).collect();
        let warnings = varbinds
            .iter()
            .filter_map(|(o, v)| {
                value_warning(v).map(|mut w| {
                    w.oid = Some(o.clone());
                    w
                })
            })
            .collect();
        let bindings = varbinds
            .into_iter()
            .map(|(o, v)| binding_from_snmp(o, v))
            .collect();
        (bindings, warnings)
    }

    /// Shared walk loop used by Walk, BulkWalk, and Get Table.
    async fn do_walk_loop(
        mode: WalkMode,
        target: &Target,
        root_oid: &str,
        sink: Option<&mut WalkSink<'_>>,
        cancel_token: Option<&AtomicBool>,
    ) -> Result<ResultSet, String> {
        let op_name = mode.label();
        info!("{} started on {} from {}", op_name, target.addr(), root_oid);
        let mut session = Self::connect(target).await?;
        let mut rs = ResultSet::new();

        let root: snmp2::Oid = Self::normalize_bare_oid(root_oid)
            .parse()
            .map_err(|e| format!("Invalid root OID '{}': {:?}", root_oid, e))?;
        let mut current_oid = root;
        let mut retry_count: u32 = 0;
        let mut client_gone = false;

        loop {
            if cancel_token.is_some_and(|t| t.load(Ordering::Acquire)) {
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
                        let gone = match sink {
                            Some(WalkSink::Stream(s)) => !s.send_binding(&binding),
                            Some(WalkSink::Collect { bindings, progress }) => {
                                bindings.push(binding);
                                let n = bindings.len();
                                n % TABLE_PROGRESS_STEP == 0 && !progress(n)
                            }
                            None => false,
                        };
                        if gone {
                            warn!("{} client gone, stopping", op_name);
                            client_gone = true;
                            break;
                        }
                        received_varbinds = true;

                        match snmp2::Oid::from_str(&oid_str) {
                            Ok(new_oid) => current_oid = new_oid,
                            Err(_) => break,
                        }
                    }

                    // A client-gone stop must end the whole walk, not just the
                    // current PDU's varbinds.
                    if !received_varbinds || client_gone {
                        if client_gone {
                            rs.partial = true;
                        }
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

                        if cancel_token.is_some_and(|t| t.load(Ordering::Acquire)) {
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

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Builds a partial result set carrying a single error warning.
    fn error_result_set(error: String) -> ResultSet {
        ResultSet {
            bindings: Vec::new(),
            partial: true,
            warnings: vec![SnmpWarning {
                kind: "error".to_string(),
                message: error,
                oid: None,
            }],
            retries: 0,
        }
    }

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

    /// Normalizes a bare top-level arc (no dots, e.g. `"1"` for `iso`) to
    /// `"{oid}.0"`. A single subidentifier is not representable in an ASN.1
    /// OBJECT IDENTIFIER — the first encoded byte carries the first two arcs —
    /// so any SNMP request for such an OID must target its `.0` extension,
    /// the smallest encodable OID at or above it. Walk and GetNext share this.
    fn normalize_bare_oid(oid: &str) -> String {
        if oid.contains('.') {
            oid.to_string()
        } else {
            format!("{oid}.0")
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
    fn version_serialization() {
        let v = Version::V2c;
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"v2c\"");
    }

    #[test]
    fn engine_new_succeeds() {
        let _engine = SnmpEngine::new();
    }
}
