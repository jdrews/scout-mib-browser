use std::path::Path;
use std::sync::OnceLock;
use tracing::{info, warn};

use super::{IndexColumn, IndexEncoding, MibNode, SyntaxType, TableInfo};

static OBJECT_TYPE_HEADER_RE: OnceLock<regex::Regex> = OnceLock::new();
fn object_type_header_re() -> &'static regex::Regex {
    OBJECT_TYPE_HEADER_RE.get_or_init(|| {
        regex::Regex::new(r"(?m)^\s*([A-Za-z][A-Za-z0-9_-]*)\s+OBJECT-TYPE[ \t]*\r?\n").unwrap()
    })
}

static SEQUENCE_OF_RE: OnceLock<regex::Regex> = OnceLock::new();
fn sequence_of_re() -> &'static regex::Regex {
    SEQUENCE_OF_RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\bSYNTAX\s+SEQUENCE\s+OF\s+([A-Za-z][A-Za-z0-9_]*)").unwrap()
    })
}

static INDEX_CLAUSE_RE: OnceLock<regex::Regex> = OnceLock::new();
fn index_clause_re() -> &'static regex::Regex {
    INDEX_CLAUSE_RE.get_or_init(|| regex::Regex::new(r"(?i)\bINDEX\s*\{([^}]*)\}").unwrap())
}

/// One OBJECT-TYPE block recovered by the fallback scan.
struct ObjectTypeBlock {
    name: String,
    body: String,
}

static OID_ASSIGNMENT_RE: OnceLock<regex::Regex> = OnceLock::new();
fn oid_assignment_re() -> &'static regex::Regex {
    OID_ASSIGNMENT_RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(\b[A-Za-z][A-Za-z0-9_-]*)\s+(?:OBJECT\s+IDENTIFIER\s+|MODULE-IDENTITY\s+|NOTIFICATION-TYPE\s+)?::=\s*\{\s*([A-Za-z][A-Za-z0-9_.-]*)\s+(\d+)\s*\}",
        )
        .unwrap()
    })
}

static SYNTAX_RE: OnceLock<regex::Regex> = OnceLock::new();
fn syntax_re() -> &'static regex::Regex {
    SYNTAX_RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\bSYNTAX\s+([A-Za-z][A-Za-z0-9_]*(?:\s*\([^)]*\))?)").unwrap()
    })
}

static OID_FROM_ASSIGNMENT_RE: OnceLock<regex::Regex> = OnceLock::new();
fn oid_from_assignment_re() -> &'static regex::Regex {
    OID_FROM_ASSIGNMENT_RE.get_or_init(|| {
        regex::Regex::new(r"::=\s*\{\s*([A-Za-z][A-Za-z0-9_.-]*)\s+(\d+)\s*\}").unwrap()
    })
}

/// Regex-based fallback extractor for MIB files that mib-rs cannot parse.
///
/// Pulls OBJECT-TYPE blocks, name/SYNTAX mappings, and explicit numeric OID
/// assignments from malformed vendor MIBs. This is a best-effort parser that
/// tolerates syntax errors, missing imports, and non-standard constructs.
#[derive(Default)]
pub struct FallbackExtractor {
    /// Name of the last successfully parsed MIB module.
    last_mib_name: String,
}

impl FallbackExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the name of the last MIB module that was extracted.
    pub fn last_mib_name(&self) -> &str {
        &self.last_mib_name
    }

    /// Extracts MIB nodes and table metadata from a file using regex-based parsing.
    pub fn extract_from_file(
        &mut self,
        path: &Path,
    ) -> Result<(Vec<MibNode>, Vec<TableInfo>), String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
        // Strip comment lines so regexes don't match keywords inside `--` comments.
        let content = Self::strip_comments(&raw);

        // Detect module name.
        let mib_name = Self::detect_module_name(&content);
        if mib_name.is_empty() {
            warn!("Fallback: no module name detected in {}", path.display());
            return Ok((Vec::new(), Vec::new()));
        }

        self.last_mib_name = mib_name.clone();
        info!(
            "Fallback extracting from {} (module: {})",
            path.display(),
            mib_name
        );

        // Strategy 1: Extract OBJECT-TYPE blocks with regex.
        let blocks = Self::parse_object_type_blocks(&content);
        let tables = Self::detect_tables(&blocks);
        let mut nodes = Self::nodes_from_blocks(&blocks, &mib_name, &tables);

        // Strategy 2: Extract explicit OID assignments (e.g., `name ::= { parent num }`).
        nodes.extend(Self::extract_oid_assignments(&content, &mib_name));

        info!(
            "Fallback extracted {} nodes ({} tables) from {}",
            nodes.len(),
            tables.len(),
            path.display()
        );

        Ok((nodes, tables))
    }

    /// Splits the content into OBJECT-TYPE blocks.
    ///
    /// A block starts at a line of the form `name OBJECT-TYPE` and runs up to
    /// (not including) the next such line, or end of input. The previous
    /// single-regex approach used a greedy body that swallowed every block
    /// after the first; explicit boundary scanning fixes that.
    fn parse_object_type_blocks(content: &str) -> Vec<ObjectTypeBlock> {
        let matches: Vec<_> = object_type_header_re().captures_iter(content).collect();
        let mut blocks = Vec::new();

        for (i, caps) in matches.iter().enumerate() {
            let name = match caps.get(1) {
                Some(n) if !n.as_str().is_empty() => n.as_str().to_string(),
                _ => continue,
            };
            // The header match includes its trailing newline, so the body
            // starts right at the end of the match.
            let body_start = match caps.get(0) {
                Some(m) => m.end(),
                None => continue,
            };
            let body_end = matches
                .get(i + 1)
                .and_then(|c| c.get(0))
                .map(|m| m.start())
                .unwrap_or(content.len());

            blocks.push(ObjectTypeBlock {
                name,
                body: content[body_start..body_end].to_string(),
            });
        }

        blocks
    }

    /// Best-effort table detection across OBJECT-TYPE blocks.
    ///
    /// An OBJECT-TYPE with `SYNTAX SEQUENCE OF X` whose corresponding entry
    /// (an OBJECT-TYPE with `SYNTAX X`) carries an `INDEX { … }` clause marks
    /// a table. Only index *names* are recorded — the fallback has no type
    /// resolution, so every component is encoded as [`IndexEncoding::Variable`]
    /// and instance decoding degrades to the raw suffix (tolerance path).
    fn detect_tables(blocks: &[ObjectTypeBlock]) -> Vec<TableInfo> {
        let mut tables = Vec::new();

        for block in blocks {
            let entry_type = match sequence_of_re().captures(&block.body) {
                Some(caps) => caps.get(1).map(|m| m.as_str().to_string()),
                None => continue,
            };
            let Some(entry_type) = entry_type else {
                continue;
            };

            // The entry is the block whose SYNTAX names the SEQUENCE type and
            // which declares an INDEX clause.
            let entry = match blocks.iter().find(|b| {
                Self::syntax_type_name(&b.body).is_some_and(|s| s.eq_ignore_ascii_case(&entry_type))
                    && index_clause_re().is_match(&b.body)
            }) {
                Some(e) => e,
                None => continue,
            };

            let index_clause = match index_clause_re().captures(&entry.body) {
                Some(caps) => caps.get(1).map(|m| m.as_str().to_string()),
                None => continue,
            };
            let Some(index_clause) = index_clause else {
                continue;
            };

            tables.push(TableInfo {
                table_oid: Self::block_oid(block),
                name: block.name.clone(),
                row_entry_oids: vec![Self::block_oid(entry)],
                index_columns: Self::parse_index_clause(&index_clause),
                column_oids: Vec::new(),
            });
        }

        tables
    }

    /// Parses the contents of an `INDEX { … }` clause into index columns.
    ///
    /// Each comma-separated item is `name [IMPLIED]`; encoding is always
    /// [`IndexEncoding::Variable`] (no type resolution in the fallback).
    fn parse_index_clause(clause: &str) -> Vec<IndexColumn> {
        clause
            .split(',')
            .filter_map(|item| {
                let tokens: Vec<&str> = item.split_whitespace().collect();
                // SMIv2 writes the keyword first: `IMPLIED name` (also accept
                // the reversed order some vendors use).
                let implied = tokens.iter().any(|t| t.eq_ignore_ascii_case("IMPLIED"));
                let name = tokens
                    .into_iter()
                    .find(|t| !t.eq_ignore_ascii_case("IMPLIED"))?;
                Some(IndexColumn {
                    name: name.to_string(),
                    oid: String::new(),
                    implied,
                    encoding: IndexEncoding::Variable,
                })
            })
            .collect()
    }

    /// The OID for a block: its `::= { parent num }` assignment, or a
    /// `.fallback.<name>` placeholder when absent (keeps nodes and table
    /// metadata consistent).
    fn block_oid(block: &ObjectTypeBlock) -> String {
        match Self::extract_oid_from_assignment(&block.body) {
            oid if !oid.is_empty() => oid,
            _ => format!(".fallback.{}", block.name),
        }
    }

    /// Returns the first token of the block's SYNTAX clause (e.g. `"TestEntry"`).
    fn syntax_type_name(body: &str) -> Option<String> {
        syntax_re()
            .captures(body)
            .and_then(|caps| caps.get(1))
            .map(|m| {
                m.as_str()
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .filter(|s| !s.is_empty())
    }

    /// Builds MibNodes from OBJECT-TYPE blocks, marking detected tables.
    fn nodes_from_blocks(
        blocks: &[ObjectTypeBlock],
        mib_name: &str,
        tables: &[TableInfo],
    ) -> Vec<MibNode> {
        let mut nodes = Vec::new();

        for block in blocks {
            let is_table = tables.iter().any(|t| t.name == block.name);
            let syntax_type = if is_table {
                SyntaxType::Table
            } else {
                Self::extract_syntax(&block.body)
            };
            // No explicit OID — use a placeholder. The node can still be
            // useful for reverse_lookup by name.
            let oid = match Self::extract_oid_from_assignment(&block.body) {
                oid if !oid.is_empty() => oid,
                _ => format!(".fallback.{}", block.name),
            };

            nodes.push(MibNode {
                oid,
                name: block.name.clone(),
                syntax_type,
                mib_name: mib_name.to_string(),
                is_table,
            });
        }

        nodes
    }

    /// Extracts OBJECT-TYPE definitions using regex (test-facing convenience).
    #[cfg(test)]
    fn extract_object_types(content: &str, mib_name: &str) -> Vec<MibNode> {
        let blocks = Self::parse_object_type_blocks(content);
        let tables = Self::detect_tables(&blocks);
        Self::nodes_from_blocks(&blocks, mib_name, &tables)
    }

    /// Detects the MIB module name from file content.
    fn detect_module_name(content: &str) -> String {
        super::detect_module_name(content)
    }

    /// Removes MIB comment lines (`--` through end of line). Without this,
    /// a comment mentioning e.g. "OBJECT-TYPE" produces a garbage node.
    fn strip_comments(content: &str) -> String {
        content
            .lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Extracts explicit OID assignments like `name ::= { parent num }`.
    fn extract_oid_assignments(content: &str, mib_name: &str) -> Vec<MibNode> {
        let mut nodes = Vec::new();

        for captures in oid_assignment_re().captures_iter(content) {
            let name = captures.get(1).map(|m| m.as_str().to_string()).unwrap();
            let parent = captures.get(2).map(|m| m.as_str().to_string()).unwrap();
            let suffix = captures.get(3).map(|m| m.as_str().to_string()).unwrap();

            // Skip if this looks like an OBJECT-TYPE (already handled above).
            let preceding = Self::text_before(content, &name);
            if preceding.to_lowercase().contains("object-type")
                || preceding.to_lowercase().contains("notification-type")
            {
                continue;
            }

            // Resolve parent OID if it's a known root.
            let oid = Self::resolve_oid_assignment(&parent, &suffix);

            nodes.push(MibNode {
                oid,
                name,
                syntax_type: super::SyntaxType::ObjectIdentifier,
                mib_name: mib_name.to_string(),
                is_table: false,
            });
        }

        nodes
    }

    /// Extracts the SYNTAX type from an OBJECT-TYPE body.
    fn extract_syntax(body: &str) -> super::SyntaxType {
        if let Some(captures) = syntax_re().captures(body) {
            if let Some(syntax_match) = captures.get(1) {
                let syntax_str = syntax_match
                    .as_str()
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                return Self::parse_syntax_name(syntax_str);
            }
        }
        super::SyntaxType::Unknown("unknown".to_string())
    }

    /// Parses a SYNTAX type name into our SyntaxType enum.
    fn parse_syntax_name(name: &str) -> super::SyntaxType {
        match name.to_uppercase().as_str() {
            "INTEGER32" | "INTEGER" => super::SyntaxType::Integer32,
            "OCTETSTRING" | "OCTET STRING" => super::SyntaxType::OctetString,
            "OBJECTIDENTIFIER" | "OBJECT IDENTIFIER" => super::SyntaxType::ObjectIdentifier,
            "COUNTER32" => super::SyntaxType::Counter32,
            "COUNTER64" => super::SyntaxType::Counter64,
            "GAUGE32" | "GAUGE" => super::SyntaxType::Gauge32,
            "TIMETICKS" | "TIME TICKS" => super::SyntaxType::TimeTicks,
            "IPADDRESS" | "IP ADDRESS" => super::SyntaxType::IpAddress,
            "UNSIGNED32" => super::SyntaxType::Unsigned32,
            "TRUTHVALUE" => super::SyntaxType::TruthValue,
            "BITS" => super::SyntaxType::Bits,
            _ => {
                // Could be a textual convention — return as-is.
                super::SyntaxType::Unknown(name.to_string())
            }
        }
    }

    /// Extracts the OID from a `::= { parent num }` assignment clause.
    fn extract_oid_from_assignment(body: &str) -> String {
        if let Some(captures) = oid_from_assignment_re().captures(body) {
            let parent = captures.get(1).map(|m| m.as_str().to_string()).unwrap();
            let suffix = captures.get(2).map(|m| m.as_str().to_string()).unwrap();
            return Self::resolve_oid_assignment(&parent, &suffix);
        }
        String::new()
    }

    /// Resolves a parent name + numeric suffix to a dotted-decimal OID.
    fn resolve_oid_assignment(parent: &str, suffix: &str) -> String {
        // Known well-known OID roots.
        let known_roots: std::collections::HashMap<&str, &str> = [
            ("iso", "1"),
            ("ccitt", "2"),
            ("joint-iso-ccitt", "0"),
            ("org", "3"),
            ("dod", "6"),
            ("internet", "1.3.6.1"),
            ("directory", "1.3.6.1.1"),
            ("mgmt", "1.3.6.1.2"),
            ("mib-2", "1.3.6.1.2.1"),
            ("experimental", "1.3.6.1.3"),
            ("private", "1.3.6.1.4"),
            ("enterprises", "1.3.6.1.4.1"),
        ]
        .into_iter()
        .collect();

        let parent_lower = parent.to_lowercase();

        if let Some(root_oid) = known_roots.get(parent_lower.as_str()) {
            format!("{}.{}", root_oid, suffix)
        } else {
            // Unknown parent — use the parent name as a placeholder prefix.
            format!(".unknown.{}.{}", parent, suffix)
        }
    }

    /// Gets the text preceding a given token (for context analysis).
    fn text_before(content: &str, token: &str) -> String {
        if let Some(pos) = content.find(token) {
            let start = pos.saturating_sub(200);
            content[start..pos].to_lowercase()
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_object_type_basic() {
        let content = r#"VENDOR-MIB DEFINITIONS ::= BEGIN
IMPORTS
    OBJECT-TYPE, enterprises FROM SNMPv2-SMI;

vendorMib MODULE-IDENTITY
    ::= { enterprises 99997 }

myObject OBJECT-TYPE
    SYNTAX DisplayString (SIZE (0..255))
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "A test object."
    ::= { vendorMib 1 }

END
"#;

        let nodes = FallbackExtractor::extract_object_types(content, "VENDOR-MIB");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "myObject");
        assert_eq!(nodes[0].mib_name, "VENDOR-MIB");
    }

    /// A vendor-style MIB with a SEQUENCE OF table and an entry carrying an
    /// INDEX clause (including an IMPLIED component) must yield one table.
    #[test]
    fn extract_vendor_table_with_index() {
        let content = r#"VENDOR-MIB DEFINITIONS ::= BEGIN
IMPORTS
    OBJECT-TYPE, enterprises FROM SNMPv2-SMI;

vendorMib MODULE-IDENTITY
    ::= { enterprises 99997 }

vendorObjects OBJECT IDENTIFIER ::= { vendorMib 1 }

devTable OBJECT-TYPE
    SYNTAX SEQUENCE OF DevEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "A device table."
    ::= { vendorObjects 2 }

DevEntry ::= SEQUENCE {
    devIndex INTEGER,
    devMac OCTET STRING (SIZE (6)),
    devName DisplayString
}

devEntry OBJECT-TYPE
    SYNTAX DevEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "A device row."
    INDEX { devIndex, IMPLIED devMac }
    ::= { devTable 1 }

devIndex OBJECT-TYPE
    SYNTAX INTEGER
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Row index."
    ::= { devEntry 1 }

devMac OBJECT-TYPE
    SYNTAX OCTET STRING (SIZE (6))
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "Implied MAC index."
    ::= { devEntry 2 }

devName OBJECT-TYPE
    SYNTAX DisplayString (SIZE (0..32))
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Device name."
    ::= { devEntry 3 }

END
"#;

        let mut extractor = FallbackExtractor::new();
        let tmp_dir = std::env::temp_dir().join("scout_fallback_table_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mib_path = tmp_dir.join("VENDOR-TABLE-MIB.txt");
        std::fs::write(&mib_path, content).unwrap();

        let (nodes, tables) = extractor
            .extract_from_file(&mib_path)
            .expect("should not panic");

        // The table node is flagged.
        let table_node = nodes
            .iter()
            .find(|n| n.name == "devTable")
            .expect("devTable node");
        assert!(table_node.is_table);
        assert_eq!(table_node.syntax_type, SyntaxType::Table);

        // Exactly one table detected, with both index components in order.
        assert_eq!(tables.len(), 1);
        let info = &tables[0];
        assert_eq!(info.name, "devTable");
        assert_eq!(info.table_oid, table_node.oid);
        assert_eq!(info.index_columns.len(), 2);

        assert_eq!(info.index_columns[0].name, "devIndex");
        assert!(!info.index_columns[0].implied);
        assert_eq!(info.index_columns[0].encoding, IndexEncoding::Variable);

        assert_eq!(info.index_columns[1].name, "devMac");
        assert!(info.index_columns[1].implied);
        assert_eq!(info.index_columns[1].encoding, IndexEncoding::Variable);

        // Fallback has no type resolution: column list stays empty so the
        // resolver falls back to its leaf heuristic.
        assert!(info.column_oids.is_empty());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn extract_oid_assignment() {
        let content = r#"VENDOR-MIB DEFINITIONS ::= BEGIN
IMPORTS enterprises FROM SNMPv2-SMI;

vendorMib MODULE-IDENTITY
    ::= { enterprises 99997 }

mySubtree OBJECT IDENTIFIER ::= { vendorMib 1 }

END
"#;

        let nodes = FallbackExtractor::extract_oid_assignments(content, "VENDOR-MIB");
        // Should find vendorMib (MODULE-IDENTITY) and mySubtree (OBJECT IDENTIFIER).
        assert!(nodes.len() >= 2);

        let names: Vec<_> = nodes.iter().map(|n| &n.name).collect();
        assert!(names.contains(&&"vendorMib".to_string()));
        assert!(names.contains(&&"mySubtree".to_string()));

        let subtree = nodes.iter().find(|n| n.name == "mySubtree").unwrap();
        assert_eq!(
            subtree.syntax_type,
            super::super::SyntaxType::ObjectIdentifier
        );
    }

    #[test]
    fn extract_malformed_vendor_mib() {
        // Simulates a real-world malformed vendor MIB with missing imports,
        // non-standard syntax, and broken clauses.
        let content = r#"BROKEN-VENDOR-MIB DEFINITIONS ::= BEGIN
-- Missing IMPORTS clause entirely

brokenMib MODULE-IDENTITY
    LAST-UPDATED "202501010000Z"
    ORGANIZATION "Broken Vendor Inc"
    CONTACT-INFO "nobody@nowhere.com"
    DESCRIPTION "This MIB has issues."
    ::= { enterprises 54321 }

-- Missing SYNTAX clause
badObject OBJECT-TYPE
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Missing syntax."
    ::= { brokenMib 1 }

-- Has extra whitespace and odd formatting
weirdObject   OBJECT-TYPE
    SYNTAX     Integer32
        MAX-ACCESS read-write
    STATUS obsolete
    DESCRIPTION "Oddly formatted."
    ::= { brokenMib 2 }

END
"#;

        let mut extractor = FallbackExtractor::new();
        let tmp_dir = std::env::temp_dir().join("scout_fallback_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mib_path = tmp_dir.join("BROKEN-VENDOR-MIB.txt");
        std::fs::write(&mib_path, content).unwrap();

        let (nodes, tables) = extractor
            .extract_from_file(&mib_path)
            .expect("should not panic");

        assert!(
            !nodes.is_empty(),
            "Should extract at least some nodes from malformed MIB"
        );
        assert_eq!(extractor.last_mib_name(), "BROKEN-VENDOR-MIB");
        assert!(tables.is_empty(), "no tables in this fixture");

        // Check that we found every OBJECT-TYPE block despite missing imports.
        let names: Vec<_> = nodes.iter().map(|n| &n.name).collect();
        assert!(names.contains(&&"badObject".to_string()));
        assert!(names.contains(&&"weirdObject".to_string()));

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn comments_do_not_produce_nodes() {
        let content = r#"COMMENT-MIB DEFINITIONS ::= BEGIN

-- This module documents the OBJECT-TYPE blocks below.
realObject OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Real."
    ::= { commentRoot 1 }

END
"#;

        let nodes = FallbackExtractor::extract_object_types(content, "COMMENT-MIB");
        let names: Vec<_> = nodes.iter().map(|n| &n.name).collect();
        assert!(names.contains(&&"realObject".to_string()));
        assert!(
            !names.contains(&&"This".to_string()),
            "comment keyword leaked into nodes"
        );
    }

    #[test]
    fn strip_comments_removes_dash_lines() {
        let stripped = FallbackExtractor::strip_comments("-- a\nkeep -- inline\n  -- b\n");
        assert_eq!(stripped, "keep -- inline");
    }

    #[test]
    fn parse_syntax_name() {
        assert_eq!(
            FallbackExtractor::parse_syntax_name("Integer32"),
            super::super::SyntaxType::Integer32
        );
        assert_eq!(
            FallbackExtractor::parse_syntax_name("DisplayString"),
            super::super::SyntaxType::Unknown("DisplayString".to_string())
        );
        assert_eq!(
            FallbackExtractor::parse_syntax_name("OCTET STRING"),
            super::super::SyntaxType::OctetString
        );
    }

    #[test]
    fn resolve_oid_assignment_known_root() {
        let oid = FallbackExtractor::resolve_oid_assignment("enterprises", "12345");
        assert_eq!(oid, "1.3.6.1.4.1.12345");

        let oid = FallbackExtractor::resolve_oid_assignment("internet", "1");
        assert_eq!(oid, "1.3.6.1.1");
    }

    #[test]
    fn resolve_oid_assignment_unknown_parent() {
        let oid = FallbackExtractor::resolve_oid_assignment("customParent", "5");
        assert!(oid.starts_with(".unknown.customParent."));
    }
}
