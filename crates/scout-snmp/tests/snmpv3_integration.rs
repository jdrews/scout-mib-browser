//! Integration tests for SNMPv3 USM against a live snmpsim agent.
//!
//! These exercise the full engine path (Target -> connect() -> resolve() ->
//! new_v3 + init() discovery -> Get) across every supported auth protocol,
//! privacy protocol, and security level.
//!
//! They require the v3 agent started by `scripts/snmpsim-v3-test.py` on
//! 127.0.0.1:11700 (the user/passphrase matrix is fixed in that script). The
//! tests auto-skip if the agent is not reachable, so a plain
//! `cargo test --workspace --all-features` never hangs or fails without it.
//!
//! Run explicitly with the agent up:
//!   python3 scripts/snmpsim-v3-test.py &
//!   cargo test -p scout-snmp --test snmpv3_integration -- --ignored
//!
//! NOTE: snmp2 recurses deeply and overflows a default (2MB) thread stack, so
//! each test runs its own current-thread tokio runtime on an 8MB-stack thread
//! (mirroring the app crate's worker-stack sizing).

use std::future::Future;
use std::time::Duration;

use scout_snmp::{
    AuthProtocol, PrivProtocol, ResultSet, SnmpEngine, SnmpV3SecurityConfig, SnmpValue, Target,
};

const V3_ADDR: &str = "127.0.0.1";
const V3_PORT: u16 = 11700;
const SYS_DESCR_OID: &str = "1.3.6.1.2.1.1.1.0";

/// Runs an async v3 test body on an 8MB-stack thread with its own
/// current-thread tokio runtime (snmp2 needs the extra stack headroom).
fn run_v3_test<F, Fut>(f: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    const STACK_SIZE: usize = 8 * 1024 * 1024;
    let handle = std::thread::Builder::new()
        .name("snmpv3-test".to_string())
        .stack_size(STACK_SIZE)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build current-thread runtime");
            rt.block_on(f());
        })
        .expect("spawn 8MB-stack test thread");
    handle.join().expect("v3 test thread panicked");
}

fn v3_target(
    username: &str,
    auth: AuthProtocol,
    auth_pass: &str,
    priv_: PrivProtocol,
    priv_pass: &str,
) -> Target {
    Target::v3(
        V3_ADDR,
        V3_PORT,
        SnmpV3SecurityConfig {
            username: username.to_string(),
            auth_protocol: auth,
            auth_passphrase: auth_pass.to_string(),
            priv_protocol: priv_,
            priv_passphrase: priv_pass.to_string(),
        },
    )
}

/// True when the v3 agent answers a Get within the probe window. A bare UDP
/// connect always "succeeds", so we must actually round-trip a PDU. Probes with
/// an authenticated user (authNoPriv/MD5) — a level that works against strict
/// agents — so liveness detection is independent of any single security level.
async fn v3_agent_available() -> bool {
    let target = v3_target(
        "v3_md5",
        AuthProtocol::Md5,
        "auctoritas-md5",
        PrivProtocol::None,
        "",
    );
    let engine = SnmpEngine::new();
    let oids = vec![SYS_DESCR_OID.to_string()];
    match tokio::time::timeout(Duration::from_secs(6), engine.get(&target, &oids)).await {
        Ok(Ok(_)) => true,
        Ok(Err(e)) => {
            eprintln!("PROBE-ERR: {e}");
            false
        }
        Err(_) => {
            eprintln!("PROBE-TIMEOUT");
            false
        }
    }
}

async fn require_v3_agent() {
    assert!(
        v3_agent_available().await,
        "v3 snmpsim agent not reachable at {V3_ADDR}:{V3_PORT}. Start it with `python3 scripts/snmpsim-v3-test.py`"
    );
}

/// Gets sysDescr over the given v3 target and asserts it carries the recorded
/// Linux banner. Returns the result set for further assertions.
async fn get_sysdescr(target: &Target) -> ResultSet {
    let engine = SnmpEngine::new();
    let oids = vec![SYS_DESCR_OID.to_string()];
    let rs = engine
        .get(target, &oids)
        .await
        .unwrap_or_else(|e| panic!("v3 Get failed: {e}"));
    assert!(
        !rs.bindings.is_empty(),
        "expected a sysDescr binding, got none (warnings: {:?})",
        rs.warnings
    );
    let b = &rs.bindings[0];
    assert_eq!(b.oid, SYS_DESCR_OID);
    match &b.value {
        SnmpValue::OctetString(bytes) => {
            let s = String::from_utf8_lossy(bytes);
            assert!(
                s.contains("Linux"),
                "sysDescr should mention Linux, got: {s}"
            );
        }
        other => panic!("expected OctetString sysDescr, got {other:?}"),
    }
    rs
}

// ── noAuthNoPriv ────────────────────────────────────────────────────────────

#[ignore]
#[test]
fn v3_no_auth_no_priv() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target("v3_noauth", AuthProtocol::None, "", PrivProtocol::None, "");
        get_sysdescr(&target).await;
    });
}

// ── authNoPriv: every supported authentication protocol ─────────────────────

#[ignore]
#[test]
fn v3_auth_no_priv_md5() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_md5",
            AuthProtocol::Md5,
            "auctoritas-md5",
            PrivProtocol::None,
            "",
        );
        get_sysdescr(&target).await;
    });
}

#[ignore]
#[test]
fn v3_auth_no_priv_sha1() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_sha",
            AuthProtocol::Sha1,
            "auctoritas-sha",
            PrivProtocol::None,
            "",
        );
        get_sysdescr(&target).await;
    });
}

#[ignore]
#[test]
fn v3_auth_no_priv_sha224() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_sha224",
            AuthProtocol::Sha224,
            "auctoritas-sha224",
            PrivProtocol::None,
            "",
        );
        get_sysdescr(&target).await;
    });
}

#[ignore]
#[test]
fn v3_auth_no_priv_sha256() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_sha256",
            AuthProtocol::Sha256,
            "auctoritas-sha256",
            PrivProtocol::None,
            "",
        );
        get_sysdescr(&target).await;
    });
}

#[ignore]
#[test]
fn v3_auth_no_priv_sha384() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_sha384",
            AuthProtocol::Sha384,
            "auctoritas-sha384",
            PrivProtocol::None,
            "",
        );
        get_sysdescr(&target).await;
    });
}

#[ignore]
#[test]
fn v3_auth_no_priv_sha512() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_sha512",
            AuthProtocol::Sha512,
            "auctoritas-sha512",
            PrivProtocol::None,
            "",
        );
        get_sysdescr(&target).await;
    });
}

// ── authPriv: privacy ciphers across the protocol matrix ────────────────────

#[ignore]
#[test]
fn v3_auth_priv_des_md5() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_des_md5",
            AuthProtocol::Md5,
            "authpass-md5",
            PrivProtocol::Des,
            "privpass-des",
        );
        get_sysdescr(&target).await;
    });
}

#[ignore]
#[test]
fn v3_auth_priv_aes128_sha1() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_aes128_sha",
            AuthProtocol::Sha1,
            "authpass-sha",
            PrivProtocol::Aes128,
            "privpass-aes128",
        );
        get_sysdescr(&target).await;
    });
}

#[ignore]
#[test]
fn v3_auth_priv_aes192_sha256() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_aes192_sha256",
            AuthProtocol::Sha256,
            "authpass-sha256",
            PrivProtocol::Aes192,
            "privpass-aes192",
        );
        get_sysdescr(&target).await;
    });
}

#[ignore]
#[test]
fn v3_auth_priv_aes256_sha512() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_aes256_sha512",
            AuthProtocol::Sha512,
            "authpass-sha512",
            PrivProtocol::Aes256,
            "privpass-aes256",
        );
        get_sysdescr(&target).await;
    });
}

// AES-192 with a short auth hash (MD5): exercises the Reeder key-extension path.
#[ignore]
#[test]
fn v3_auth_priv_aes192_md5_key_extension() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_aes192_md5",
            AuthProtocol::Md5,
            "authpass-md5ext",
            PrivProtocol::Aes192,
            "privpass-aes192ext",
        );
        get_sysdescr(&target).await;
    });
}

// ── Negative: wrong credentials must not succeed ────────────────────────────

#[ignore]
#[test]
fn v3_auth_no_priv_wrong_passphrase_fails() {
    run_v3_test(|| async {
        require_v3_agent().await;
        let target = v3_target(
            "v3_md5",
            AuthProtocol::Md5,
            "wrong-pass-123",
            PrivProtocol::None,
            "",
        );
        let engine = SnmpEngine::new();
        let oids = vec![SYS_DESCR_OID.to_string()];
        let result =
            tokio::time::timeout(Duration::from_secs(30), engine.get(&target, &oids)).await;
        match result {
            Err(_) => {} // Timed out: acceptable — the bad auth never produced data.
            Ok(Ok(rs)) => panic!(
                "wrong passphrase should not yield bindings, got {:?}",
                rs.bindings
            ),
            Ok(Err(_)) => {} // Explicit auth failure: expected.
        }
    });
}
