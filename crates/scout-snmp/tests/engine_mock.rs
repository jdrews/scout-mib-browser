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
    MockSnmpServer, ResultSet, SetValue, SnmpEngine, SnmpValue, Target, VariableBinding,
    WalkBatchSender,
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
    let sender_done = sender.clone();
    rt.block_on(rt.spawn(async move {
        let engine = SnmpEngine::new();
        let handle = tokio::runtime::Handle::current();

        // Walk the ifDescr column — exactly three entries in the mock MIB.
        let join = engine.walk_streaming(
            &handle,
            &target,
            "1.3.6.1.2.1.2.2.1.2",
            sender_done.clone(),
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
    let sender_done = sender.clone();
    rt.block_on(rt.spawn(async move {
        let engine = SnmpEngine::new();
        let handle = tokio::runtime::Handle::current();

        let join = engine.bulk_walk_streaming(
            &handle,
            &target,
            "1.3.6.1.2.1.2.2.1.2",
            sender_done.clone(),
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
