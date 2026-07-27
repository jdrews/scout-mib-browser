//! Integration tests against a live snmpsim instance.
//!
//! These tests require snmpsim running on 127.0.0.1:11611 with community "public" (v2c).
//! Tests are skipped automatically if the endpoint is unreachable.

use std::net::SocketAddr;

enum WalkTerm {
    Data,
    Done,
}

const SNMP_ADDR: &str = "127.0.0.1:11611";
const SNMP_COMMUNITY: &[u8] = b"public";

fn is_snmpsim_available() -> bool {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .ok()
        .and_then(|socket| socket.connect(SNMP_ADDR).ok())
        .is_some()
}

fn require_snmpsim() {
    assert!(
        is_snmpsim_available(),
        "snmpsim not reachable at {}. Start it with `scripts/snmpsim-test.py`",
        SNMP_ADDR
    );
}

async fn session() -> snmp2::AsyncSession {
    let addr: SocketAddr = SNMP_ADDR.parse().unwrap();
    snmp2::AsyncSession::new_v2c(addr, SNMP_COMMUNITY, 0)
        .await
        .expect("connect to snmpsim")
}

/// Walks a subtree using GetNext and returns all OID strings.
async fn walk_getnext(root_oid: &str) -> Vec<String> {
    let root_prefix = format!("{}.", root_oid);
    let mut bindings = Vec::new();
    let mut current_oid: snmp2::Oid = root_oid.parse().unwrap();

    loop {
        // Fresh session per request to avoid borrow checker issues with async loops.
        let varbind_oids: Vec<(String, WalkTerm)> = {
            let mut sess = session().await;
            let pdu = sess.getnext(&current_oid).await.expect("GetNext in walk");
            let result: Vec<_> = pdu
                .varbinds
                .map(|(o, v)| {
                    let term = match v {
                        snmp2::Value::EndOfMibView
                        | snmp2::Value::NoSuchObject
                        | snmp2::Value::NoSuchInstance => WalkTerm::Done,
                        _ => WalkTerm::Data,
                    };
                    (o.to_string(), term)
                })
                .collect();
            result
        };

        if varbind_oids.is_empty() {
            return bindings;
        }

        let mut next_oid = None;
        for (oid_str, term) in varbind_oids {
            if matches!(term, WalkTerm::Done) {
                return bindings;
            }

            if oid_str != root_oid && !oid_str.starts_with(&root_prefix) {
                return bindings;
            }

            bindings.push(oid_str.clone());
            next_oid = Some(oid_str.parse().unwrap());
        }

        match next_oid {
            Some(oid) => current_oid = oid,
            None => return bindings,
        }
    }
}

/// Walks a subtree using GetBulk and returns all OID strings.
async fn walk_getbulk(root_oid: &str) -> Vec<String> {
    let root_prefix = format!("{}.", root_oid);
    let mut bindings = Vec::new();
    let mut current_oid: snmp2::Oid = root_oid.parse().unwrap();

    loop {
        // Fresh session per request to avoid borrow checker issues with async loops.
        let varbind_oids: Vec<(String, WalkTerm)> = {
            let mut sess = session().await;
            let pdu = sess
                .getbulk(&[&current_oid], 0, 50)
                .await
                .expect("GetBulk in walk");
            let result: Vec<_> = pdu
                .varbinds
                .map(|(o, v)| {
                    let term = match v {
                        snmp2::Value::EndOfMibView
                        | snmp2::Value::NoSuchObject
                        | snmp2::Value::NoSuchInstance => WalkTerm::Done,
                        _ => WalkTerm::Data,
                    };
                    (o.to_string(), term)
                })
                .collect();
            result
        };

        if varbind_oids.is_empty() {
            return bindings;
        }

        let mut next_oid = None;
        for (oid_str, term) in varbind_oids {
            if matches!(term, WalkTerm::Done) {
                return bindings;
            }

            if oid_str != root_oid && !oid_str.starts_with(&root_prefix) {
                return bindings;
            }

            bindings.push(oid_str.clone());
            next_oid = Some(oid_str.parse().unwrap());
        }

        match next_oid {
            Some(oid) => current_oid = oid,
            None => return bindings,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn snmp_get_sysdescr() {
    require_snmpsim();
    let mut sess = session().await;
    let oid: snmp2::Oid = "1.3.6.1.2.1.1.1.0".parse().unwrap();
    let pdu = sess.get(&oid).await.expect("Get sysDescr should succeed");

    let bindings: Vec<_> = pdu.varbinds.collect();
    assert_eq!(bindings.len(), 1);

    let (_, v) = &bindings[0];
    match v {
        snmp2::Value::OctetString(bytes) => {
            let s = String::from_utf8_lossy(&bytes);
            assert!(
                s.contains("Linux"),
                "sysDescr should mention Linux, got: {}",
                s
            );
        }
        other => panic!("Expected OctetString for sysDescr, got {:?}", other),
    }
}

#[tokio::test]
async fn snmp_get_multiple_oids() {
    require_snmpsim();
    let mut sess = session().await;
    let oid1: snmp2::Oid = "1.3.6.1.2.1.1.1.0".parse().unwrap();
    let oid2: snmp2::Oid = "1.3.6.1.2.1.1.5.0".parse().unwrap();
    let pdu = sess
        .get_many(&[&oid1, &oid2])
        .await
        .expect("Get multiple OIDs");

    let bindings: Vec<_> = pdu.varbinds.collect();
    assert_eq!(bindings.len(), 2);
}

#[tokio::test]
async fn snmp_get_next() {
    require_snmpsim();
    let mut sess = session().await;
    let oid: snmp2::Oid = "1.3.6.1.2.1.1.1.0".parse().unwrap();
    let pdu = sess.getnext(&oid).await.expect("GetNext should succeed");

    let bindings: Vec<_> = pdu.varbinds.collect();
    assert_eq!(bindings.len(), 1);

    let (o, _) = &bindings[0];
    assert_ne!(o.to_string(), "1.3.6.1.2.1.1.1.0");
}

#[tokio::test]
async fn snmp_walk_sysgroup() {
    require_snmpsim();
    let bindings = walk_getnext("1.3.6.1.2.1.1").await;

    assert!(
        bindings.len() >= 5,
        "Expected at least 5 bindings in sys group walk, got {}",
        bindings.len()
    );

    for oid_str in &bindings {
        assert!(
            oid_str.starts_with("1.3.6.1.2.1.1"),
            "OID {} is not under subtree",
            oid_str
        );
    }
}

#[tokio::test]
async fn snmp_bulk_walk_sysgroup() {
    require_snmpsim();
    let bindings = walk_getbulk("1.3.6.1.2.1.1").await;

    assert!(
        bindings.len() >= 5,
        "Expected at least 5 bindings in BulkWalk, got {}",
        bindings.len()
    );

    for oid_str in &bindings {
        assert!(
            oid_str.starts_with("1.3.6.1.2.1.1"),
            "OID {} is not under subtree",
            oid_str
        );
    }
}

#[tokio::test]
async fn snmp_walk_contains_sysname() {
    require_snmpsim();
    let bindings = walk_getnext("1.3.6.1.2.1.1").await;

    assert!(
        bindings.iter().any(|oid| oid == "1.3.6.1.2.1.1.5.0"),
        "Walk should contain sysName (1.3.6.1.2.1.1.5.0)"
    );
}

#[tokio::test]
async fn snmp_walk_contains_uptime() {
    require_snmpsim();
    let oids = walk_getnext("1.3.6.1.2.1.1").await;

    assert!(
        oids.iter().any(|oid| oid == "1.3.6.1.2.1.1.3.0"),
        "Walk should contain sysUpTime (1.3.6.1.2.1.1.3.0)"
    );
}

#[tokio::test]
async fn snmp_walk_small_subtree() {
    require_snmpsim();
    let bindings = walk_getnext("1.3.6.1.2.1.1.5").await;

    assert_eq!(
        bindings.len(),
        1,
        "sysName subtree should have exactly 1 binding"
    );
    assert_eq!(bindings[0], "1.3.6.1.2.1.1.5.0");
}

#[tokio::test]
async fn snmp_get_nonexistent_oid() {
    require_snmpsim();
    let mut sess = session().await;
    let oid: snmp2::Oid = "1.3.6.1.999.999.999.0".parse().unwrap();

    match sess.get(&oid).await {
        Ok(pdu) => {
            let bindings: Vec<_> = pdu.varbinds.collect();
            assert!(!bindings.is_empty());
        }
        Err(_) => {
            // Agent may return an error for non-existent OIDs.
        }
    }
}
