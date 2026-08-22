//! Integration test for the curated e2e MIB set in `test/mibs/`.
//!
//! Guards the assumptions the e2e suite makes about this directory:
//! the real MIBs load via mib-rs (primary path) and BROKEN-MIB loads via
//! the regex fallback, with the OID anchors used by the specs present.

use scout_mib::Resolver;

fn test_mibs_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("test")
        .join("mibs");
    assert!(
        dir.is_dir(),
        "test/mibs not found — run `bash scripts/prepare-test-mibs.sh` first"
    );
    dir
}

#[test]
fn curated_mib_set_loads_with_expected_fallback() {
    let dir = test_mibs_dir();
    let mut resolver = Resolver::default();
    resolver.load_directories(&[dir.to_string_lossy().to_string()]);

    let loaded = resolver.loaded_mibs();
    // Module names may be normalized to uppercase by the parser.
    let names: Vec<String> = loaded.iter().map(|m| m.mib_name.to_uppercase()).collect();

    for expected in [
        "BROKEN-MIB",
        "IF-MIB",
        "SNMPV2-MIB",
        "SNMPV2-SMI",
        "SNMPV2-TC",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "{expected} not loaded; got {names:?}"
        );
    }

    // BROKEN-MIB must take the regex-fallback path (mib-rs cannot close it).
    let fallback: Vec<String> = resolver.fallback_mib_names().cloned().collect();
    assert!(
        fallback.iter().any(|n| n == "BROKEN-MIB"),
        "BROKEN-MIB should be a fallback MIB; got {fallback:?}"
    );

    // Exactly one fallback MIB — the e2e banner asserts "1 MIB(s) loaded via regex fallback".
    assert_eq!(
        fallback.len(),
        1,
        "expected exactly one fallback MIB, got {fallback:?}"
    );

    // Fallback recovered the OBJECT-TYPE blocks.
    let broken = resolver.reverse_lookup("brokenThing");
    assert!(
        broken.is_some(),
        "brokenThing not recovered by regex fallback"
    );

    // E2E OID anchors used by the operation/table specs. Note: this is the
    // node OID; scalar Gets append the `.0` instance suffix at query time.
    assert_eq!(resolver.reverse_lookup("sysDescr"), Some("1.3.6.1.2.1.1.1"));
    let if_table = resolver
        .resolve("1.3.6.1.2.1.2.2")
        .expect("ifTable missing");
    assert!(if_table.is_table, "ifTable should be a TABLE node");

    // The system subtree anchor for walk tests.
    assert_eq!(resolver.reverse_lookup("system"), Some("1.3.6.1.2.1.1"));
}
