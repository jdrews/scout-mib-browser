//! Engine integration tests against the in-process MockSnmpServer.
//!
//! These run entirely over localhost UDP with no external dependencies:
//! the mock server binds an ephemeral port and speaks SNMPv2c.
//!
//! All engine work is spawned on a multi-threaded runtime with 8MB worker
//! stacks, mirroring the app crate: snmp2's connection code can recurse
//! deeply and overflow default (2MB) thread stacks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use scout_snmp::{
    IndexColumnSpec, IndexEncoding, MockSnmpServer, ResultSet, SetValue, SnmpEngine, SnmpValue,
    TableResult, TableRowSender, Target, VariableBinding, WalkBatchSender,
};

/// Builds a runtime shaped like the app's: 8MB worker stacks.
fn app_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(8 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("failed to build test runtime")
}

/// Collects streamed bindings and the final result set.
#[derive(Default)]
struct TestSender {
    bindings: Mutex<Vec<VariableBinding>>,
    complete: Mutex<Option<ResultSet>>,
}

impl WalkBatchSender for TestSender {
    fn send_binding(&self, binding: &VariableBinding) -> bool {
        self.bindings.lock().unwrap().push(binding.clone());
        true
    }

    fn send_complete(&self, result: &ResultSet) {
        *self.complete.lock().unwrap() = Some(result.clone());
    }
}

/// A sender that requests cancellation as soon as the first binding arrives.
struct CancellingSender {
    inner: Arc<TestSender>,
    cancel: Arc<AtomicBool>,
}

impl WalkBatchSender for CancellingSender {
    fn send_binding(&self, binding: &VariableBinding) -> bool {
        self.inner.send_binding(binding);
        self.cancel.store(true, Ordering::Release);
        true
    }

    fn send_complete(&self, result: &ResultSet) {
        self.inner.send_complete(result);
    }
}

fn start_server() -> (MockSnmpServer, Target) {
    let server = MockSnmpServer::new(0);
    let target = Target::v2c("127.0.0.1", server.addr.port(), "public");
    (server, target)
}

#[test]
fn engine_get_returns_value() {
    let rt = app_runtime();
    let (_server, target) = start_server();

    // Spawn onto an 8MB-stack worker; block_on itself only awaits the join.
    let rs = rt
        .block_on(rt.spawn(async move {
            let engine = SnmpEngine::new();
            let oids = vec!["1.3.6.1.2.1.1.1.0".to_string()];
            tokio::time::timeout(Duration::from_secs(5), engine.get(&target, &oids))
                .await
                .expect("get timed out")
                .expect("get failed")
        }))
        .expect("get task panicked");

    assert_eq!(rs.bindings.len(), 1);
    assert_eq!(rs.bindings[0].oid, "1.3.6.1.2.1.1.1.0");
    assert_eq!(
        rs.bindings[0].value,
        SnmpValue::OctetString(b"Linux router".to_vec())
    );
    assert!(!rs.partial);
}

#[test]
fn engine_walk_streams_bindings() {
    let rt = app_runtime();
    let (_server, target) = start_server();

    let sender = Arc::new(TestSender::default());
    let sender_for_task = sender.clone();
    rt.block_on(rt.spawn(async move {
        let engine = SnmpEngine::new();
        let handle = tokio::runtime::Handle::current();

        // Walk the ifDescr column — exactly three entries in the mock MIB.
        let join = engine.walk_streaming(
            &handle,
            &target,
            "1.3.6.1.2.1.2.2.1.2",
            sender_for_task,
            None,
        );
        tokio::time::timeout(Duration::from_secs(10), join)
            .await
            .expect("walk timed out")
            .expect("walk task panicked");
    }))
    .expect("test task panicked");

    let bindings = sender.bindings.lock().unwrap().clone();
    let oids: Vec<&str> = bindings.iter().map(|b| b.oid.as_str()).collect();
    assert_eq!(
        oids,
        vec![
            "1.3.6.1.2.1.2.2.1.2.1",
            "1.3.6.1.2.1.2.2.1.2.2",
            "1.3.6.1.2.1.2.2.1.2.3"
        ]
    );
    let complete = sender.complete.lock().unwrap().take();
    assert!(complete.is_some(), "complete result was not sent");
}

#[test]
fn engine_bulk_walk_streams_bindings() {
    let rt = app_runtime();
    let (_server, target) = start_server();

    let sender = Arc::new(TestSender::default());
    let sender_for_task = sender.clone();
    rt.block_on(rt.spawn(async move {
        let engine = SnmpEngine::new();
        let handle = tokio::runtime::Handle::current();

        let join = engine.bulk_walk_streaming(
            &handle,
            &target,
            "1.3.6.1.2.1.2.2.1.2",
            sender_for_task,
            None,
        );
        tokio::time::timeout(Duration::from_secs(10), join)
            .await
            .expect("bulk walk timed out")
            .expect("bulk walk task panicked");
    }))
    .expect("test task panicked");

    let bindings = sender.bindings.lock().unwrap().clone();
    let oids: Vec<&str> = bindings.iter().map(|b| b.oid.as_str()).collect();
    assert_eq!(
        oids,
        vec![
            "1.3.6.1.2.1.2.2.1.2.1",
            "1.3.6.1.2.1.2.2.1.2.2",
            "1.3.6.1.2.1.2.2.1.2.3"
        ]
    );
    assert!(sender.complete.lock().unwrap().is_some());
}

#[test]
fn engine_walk_cancel_stops_early() {
    let rt = app_runtime();
    let (_server, target) = start_server();

    let sender = Arc::new(TestSender::default());
    let cancel = Arc::new(AtomicBool::new(false));
    let cancelling = Arc::new(CancellingSender {
        inner: sender.clone(),
        cancel: cancel.clone(),
    });
    rt.block_on(rt.spawn(async move {
        let engine = SnmpEngine::new();
        let handle = tokio::runtime::Handle::current();

        // The full managed subtree has 11 OIDs; cancelling after the first
        // binding must stop the walk well short of that.
        let join = engine.walk_streaming(
            &handle,
            &target,
            "1.3.6.1.2.1",
            cancelling,
            Some(cancel.clone()),
        );
        tokio::time::timeout(Duration::from_secs(10), join)
            .await
            .expect("walk timed out")
            .expect("walk task panicked");
    }))
    .expect("test task panicked");

    let bindings = sender.bindings.lock().unwrap().clone();
    assert_eq!(
        bindings.len(),
        1,
        "cancel should stop after the first binding"
    );
    let complete = sender.complete.lock().unwrap().take();
    assert!(complete.is_some(), "complete result was not sent");
    assert!(
        complete.unwrap().partial,
        "cancelled walk must be marked partial"
    );
}

/// Collects progress updates and the final table grid.
#[derive(Default)]
struct TestTableSender {
    progress: Mutex<Vec<usize>>,
    complete: Mutex<Option<TableResult>>,
}

impl TableRowSender for TestTableSender {
    fn send_progress(&self, count: usize) -> bool {
        self.progress.lock().unwrap().push(count);
        true
    }

    fn send_complete(&self, result: &TableResult) {
        *self.complete.lock().unwrap() = Some(result.clone());
    }
}

/// A table sender that declares the client gone once `count` bindings have
/// been reported — exercises the stop-on-client-gone path.
struct GoneAtTableSender {
    inner: Arc<TestTableSender>,
    gone_at: usize,
}

impl TableRowSender for GoneAtTableSender {
    fn send_progress(&self, count: usize) -> bool {
        self.inner.send_progress(count);
        count < self.gone_at
    }

    fn send_complete(&self, result: &TableResult) {
        self.inner.send_complete(result);
    }
}

/// Populates a fresh mock server with a 12-row table (5 columns on an integer
/// index) plus a nested sub-table whose row suffixes collide with the outer
/// rows. Returns the server, target, table root OID, and column OIDs.
fn start_table_server() -> (MockSnmpServer, Target, String, Vec<String>) {
    let (server, target) = start_server();
    let table_oid = "1.3.6.1.4.1.99997.5".to_string();

    for row in 1..=12u32 {
        for col in 1..=5u32 {
            server.set_value(
                &format!("{table_oid}.1.{col}.{row}"),
                MockSnmpServer::ber_integer(row as i32 * col as i32),
            );
        }
    }
    // Nested sub-table under the same table subtree — must not appear in the
    // grid, even though its suffixes ("9", "10") match outer row IDs.
    server.set_value(
        &format!("{table_oid}.3.1.1.9"),
        MockSnmpServer::ber_integer(999),
    );
    server.set_value(
        &format!("{table_oid}.3.1.1.10"),
        MockSnmpServer::ber_integer(998),
    );

    let column_oids: Vec<String> = (1..=5).map(|c| format!("{table_oid}.1.{c}")).collect();
    (server, target, table_oid, column_oids)
}

#[test]
fn engine_get_table_single_pass_grid() {
    let rt = app_runtime();
    let (server, target, table_oid, column_oids) = start_table_server();

    let col1 = column_oids[0].clone();
    let sender = Arc::new(TestTableSender::default());
    let sender_for_task = sender.clone();
    rt.block_on(rt.spawn(async move {
        let engine = SnmpEngine::new();
        let handle = tokio::runtime::Handle::current();

        // 12 rows on an integer index, decoded per-component.
        let index_columns = vec![IndexColumnSpec {
            name: "rowId".to_string(),
            implied: false,
            encoding: IndexEncoding::Integer,
        }];
        let join = engine.get_table_streaming(
            &handle,
            &target,
            &table_oid,
            column_oids,
            index_columns,
            sender_for_task,
            None,
        );
        tokio::time::timeout(Duration::from_secs(15), join)
            .await
            .expect("get table timed out")
            .expect("get table task panicked");
    }))
    .expect("test task panicked");

    let grid = sender
        .complete
        .lock()
        .unwrap()
        .take()
        .expect("complete result was not sent");

    // G3 regression: rows in walk order — 2 before 10, no string sorting.
    assert_eq!(grid.total_rows, 12);
    let ids: Vec<&str> = grid.rows.iter().map(|r| r.instance_id.as_str()).collect();
    assert_eq!(ids, (1..=12).map(|i| i.to_string()).collect::<Vec<_>>());

    // Index values decoded per component.
    assert_eq!(grid.rows[0].index_values, vec![Some("1".to_string())]);
    assert_eq!(grid.rows[9].index_values, vec![Some("10".to_string())]);

    // Every selected column has data for every row.
    assert_eq!(grid.missing_cells, 0);
    assert_eq!(
        grid.rows[3]
            .cells
            .get(&col1)
            .and_then(|c| c.value.as_ref())
            .map(|v| v.value.clone()),
        Some(SnmpValue::Integer(4)) // row 4, column 1: value = row * col
    );

    // Nested sub-table data is excluded from the grid (G4) — its values
    // (999/998) can never come from the outer columns (max row*col = 60).
    for row in &grid.rows {
        for cell in row.cells.values() {
            if let Some(v) = &cell.value {
                assert_ne!(v.value, SnmpValue::Integer(999), "nested sub-table leaked");
                assert_ne!(v.value, SnmpValue::Integer(998), "nested sub-table leaked");
            }
        }
    }

    // Single pass: one walk chain over the whole subtree (not one per column).
    assert_eq!(server.walk_chain_count(), 1, "expected a single walk chain");
    assert!(server.request_count() >= 60, "all bindings must be fetched");

    // Progress streamed during the walk.
    let progress = sender.progress.lock().unwrap().clone();
    assert!(!progress.is_empty(), "no progress updates were sent");
    assert!(
        progress.windows(2).all(|w| w[0] < w[1]),
        "progress must increase"
    );

    assert!(!grid.partial);
}

#[test]
fn engine_get_table_client_gone_stops_early() {
    let rt = app_runtime();
    let (_server, target, table_oid, column_oids) = start_table_server();

    // 60 bindings total; declare the client gone at the first progress tick
    // (10 bindings — column-major, so rows 1-10 only). The walk must stop
    // promptly and report partial results.
    let inner = Arc::new(TestTableSender::default());
    let sender = Arc::new(GoneAtTableSender {
        inner: inner.clone(),
        gone_at: 10,
    });
    rt.block_on(rt.spawn(async move {
        let engine = SnmpEngine::new();
        let handle = tokio::runtime::Handle::current();

        let join = engine.get_table_streaming(
            &handle,
            &target,
            &table_oid,
            column_oids,
            Vec::new(),
            sender,
            None,
        );
        tokio::time::timeout(Duration::from_secs(15), join)
            .await
            .expect("get table timed out (hang on client-gone?)")
            .expect("get table task panicked");
    }))
    .expect("test task panicked");

    let grid = inner
        .complete
        .lock()
        .unwrap()
        .take()
        .expect("complete result was not sent");
    assert!(grid.partial, "client-gone stop must be marked partial");
    assert!(
        grid.total_rows < 12,
        "walk should have stopped before all rows arrived"
    );
}

#[test]
fn engine_set_roundtrip() {
    let rt = app_runtime();
    let (_server, target) = start_server();

    let rs = rt
        .block_on(rt.spawn(async move {
            let engine = SnmpEngine::new();

            let set = tokio::time::timeout(
                Duration::from_secs(5),
                engine.set(
                    &target,
                    "1.3.6.1.2.1.1.5.0",
                    SetValue::OctetString(b"renamed".to_vec()),
                ),
            )
            .await
            .expect("set timed out")
            .expect("set failed");
            assert_eq!(set.bindings.len(), 1);

            // The mock stores the value; a follow-up Get must return it.
            let oids = vec!["1.3.6.1.2.1.1.5.0".to_string()];
            tokio::time::timeout(Duration::from_secs(5), engine.get(&target, &oids))
                .await
                .expect("get timed out")
                .expect("get failed")
        }))
        .expect("set task panicked");

    assert_eq!(
        rs.bindings[0].value,
        SnmpValue::OctetString(b"renamed".to_vec())
    );
}
