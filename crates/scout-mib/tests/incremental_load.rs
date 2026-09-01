//! Incremental load behavior: unchanged files are served from the parse
//! cache (no re-parse), changed/new files are re-parsed, removed files drop
//! out of the index, and unloading invalidates the cache.

use scout_mib::Resolver;
use std::fs;
use std::path::{Path, PathBuf};

fn mib_fixture(module: &str, ident: &str, enterprise: u32, extra_objects: &str) -> String {
    format!(
        r#"{module} DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, enterprises
        FROM SNMPv2-SMI;

{ident} MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "A test module."
    ::= {{ enterprises {enterprise} }}

{ident}Objects OBJECT IDENTIFIER ::= {{ {ident} 1 }}

baseValue OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Always present."
    ::= {{ {ident}Objects 1 }}

{extra_objects}
END
"#,
        module = module,
        ident = ident,
        enterprise = enterprise,
        extra_objects = extra_objects,
    )
}

/// An extra OBJECT-TYPE nested under the module's own objects subtree.
fn extra_object(ident: &str, name: &str) -> String {
    format!(
        r#"{name} OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Added later."
    ::= {{ {ident}Objects 2 }}
"#
    )
}

/// Creates a temp dir with two fixture MIBs and loads them into a fresh
/// resolver. Returns (dir, resolver).
fn setup_two_mibs(tag: &str) -> (PathBuf, Resolver) {
    let dir = std::env::temp_dir().join(format!("scout_incremental_{tag}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("ALPHA-MIB"),
        mib_fixture("ALPHA-MIB", "alphaMib", 99001, ""),
    )
    .unwrap();
    fs::write(
        dir.join("BETA-MIB"),
        mib_fixture("BETA-MIB", "betaMib", 99002, ""),
    )
    .unwrap();

    let mut resolver = Resolver::default();
    let stats = resolver.load_directories(&[dir.to_string_lossy().to_string()]);
    assert_eq!(stats.parsed, 2, "fresh load must parse both files");
    assert_eq!(stats.cached, 0);
    (dir, resolver)
}

fn dirs(dir: &Path) -> Vec<String> {
    vec![dir.to_string_lossy().to_string()]
}

#[test]
fn fresh_load_parses_all_files() {
    let (_dir, resolver) = setup_two_mibs("fresh");

    assert!(resolver.reverse_lookup("baseValue").is_some());
    let loaded: Vec<String> = resolver
        .loaded_mibs()
        .iter()
        .map(|m| m.mib_name.clone())
        .collect();
    assert!(loaded.iter().any(|n| n == "ALPHA-MIB"));
    assert!(loaded.iter().any(|n| n == "BETA-MIB"));
}

#[test]
fn unchanged_reload_is_served_from_cache() {
    let (dir, mut resolver) = setup_two_mibs("unchanged");
    let nodes_before = resolver.node_count();

    let stats = resolver.load_directories(&dirs(&dir));
    assert_eq!(stats.parsed, 0, "nothing changed — no re-parsing");
    assert_eq!(stats.cached, 2);
    assert_eq!(resolver.node_count(), nodes_before, "index must be intact");
}

#[test]
fn changed_file_is_reparsed_and_updated() {
    let (dir, mut resolver) = setup_two_mibs("changed");

    // Append a new object to ALPHA-MIB (content and size change).
    let path = dir.join("ALPHA-MIB");
    fs::write(
        &path,
        mib_fixture(
            "ALPHA-MIB",
            "alphaMib",
            99001,
            &extra_object("alphaMib", "addedAlpha"),
        ),
    )
    .unwrap();

    let stats = resolver.load_directories(&dirs(&dir));
    assert_eq!(stats.parsed, 1, "only the changed file is re-parsed");
    assert_eq!(stats.cached, 1);

    assert!(
        resolver.reverse_lookup("addedAlpha").is_some(),
        "newly added object must be indexed"
    );
    // The untouched module is still fully present.
    let loaded = resolver.loaded_mibs();
    let beta = loaded
        .iter()
        .find(|m| m.mib_name == "BETA-MIB")
        .expect("BETA-MIB still loaded");
    assert!(beta.node_count > 0);
}

#[test]
fn removed_file_drops_out_of_index() {
    let (dir, mut resolver) = setup_two_mibs("removed");

    fs::remove_file(dir.join("ALPHA-MIB")).unwrap();

    let stats = resolver.load_directories(&dirs(&dir));
    assert_eq!(stats.parsed, 0);
    assert_eq!(stats.cached, 1);

    let loaded: Vec<String> = resolver
        .loaded_mibs()
        .iter()
        .map(|m| m.mib_name.clone())
        .collect();
    assert!(
        !loaded.iter().any(|n| n == "ALPHA-MIB"),
        "removed file must drop out"
    );
    assert!(loaded.iter().any(|n| n == "BETA-MIB"));
}

#[test]
fn new_file_is_parsed_on_next_load() {
    let (dir, mut resolver) = setup_two_mibs("newfile");

    fs::write(
        dir.join("GAMMA-MIB"),
        mib_fixture("GAMMA-MIB", "gammaMib", 99003, ""),
    )
    .unwrap();

    let stats = resolver.load_directories(&dirs(&dir));
    assert_eq!(stats.parsed, 1, "only the new file is parsed");
    assert_eq!(stats.cached, 2);

    let loaded: Vec<String> = resolver
        .loaded_mibs()
        .iter()
        .map(|m| m.mib_name.clone())
        .collect();
    assert!(loaded.iter().any(|n| n == "GAMMA-MIB"));
}

#[test]
fn unload_invalidates_cache_and_reload_restores() {
    let (dir, mut resolver) = setup_two_mibs("unload");

    resolver.unload_mib("ALPHA-MIB");
    assert!(resolver.reverse_lookup("baseValue").is_none());

    // Next load re-parses the unloaded file's module and restores it.
    let stats = resolver.load_directories(&dirs(&dir));
    assert!(stats.parsed >= 1, "unloaded module must be re-parsed");
    assert!(resolver.reverse_lookup("baseValue").is_some());

    let loaded: Vec<String> = resolver
        .loaded_mibs()
        .iter()
        .map(|m| m.mib_name.clone())
        .collect();
    assert!(loaded.iter().any(|n| n == "ALPHA-MIB"));
}

#[test]
fn nonexistent_directory_is_tolerated() {
    let (dir, mut resolver) = setup_two_mibs("nodir");
    let missing = dir.join("does-not-exist").to_string_lossy().to_string();

    let stats = resolver.load_directories(&[missing]);
    // All previously loaded files vanished from the directory list.
    assert_eq!(stats.parsed, 0);
    assert_eq!(resolver.node_count(), 0);
}
