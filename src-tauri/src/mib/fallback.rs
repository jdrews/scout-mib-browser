use std::path::Path;
use std::sync::OnceLock;
use tracing::{info, warn};

use super::MibNode;

static MODULE_NAME_RE: OnceLock<regex::Regex> = OnceLock::new();
fn module_name_re() -> &'static regex::Regex {
    MODULE_NAME_RE
        .get_or_init(|| regex::Regex::new(r"(?i)\b([A-Za-z0-9_-]+)\s+DEFINITIONS\s*::=").unwrap())
}

static OBJECT_TYPE_BLOCK_RE: OnceLock<regex::Regex> = OnceLock::new();
fn object_type_block_re() -> &'static regex::Regex {
    OBJECT_TYPE_BLOCK_RE.get_or_init(|| {
        regex::Regex::new(r"(?ims)(\b[A-Za-z][A-Za-z0-9_-]*)\s+OBJECT-TYPE\s*\n((?:[^\n]*\n)*)")
            .unwrap()
    })
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

    /// Extracts MIB nodes from a file using regex-based parsing.
    pub fn extract_from_file(&mut self, path: &Path) -> Result<Vec<MibNode>, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

        // Detect module name.
        let mib_name = Self::detect_module_name(&content);
        if mib_name.is_empty() {
            warn!("Fallback: no module name detected in {}", path.display());
            return Ok(Vec::new());
        }

        self.last_mib_name = mib_name.clone();
        info!(
            "Fallback extracting from {} (module: {})",
            path.display(),
            mib_name
        );

        let mut nodes = Vec::new();

        // Strategy 1: Extract OBJECT-TYPE blocks with regex.
        nodes.extend(Self::extract_object_types(&content, &mib_name));

        // Strategy 2: Extract explicit OID assignments (e.g., `name ::= { parent num }`).
        nodes.extend(Self::extract_oid_assignments(&content, &mib_name));

        info!(
            "Fallback extracted {} nodes from {}",
            nodes.len(),
            path.display()
        );

        Ok(nodes)
    }

    /// Detects the MIB module name from file content.
    fn detect_module_name(content: &str) -> String {
        if let Some(captures) = module_name_re().captures(content) {
            if let Some(name_match) = captures.get(1) {
                return name_match.as_str().to_uppercase();
            }
        }
        String::new()
    }

    /// Extracts OBJECT-TYPE definitions using regex.
    ///
    /// Matches patterns like:
    /// ```text
    /// myObject OBJECT-TYPE
    ///     SYNTAX DisplayString
    ///     MAX-ACCESS read-only
    ///     STATUS current
    ///     DESCRIPTION "..."
    ///     ::= { parentSubtree 1 }
    /// ```
    fn extract_object_types(content: &str, mib_name: &str) -> Vec<MibNode> {
        let mut nodes = Vec::new();

        for captures in object_type_block_re().captures_iter(content) {
            let name = captures.get(1).map(|m| m.as_str().to_string());
            let body = captures.get(2).map(|m| m.as_str()).unwrap_or("");

            let name = match name {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };

            // Extract SYNTAX type.
            let syntax_type = Self::extract_syntax(body);

            // Extract OID assignment from ::= { ... } clause.
            let oid = Self::extract_oid_from_assignment(body);

            if !oid.is_empty() {
                nodes.push(MibNode {
                    oid,
                    name,
                    syntax_type,
                    mib_name: mib_name.to_string(),
                });
            } else {
                // No explicit OID — use a placeholder. The node can still be
                // useful for reverse_lookup by name.
                nodes.push(MibNode {
                    oid: format!(".fallback.{}", name),
                    name,
                    syntax_type,
                    mib_name: mib_name.to_string(),
                });
            }
        }

        nodes
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

        let nodes = extractor
            .extract_from_file(&mib_path)
            .expect("should not panic");

        assert!(
            !nodes.is_empty(),
            "Should extract at least some nodes from malformed MIB"
        );
        assert_eq!(extractor.last_mib_name(), "BROKEN-VENDOR-MIB");

        // Check that we found the objects despite missing imports.
        let names: Vec<_> = nodes.iter().map(|n| &n.name).collect();
        assert!(
            names.contains(&&"badObject".to_string())
                || names.contains(&&"weirdObject".to_string())
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
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
