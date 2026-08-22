use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use super::{LoadResult, MibNode, SyntaxType};

/// Primary MIB loader using the mib-rs crate.
///
/// Handles SMIv1/SMIv2 parsing with full IMPORT/EXPORT resolution and macro
/// expansion. Builds a complete OID-to-name-to-type index from parsed modules.
#[derive(Default)]
pub struct MibRsLoader {
    /// Tracks which files produced at least one module.
    loaded_files: HashSet<PathBuf>,
}

impl MibRsLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if mib-rs successfully produced results for the given file.
    pub fn has_module_for_file(&self, path: &Path) -> bool {
        self.loaded_files.contains(path)
    }

    /// Attempts to load a single MIB file using mib-rs.
    ///
    /// Returns a [`LoadResult`] with extracted nodes. `primary_success` is
    /// `true` if mib-rs parsed the file without fatal errors, `false` if it
    /// produced partial results or only diagnostics.
    pub fn load_file(&mut self, path: &Path) -> Result<LoadResult, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

        // Try to detect the module name from file content, fallback to filename.
        let module_name = self.detect_module_name_or_filename(&content, path);
        if module_name.is_empty() {
            warn!(
                "Cannot determine module name for {}, skipping",
                path.display()
            );
            return Ok(LoadResult {
                nodes: Vec::new(),
                primary_success: false,
                module_name: String::new(),
            });
        }

        info!(
            "Loading MIB module '{}' from {}",
            module_name,
            path.display()
        );

        let source = mib_rs::source::memory(&module_name, content.as_bytes());

        // Use permissive resolver and silent diagnostics for maximum tolerance.
        let result = mib_rs::Loader::new()
            .source(source)
            .modules([module_name.clone()])
            .resolver_strictness(mib_rs::ResolverStrictness::Permissive)
            .diagnostic_config(mib_rs::DiagnosticConfig::silent())
            .load();

        match result {
            Ok(mib) => {
                let mut nodes = Vec::new();

                if mib.has_errors() {
                    let error_count = mib
                        .diagnostics()
                        .iter()
                        .filter(|d| {
                            matches!(
                                d.severity,
                                mib_rs::Severity::Error | mib_rs::Severity::Severe
                            )
                        })
                        .count();
                    warn!("mib-rs loaded {} with {} errors", module_name, error_count);
                }

                // Extract all objects from the module.
                if let Some(module) = mib.module(&module_name) {
                    for obj in module.objects() {
                        let node_obj = obj.node();
                        let oid_str = node_obj.oid().to_string();
                        let name = obj.name().to_string();

                        // Detect SMI table structure from mib-rs metadata.
                        let is_table = obj.is_table();
                        let is_row = obj.is_row();

                        // Determine syntax type from the object's type and structural role.
                        let syntax_type = if is_table {
                            SyntaxType::Table
                        } else if is_row {
                            SyntaxType::TableRow
                        } else if let Some(ty) = obj.ty() {
                            Self::base_type_to_syntax(&ty.effective_base())
                        } else {
                            SyntaxType::Unknown("none".to_string())
                        };

                        nodes.push(MibNode {
                            oid: oid_str,
                            name,
                            syntax_type,
                            mib_name: module.name().to_string(),
                            is_table,
                        });
                    }

                    // Also extract OBJECT IDENTIFIER nodes (OID subtrees).
                    for node in module.nodes() {
                        let oid_str = node.oid().to_string();
                        let name = node.name().to_string();

                        // Skip if already indexed as an object.
                        if !nodes.iter().any(|n| n.oid == oid_str) {
                            nodes.push(MibNode {
                                oid: oid_str,
                                name,
                                syntax_type: SyntaxType::ObjectIdentifier,
                                mib_name: module.name().to_string(),
                                is_table: false,
                            });
                        }
                    }
                }

                self.loaded_files.insert(path.to_path_buf());
                info!(
                    "mib-rs loaded {} nodes from {}",
                    nodes.len(),
                    path.display()
                );

                Ok(LoadResult {
                    nodes,
                    primary_success: true,
                    module_name: module_name.clone(),
                })
            }
            Err(e) => {
                warn!(
                    "mib-rs failed to load {}: {} — will try regex fallback",
                    path.display(),
                    e
                );

                Ok(LoadResult {
                    nodes: Vec::new(),
                    primary_success: false,
                    module_name: module_name.clone(),
                })
            }
        }
    }

    /// Maps a mib-rs [`BaseType`] to our [`SyntaxType`].
    fn base_type_to_syntax(base: &mib_rs::BaseType) -> SyntaxType {
        match *base {
            mib_rs::BaseType::Integer32 => SyntaxType::Integer32,
            mib_rs::BaseType::OctetString => SyntaxType::OctetString,
            mib_rs::BaseType::ObjectIdentifier => SyntaxType::ObjectIdentifier,
            mib_rs::BaseType::Counter32 => SyntaxType::Counter32,
            mib_rs::BaseType::Counter64 => SyntaxType::Counter64,
            mib_rs::BaseType::Gauge32 => SyntaxType::Gauge32,
            mib_rs::BaseType::TimeTicks => SyntaxType::TimeTicks,
            mib_rs::BaseType::IpAddress => SyntaxType::IpAddress,
            mib_rs::BaseType::Unsigned32 => SyntaxType::Unsigned32,
            mib_rs::BaseType::Bits => SyntaxType::Bits,
            mib_rs::BaseType::Sequence => SyntaxType::Sequence,
            _ => SyntaxType::Unknown(format!("{}", base)),
        }
    }

    /// Detects the MIB module name, falling back to filename without extension.
    ///
    /// Uses the original case from the `DEFINITIONS` header: mib-rs matches
    /// requested module names case-sensitively against the parsed source, so
    /// normalizing to uppercase here would make mixed-case modules (e.g.
    /// `SNMPv2-SMI`) unloadable and force them onto the regex fallback.
    fn detect_module_name_or_filename(&self, content: &str, path: &Path) -> String {
        let name = super::detect_module_name_original(content);
        if !name.is_empty() {
            return name;
        }
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_uppercase())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_module_name_from_content() {
        let content = r#"MY-TEST-MIB DEFINITIONS ::= BEGIN
END
"#;
        assert_eq!(crate::detect_module_name(content), "MY-TEST-MIB");
    }

    #[test]
    fn detect_module_name_case_insensitive() {
        let content = r#"my-test-mib definitions ::= begin
end
"#;
        assert_eq!(crate::detect_module_name(content), "MY-TEST-MIB");
    }

    #[test]
    fn detect_module_name_no_match() {
        let content = "this is not a valid MIB file";
        assert_eq!(crate::detect_module_name(content), "");
    }

    #[test]
    fn detect_module_name_falls_back_to_filename() {
        let tmp_dir = std::env::temp_dir().join("scout_loader_filename_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mib_path = tmp_dir.join("MY-CUSTOM-MIB.txt");
        std::fs::write(&mib_path, "no module definition here").unwrap();

        let loader = MibRsLoader::new();
        let name = loader.detect_module_name_or_filename("no module definition here", &mib_path);
        assert_eq!(name, "MY-CUSTOM-MIB");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn detect_module_name_preserves_original_case() {
        let content = r#"SNMPv2-SMI DEFINITIONS ::= BEGIN
END
"#;
        let loader = MibRsLoader::new();
        assert_eq!(
            loader.detect_module_name_or_filename(content, Path::new("SNMPv2-SMI")),
            "SNMPv2-SMI"
        );
    }

    #[test]
    fn load_mixed_case_module_via_primary_parser() {
        // mib-rs matches requested module names case-sensitively against the
        // parsed source. Mixed-case headers must be requested with their
        // original case or the file is forced onto the regex fallback.
        let tmp_dir = std::env::temp_dir().join("scout_loader_mixedcase_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mib_content = r#"Mixed-Case-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, enterprises
        FROM SNMPv2-SMI;

mixedMib MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "A mixed-case test module."
    ::= { enterprises 99997 }

mixedThings OBJECT IDENTIFIER ::= { mixedMib 1 }

mixedThing OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "An object in a mixed-case module."
    ::= { mixedThings 1 }

END
"#;

        let mib_path = tmp_dir.join("Mixed-Case-MIB");
        std::fs::write(&mib_path, mib_content).unwrap();

        let mut loader = MibRsLoader::new();
        let result = loader.load_file(&mib_path).expect("should load");

        assert!(
            result.primary_success,
            "mixed-case module must load via mib-rs"
        );
        assert!(
            result.nodes.iter().any(|n| n.name == "mixedThing"),
            "expected mixedThing in nodes: {:?}",
            result.nodes.iter().map(|n| &n.name).collect::<Vec<_>>()
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn load_valid_mib_from_memory() {
        let tmp_dir = std::env::temp_dir().join("scout_loader_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mib_content = r#"TEST-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, enterprises
        FROM SNMPv2-SMI
    DisplayString
        FROM SNMPv2-TC;

testMib MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "A test module."
    ::= { enterprises 99998 }

testScalars OBJECT IDENTIFIER ::= { testMib 1 }

testName OBJECT-TYPE
    SYNTAX DisplayString (SIZE (0..255))
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "A name."
    ::= { testScalars 1 }

testCount OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "A counter."
    ::= { testScalars 2 }

END
"#;

        let mib_path = tmp_dir.join("TEST-MIB.txt");
        std::fs::write(&mib_path, mib_content).unwrap();

        let mut loader = MibRsLoader::new();
        let result = loader.load_file(&mib_path).expect("should load");

        assert!(result.primary_success);
        assert!(!result.nodes.is_empty());

        // Check that we found the expected objects.
        let names: Vec<_> = result.nodes.iter().map(|n| &n.name).collect();
        assert!(names.contains(&&"testName".to_string()));
        assert!(names.contains(&&"testCount".to_string()));

        // Verify syntax types.
        let name_node = result.nodes.iter().find(|n| n.name == "testName").unwrap();
        assert_eq!(name_node.syntax_type, SyntaxType::OctetString);

        let count_node = result.nodes.iter().find(|n| n.name == "testCount").unwrap();
        assert_eq!(count_node.syntax_type, SyntaxType::Integer32);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn load_invalid_mib_returns_partial() {
        let tmp_dir = std::env::temp_dir().join("scout_loader_bad_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        // This is a malformed MIB that mib-rs may not fully parse.
        let bad_content = r#"BROKEN-MIB DEFINITIONS ::= BEGIN
-- Missing imports, broken syntax
someObject OBJECT-TYPE
    SYNTAX SomethingUnknown
    -- missing clauses
END
"#;

        let mib_path = tmp_dir.join("BROKEN-MIB.txt");
        std::fs::write(&mib_path, bad_content).unwrap();

        let mut loader = MibRsLoader::new();
        let result = loader.load_file(&mib_path);

        // Should not panic — either succeeds with partial data or returns an error.
        match result {
            Ok(r) => {
                // If it loaded, primary_success may be false due to errors.
                info!(
                    "Loaded broken MIB: success={}, nodes={}",
                    r.primary_success,
                    r.nodes.len()
                );
            }
            Err(e) => {
                // Parse error is acceptable — fallback will handle it.
                info!("mib-rs rejected broken MIB: {}", e);
            }
        }

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn load_mib_with_table_detects_structure() {
        let tmp_dir = std::env::temp_dir().join("scout_loader_table_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        // MIB with a proper TABLE definition.
        let mib_content = r#"TABLE-TEST-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, enterprises
        FROM SNMPv2-SMI
    DisplayString
        FROM SNMPv2-TC;

tableTestMib MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "A test module with a table."
    ::= { enterprises 99997 }

tableTestTables OBJECT IDENTIFIER ::= { tableTestMib 1 }

testTable OBJECT-TYPE
    SYNTAX SEQUENCE OF TestEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "A test table."
    ::= { tableTestTables 1 }

testEntry OBJECT-TYPE
    SYNTAX TestEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "A test row entry."
    INDEX { testIndex }
    ::= { testTable 1 }

TestEntry ::= SEQUENCE {
    testIndex Integer32,
    testName DisplayString,
    testValue Integer32
}

testIndex OBJECT-TYPE
    SYNTAX Integer32 (1..2147483647)
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Test index column."
    ::= { testEntry 1 }

testName OBJECT-TYPE
    SYNTAX DisplayString (SIZE (0..255))
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Test name column."
    ::= { testEntry 2 }

testValue OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-write
    STATUS current
    DESCRIPTION "Test value column."
    ::= { testEntry 3 }

END
"#;

        let mib_path = tmp_dir.join("TABLE-TEST-MIB.txt");
        std::fs::write(&mib_path, mib_content).unwrap();

        let mut loader = MibRsLoader::new();
        let result = loader.load_file(&mib_path).expect("should load");

        assert!(result.primary_success);

        // Find the table node.
        let table_node = result.nodes.iter().find(|n| n.name == "testTable").unwrap();
        assert_eq!(table_node.syntax_type, SyntaxType::Table);
        assert!(table_node.is_table);

        // Find the row entry node.
        let row_node = result.nodes.iter().find(|n| n.name == "testEntry").unwrap();
        assert_eq!(row_node.syntax_type, SyntaxType::TableRow);
        assert!(!row_node.is_table);

        // Column nodes should have their proper types and not be marked as tables.
        let index_node = result.nodes.iter().find(|n| n.name == "testIndex").unwrap();
        assert_eq!(index_node.syntax_type, SyntaxType::Integer32);
        assert!(!index_node.is_table);

        let name_col = result.nodes.iter().find(|n| n.name == "testName").unwrap();
        assert_eq!(name_col.syntax_type, SyntaxType::OctetString);
        assert!(!name_col.is_table);

        let value_col = result.nodes.iter().find(|n| n.name == "testValue").unwrap();
        assert_eq!(value_col.syntax_type, SyntaxType::Integer32);
        assert!(!value_col.is_table);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
