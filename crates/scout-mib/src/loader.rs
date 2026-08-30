use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use super::{
    oid_numeric_cmp, IndexColumn, IndexEncoding, LoadResult, MibNode, NamedValueInfo, SyntaxType,
    TableInfo,
};

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
                tables: Vec::new(),
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
                let mut tables = Vec::new();

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
                            description: Self::non_empty(obj.description()),
                            access: Some(obj.access().to_string()),
                            status: Some(obj.status().to_string()),
                            units: Self::non_empty(obj.units()),
                            default_value: obj
                                .default_value()
                                .filter(|d| !d.is_unset())
                                .map(|d| d.raw().to_string()),
                            reference: Self::non_empty(obj.reference()),
                            display_hint: Self::non_empty(obj.effective_display_hint()),
                            constraints: Self::format_constraints(
                                obj.effective_ranges(),
                                obj.effective_sizes(),
                            ),
                            enums: obj
                                .effective_enums()
                                .iter()
                                .map(|n| NamedValueInfo {
                                    label: n.label.clone(),
                                    value: n.value,
                                })
                                .collect(),
                            bits: obj
                                .effective_bits()
                                .iter()
                                .map(|n| NamedValueInfo {
                                    label: n.label.clone(),
                                    value: n.value,
                                })
                                .collect(),
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
                                description: Self::non_empty(node.description()),
                                reference: Self::non_empty(node.reference()),
                                status: node.status().map(|s| s.to_string()),
                                ..Default::default()
                            });
                        }
                    }

                    // Extract table metadata (INDEX/AUGMENTS) for every table.
                    // Per-table tolerance: a failure degrades that table to the
                    // column heuristic and never blocks the rest of the module.
                    for obj in module.objects() {
                        if !obj.is_table() {
                            continue;
                        }
                        match Self::build_table_info(&module, obj) {
                            Some(info) => tables.push(info),
                            None => warn!(
                                "Failed to build table metadata for {} — falling back to column heuristic",
                                obj.name()
                            ),
                        }
                    }
                }

                self.loaded_files.insert(path.to_path_buf());
                info!(
                    "mib-rs loaded {} nodes ({} tables) from {}",
                    nodes.len(),
                    tables.len(),
                    path.display()
                );

                Ok(LoadResult {
                    nodes,
                    tables,
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
                    tables: Vec::new(),
                    primary_success: false,
                    module_name: module_name.clone(),
                })
            }
        }
    }

    /// Builds [`TableInfo`] for a table object from its row entries and INDEX clause.
    ///
    /// Row entries are the table's base entry plus every row reached through
    /// `AUGMENTS` (transitively). Index columns come from the base row's
    /// effective indexes, which follow the augment chain automatically. Columns
    /// are every object mib-rs classifies as a column whose containing table
    /// is this one — the exact set, including augmented columns and excluding
    /// leaves of nested sub-tables.
    fn build_table_info(
        module: &mib_rs::Module<'_>,
        table_obj: mib_rs::Object<'_>,
    ) -> Option<TableInfo> {
        let table_oid = table_obj.node().oid().to_string();
        let name = table_obj.name().to_string();

        let base_row = table_obj.row()?;

        // Index columns from the base row (effective_indexes follows AUGMENTS).
        let mut index_columns: Vec<IndexColumn> = Vec::new();
        for idx in base_row.effective_indexes() {
            index_columns.push(IndexColumn {
                name: idx.name().to_string(),
                oid: idx
                    .object()
                    .map(|o| o.node().oid().to_string())
                    .unwrap_or_default(),
                implied: idx.implied(),
                encoding: match idx.encoding() {
                    mib_rs::IndexEncoding::Integer => IndexEncoding::Integer,
                    mib_rs::IndexEncoding::IpAddress => IndexEncoding::IpAddress,
                    mib_rs::IndexEncoding::FixedString | mib_rs::IndexEncoding::Implied => {
                        // `Implied` folds the IMPLIED keyword into the encoding
                        // and skips width detection, so recover a fixed width
                        // from the object's SIZE constraint when present.
                        let size = if idx.encoding() == mib_rs::IndexEncoding::FixedString {
                            let (size, fixed) = idx.fixed_size();
                            (fixed && size > 0).then_some(size)
                        } else {
                            // Only OCTET STRING widths are byte counts; other
                            // implied bases (OID, BITS, Opaque) stay variable.
                            let is_octet_string = idx.ty().map(|t| t.effective_base())
                                == Some(mib_rs::BaseType::OctetString);
                            if !is_octet_string {
                                None
                            } else {
                                idx.object().and_then(|o| {
                                    let sizes = o.effective_sizes();
                                    (sizes.len() == 1
                                        && sizes[0].min == sizes[0].max
                                        && sizes[0].min > 0)
                                        .then(|| sizes[0].min as usize)
                                })
                            }
                        };
                        size.map(IndexEncoding::FixedString)
                            .unwrap_or(IndexEncoding::Variable)
                    }
                    // LengthPrefixed / Unknown: not splittable.
                    _ => IndexEncoding::Variable,
                },
            });
        }

        // Row entries: base entry plus augmented rows (transitively).
        let mut row_entry_oids: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: Vec<mib_rs::Object<'_>> = vec![base_row];
        while let Some(row) = queue.pop() {
            let row_oid = row.node().oid().to_string();
            if !seen.insert(row_oid) {
                continue;
            }
            row_entry_oids.push(row.node().oid().to_string());
            for augmented in row.augmented_by() {
                queue.push(augmented);
            }
        }

        // Columns: every *accessible* column whose containing table is this
        // one, in OID order. Not-accessible index objects have no instances on
        // the wire — fetching them can only produce missing cells — so they are
        // excluded (their values come from decoding the row suffix instead).
        let mut column_oids: Vec<String> = module
            .objects()
            .filter(|o| {
                o.is_column()
                    && o.access() != mib_rs::Access::NotAccessible
                    && o.table().map(|t| t.node().oid().to_string()).as_deref()
                        == Some(table_oid.as_str())
            })
            .map(|o| o.node().oid().to_string())
            .collect();
        column_oids.sort_by(|a, b| oid_numeric_cmp(a, b));

        Some(TableInfo {
            table_oid,
            name,
            row_entry_oids,
            index_columns,
            column_oids,
        })
    }

    /// Wraps non-empty clause text in `Some`, empty in `None`.
    fn non_empty(s: &str) -> Option<String> {
        (!s.is_empty()).then(|| s.to_string())
    }

    /// Formats value constraints from SYNTAX ranges and SIZE clauses, e.g.
    /// `"1..255"`, `"SIZE (0..32)"`, or both joined by `", "`.
    fn format_constraints(
        ranges: &[mib_rs::mib::types::Range],
        sizes: &[mib_rs::mib::types::Range],
    ) -> Option<String> {
        let mut parts = Vec::new();
        for r in ranges {
            parts.push(format!("{}..{}", r.min, r.max));
        }
        for s in sizes {
            parts.push(format!("SIZE ({}..{})", s.min, s.max));
        }
        (!parts.is_empty()).then(|| parts.join(", "))
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

    /// Writes the table-metadata fixture MIB to a temp file and loads it.
    ///
    /// Covers: a two-attribute index (Integer + IpAddress), an IMPLIED
    /// fixed-size string index, an AUGMENTS row adding a column, and a nested
    /// sub-table that must not leak into its outer table's columns.
    fn load_table_meta_fixture(tag: &str) -> crate::LoadResult {
        let tmp_dir = std::env::temp_dir().join(format!("scout_loader_tablemeta_{tag}"));
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mib_content = r#"TABLE-META-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, IpAddress, enterprises, OCTET STRING
        FROM SNMPv2-SMI
    DisplayString
        FROM SNMPv2-TC;

tableMetaMib MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "Table metadata test module."
    ::= { enterprises 99996 }

tableMetaObjects OBJECT IDENTIFIER ::= { tableMetaMib 1 }

pairTable OBJECT-TYPE
    SYNTAX SEQUENCE OF PairEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "A table with a two-attribute index."
    ::= { tableMetaObjects 1 }

pairEntry OBJECT-TYPE
    SYNTAX PairEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Row entry for pairTable."
    INDEX { pairIndex, pairAddr }
    ::= { pairTable 1 }

PairEntry ::= SEQUENCE {
    pairIndex Integer32,
    pairAddr IpAddress,
    pairName DisplayString
}

pairIndex OBJECT-TYPE
    SYNTAX Integer32 (1..2147483647)
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Integer index."
    ::= { pairEntry 1 }

pairAddr OBJECT-TYPE
    SYNTAX IpAddress
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "IpAddress index."
    ::= { pairEntry 2 }

pairName OBJECT-TYPE
    SYNTAX DisplayString (SIZE (0..32))
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "A regular column."
    ::= { pairEntry 3 }

augEntry OBJECT-TYPE
    SYNTAX AugEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Augments pairEntry with an extra column."
    AUGMENTS { pairEntry }
    ::= { pairTable 2 }

AugEntry ::= SEQUENCE {
    augValue Integer32
}

augValue OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "An augmented column."
    ::= { augEntry 1 }

implTable OBJECT-TYPE
    SYNTAX SEQUENCE OF ImplEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "A table with an implied fixed-size string index."
    ::= { tableMetaObjects 2 }

implEntry OBJECT-TYPE
    SYNTAX ImplEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Row entry for implTable."
    INDEX { implIndex, IMPLIED implTag }
    ::= { implTable 1 }

ImplEntry ::= SEQUENCE {
    implIndex Integer32,
    implTag OCTET STRING (SIZE (4))
}

implIndex OBJECT-TYPE
    SYNTAX Integer32 (1..2147483647)
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Integer index."
    ::= { implEntry 1 }

implTag OBJECT-TYPE
    SYNTAX OCTET STRING (SIZE (4))
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Fixed-size implied index."
    ::= { implEntry 2 }

outerTable OBJECT-TYPE
    SYNTAX SEQUENCE OF OuterEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "A table containing a nested sub-table."
    ::= { tableMetaObjects 3 }

outerEntry OBJECT-TYPE
    SYNTAX OuterEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Row entry for outerTable."
    INDEX { outerIndex }
    ::= { outerTable 1 }

OuterEntry ::= SEQUENCE {
    outerIndex Integer32,
    outerValue Integer32
}

outerIndex OBJECT-TYPE
    SYNTAX Integer32 (1..2147483647)
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Outer index."
    ::= { outerEntry 1 }

outerValue OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Outer column."
    ::= { outerEntry 2 }

innerTable OBJECT-TYPE
    SYNTAX SEQUENCE OF InnerEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "A sub-table nested inside outerTable."
    ::= { outerTable 3 }

innerEntry OBJECT-TYPE
    SYNTAX InnerEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Row entry for innerTable."
    INDEX { innerIndex }
    ::= { innerTable 1 }

InnerEntry ::= SEQUENCE {
    innerIndex Integer32,
    innerValue Integer32
}

innerIndex OBJECT-TYPE
    SYNTAX Integer32 (1..2147483647)
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Inner index."
    ::= { innerEntry 1 }

innerValue OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Inner column (not a column of outerTable)."
    ::= { innerEntry 2 }

END
"#;

        let mib_path = tmp_dir.join("TABLE-META-MIB.txt");
        std::fs::write(&mib_path, mib_content).unwrap();

        let mut loader = MibRsLoader::new();
        let result = loader.load_file(&mib_path).expect("should load");
        assert!(result.primary_success);

        let _ = std::fs::remove_dir_all(&tmp_dir);
        result
    }

    fn node_oid<'a>(result: &'a crate::LoadResult, name: &str) -> &'a str {
        result
            .nodes
            .iter()
            .find(|n| n.name == name)
            .unwrap_or_else(|| panic!("node {} not found", name))
            .oid
            .as_str()
    }

    #[test]
    fn table_info_multi_attribute_index() {
        let result = load_table_meta_fixture("multi");
        let info = result
            .tables
            .iter()
            .find(|t| t.name == "pairTable")
            .expect("pairTable metadata");

        // Index columns in INDEX clause order with derived encodings.
        assert_eq!(info.index_columns.len(), 2);
        assert_eq!(info.index_columns[0].name, "pairIndex");
        assert!(!info.index_columns[0].implied);
        assert_eq!(
            info.index_columns[0].encoding,
            crate::IndexEncoding::Integer
        );
        assert_eq!(info.index_columns[0].oid, node_oid(&result, "pairIndex"));

        assert_eq!(info.index_columns[1].name, "pairAddr");
        assert!(!info.index_columns[1].implied);
        assert_eq!(
            info.index_columns[1].encoding,
            crate::IndexEncoding::IpAddress
        );
    }

    #[test]
    fn table_info_augments_contributes_rows_and_columns() {
        let result = load_table_meta_fixture("augments");
        let info = result
            .tables
            .iter()
            .find(|t| t.name == "pairTable")
            .expect("pairTable metadata");

        // Both the base entry and the augmenting row are listed.
        assert_eq!(info.row_entry_oids.len(), 2);
        assert!(info
            .row_entry_oids
            .contains(&node_oid(&result, "pairEntry").to_string()));
        assert!(info
            .row_entry_oids
            .contains(&node_oid(&result, "augEntry").to_string()));

        // Columns include the augmented column, in OID order. Not-accessible
        // index objects are excluded — they have no instances on the wire.
        let expected: Vec<&str> =
            vec![node_oid(&result, "pairName"), node_oid(&result, "augValue")];
        assert_eq!(info.column_oids, expected);
    }

    #[test]
    fn table_info_implied_fixed_string_index() {
        let result = load_table_meta_fixture("implied");
        let info = result
            .tables
            .iter()
            .find(|t| t.name == "implTable")
            .expect("implTable metadata");

        assert_eq!(info.index_columns.len(), 2);
        assert_eq!(info.index_columns[0].name, "implIndex");
        assert_eq!(
            info.index_columns[0].encoding,
            crate::IndexEncoding::Integer
        );

        let tag = &info.index_columns[1];
        assert_eq!(tag.name, "implTag");
        assert!(tag.implied, "IMPLIED flag must be recorded");
        assert_eq!(
            tag.encoding,
            crate::IndexEncoding::FixedString(4),
            "SIZE (4) must yield a fixed-width encoding"
        );
    }

    #[test]
    fn table_info_excludes_nested_subtable_columns() {
        let result = load_table_meta_fixture("nested");
        let info = result
            .tables
            .iter()
            .find(|t| t.name == "outerTable")
            .expect("outerTable metadata");

        // Only outerTable's own accessible columns — the nested innerTable's
        // leaves are not columns of the outer table (G4), and the
        // not-accessible index object is excluded.
        let expected: Vec<&str> = vec![node_oid(&result, "outerValue")];
        assert_eq!(info.column_oids, expected);

        // The nested sub-table has its own metadata.
        let inner = result
            .tables
            .iter()
            .find(|t| t.name == "innerTable")
            .expect("innerTable metadata");
        assert_eq!(inner.column_oids.len(), 1);
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
    #[test]
    fn load_rich_mib_extracts_inspector_details() {
        let tmp_dir = std::env::temp_dir().join("scout_loader_details_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mib_content = r#"DETAILS-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, enterprises
        FROM SNMPv2-SMI;

detailsMib MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "A module with rich clause coverage."
    ::= { enterprises 99995 }

detailsObjects OBJECT IDENTIFIER ::= { detailsMib 1 }

MyString ::= TEXTUAL-CONVENTION
    DISPLAY-HINT "255a"
    STATUS current
    DESCRIPTION "A display-hinted string."
    SYNTAX OCTET STRING (SIZE (0..255))

TestMode ::= INTEGER {
    on(1),
    off(0),
    auto(2)
}

TestFlags ::= BIT STRING {
    flagA(0),
    flagB(1)
}

testName OBJECT-TYPE
    SYNTAX MyString
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "A name."
    ::= { detailsObjects 1 }

testMode OBJECT-TYPE
    SYNTAX TestMode
    UNITS "cycles"
    MAX-ACCESS read-write
    STATUS deprecated
    DESCRIPTION "A mode selector."
    REFERENCE "RFC 1213, MIB-II"
    DEFVAL { off }
    ::= { detailsObjects 2 }

testFlags OBJECT-TYPE
    SYNTAX TestFlags
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Flag bits."
    ::= { detailsObjects 3 }

testBounded OBJECT-TYPE
    SYNTAX Integer32 (1..255)
    MAX-ACCESS not-accessible
    STATUS obsolete
    DESCRIPTION "A bounded integer."
    ::= { detailsObjects 4 }

END
"#;

        let mib_path = tmp_dir.join("DETAILS-MIB.txt");
        std::fs::write(&mib_path, mib_content).unwrap();

        let mut loader = MibRsLoader::new();
        let result = loader.load_file(&mib_path).expect("should load");
        assert!(result.primary_success);

        let by_name = |name: &str| -> &MibNode {
            result
                .nodes
                .iter()
                .find(|n| n.name == name)
                .unwrap_or_else(|| panic!("node {} not found", name))
        };

        // TC display hint + SIZE constraint flow through to the object.
        let name_node = by_name("testName");
        assert_eq!(name_node.display_hint.as_deref(), Some("255a"));
        assert_eq!(name_node.constraints.as_deref(), Some("SIZE (0..255)"));
        assert_eq!(name_node.access.as_deref(), Some("read-only"));
        assert_eq!(name_node.status.as_deref(), Some("current"));
        assert_eq!(name_node.description.as_deref(), Some("A name."));

        // Enum, units, reference, defval, and a non-current status.
        let mode_node = by_name("testMode");
        assert_eq!(mode_node.access.as_deref(), Some("read-write"));
        assert_eq!(mode_node.status.as_deref(), Some("deprecated"));
        assert_eq!(mode_node.units.as_deref(), Some("cycles"));
        assert_eq!(mode_node.reference.as_deref(), Some("RFC 1213, MIB-II"));
        assert_eq!(mode_node.default_value.as_deref(), Some("off"));
        let enum_labels: Vec<&str> = mode_node.enums.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(enum_labels, vec!["on", "off", "auto"]);
        assert_eq!(mode_node.enums[0].value, 1);
        assert_eq!(mode_node.enums[2].value, 2);

        // mib-rs 0.8 does not parse the standard SMIv2 `BIT STRING` type
        // syntax (only a non-standard `BITS` keyword), so a BIT STRING
        // typedef degrades to an unknown base type and its named bits are
        // unavailable. The node itself must still be present — the inspector
        // shows whatever was resolved.
        let flags_node = by_name("testFlags");
        assert!(matches!(flags_node.syntax_type, SyntaxType::Unknown(_)));
        assert!(flags_node.bits.is_empty());

        // Numeric range constraint.
        let bounded_node = by_name("testBounded");
        assert_eq!(bounded_node.constraints.as_deref(), Some("1..255"));
        assert_eq!(bounded_node.access.as_deref(), Some("not-accessible"));
        assert_eq!(bounded_node.status.as_deref(), Some("obsolete"));

        // OBJECT IDENTIFIER subtrees carry description/status/reference too.
        let subtree = by_name("detailsObjects");
        assert_eq!(subtree.syntax_type, SyntaxType::ObjectIdentifier);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    /// The e2e grid and inspector specs depend on test/mibs/SYNTH-TABLE-MIB
    /// (Integer + IpAddress index, IMPLIED component, long enum). Guard the
    /// fixture: if mib-rs stops extracting its tables or enums, the e2e tests
    /// fail obscurely.
    #[test]
    fn synth_table_mib_fixture_extracts_expected_tables() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test/mibs/SYNTH-TABLE-MIB");
        let mut loader = MibRsLoader::new();
        let result = loader.load_file(&path).unwrap();

        assert!(
            result.primary_success,
            "SYNTH-TABLE-MIB must parse via mib-rs"
        );
        let tables: Vec<&crate::TableInfo> = result.tables.iter().collect();
        assert_eq!(tables.len(), 2, "expected both synthetic tables");

        let ip = tables
            .iter()
            .find(|t| t.name == "synthIpTable")
            .expect("synthIpTable");
        assert_eq!(ip.index_columns.len(), 2);
        assert_eq!(ip.index_columns[0].name, "synthIpRow");
        assert!(!ip.index_columns[0].implied);
        assert_eq!(ip.index_columns[1].name, "synthIpAddr");
        assert_eq!(ip.column_oids.len(), 2, "two accessible columns");

        let imp = tables
            .iter()
            .find(|t| t.name == "synthImpTable")
            .expect("synthImpTable");
        assert_eq!(imp.index_columns.len(), 2);
        assert!(!imp.index_columns[0].implied);
        assert_eq!(imp.index_columns[1].name, "synthImpIp");
        assert!(
            imp.index_columns[1].implied,
            "second component must be IMPLIED"
        );
        assert_eq!(imp.column_oids.len(), 1, "one accessible column");

        // The inspector e2e spec depends on synthState's enum list.
        let state = result
            .nodes
            .iter()
            .find(|n| n.name == "synthState")
            .expect("synthState node");
        assert_eq!(state.enums.len(), 6, "all six enum values must extract");
        assert_eq!(state.enums[0].label, "unknown");
        assert_eq!(state.enums[0].value, 0);
        assert_eq!(state.enums.last().unwrap().label, "error");
    }
}
