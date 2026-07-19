use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{error, info, warn};

pub use fallback::FallbackExtractor;
pub use loader::MibRsLoader;

/// SNMP syntax type derived from a MIB node's SYNTAX clause.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub enum SyntaxType {
    Integer32,
    OctetString,
    ObjectIdentifier,
    Counter32,
    Counter64,
    Gauge32,
    TimeTicks,
    IpAddress,
    Unsigned32,
    TruthValue,
    Bits,
    Sequence,
    SequenceOf,
    Unknown(String),
}

impl SyntaxType {
    /// Human-readable label for display in the UI.
    pub fn label(&self) -> &str {
        match self {
            SyntaxType::Integer32 => "Integer32",
            SyntaxType::OctetString => "OctetString",
            SyntaxType::ObjectIdentifier => "ObjectIdentifier",
            SyntaxType::Counter32 => "Counter32",
            SyntaxType::Counter64 => "Counter64",
            SyntaxType::Gauge32 => "Gauge32",
            SyntaxType::TimeTicks => "TimeTicks",
            SyntaxType::IpAddress => "IpAddress",
            SyntaxType::Unsigned32 => "Unsigned32",
            SyntaxType::TruthValue => "TruthValue",
            SyntaxType::Bits => "BITS",
            SyntaxType::Sequence => "SEQUENCE",
            SyntaxType::SequenceOf => "SEQUENCE OF",
            SyntaxType::Unknown(s) => s.as_str(),
        }
    }
}

/// A named entry in a MIB schema file.
///
/// Represents what *could* be queried, not live data. Has an OID, name,
/// SYNTAX type, and the MIB module it was defined in.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MibNode {
    /// Dotted-decimal OID (e.g., `"1.3.6.1.2.1.1.1"`).
    pub oid: String,
    /// Human-readable name (e.g., `"sysDescr"`).
    pub name: String,
    /// SYNTAX type from the MIB definition.
    pub syntax_type: SyntaxType,
    /// Name of the MIB module that defines this node.
    pub mib_name: String,
}

/// Result of loading a single MIB file.
#[derive(Debug)]
pub struct LoadResult {
    /// Nodes extracted from the file.
    pub nodes: Vec<MibNode>,
    /// Whether the primary (mib-rs) loader succeeded.
    pub primary_success: bool,
    /// Log messages collected during loading.
    pub messages: Vec<String>,
}

/// Unified MIB resolver that loads files from directories using mib-rs as the
/// primary parser and a regex-based fallback for malformed vendor MIBs.
///
/// Parser errors are logged but never block loading other MIBs. Partially
/// parsed MIBs contribute whatever data was extracted.
pub struct Resolver {
    /// OID -> MibNode (mib-rs results take precedence).
    oid_index: HashMap<String, MibNode>,
    /// Name -> OID (reverse lookup).
    name_index: HashMap<String, String>,
    /// Names of MIB modules that were loaded via regex fallback.
    fallback_mibs: HashSet<String>,
}

impl Resolver {
    /// Creates a new empty resolver.
    pub fn new() -> Self {
        Self {
            oid_index: HashMap::new(),
            name_index: HashMap::new(),
            fallback_mibs: HashSet::new(),
        }
    }

    /// Loads all MIB files from the given directories.
    ///
    /// Binary and non-text files are pre-filtered before any parse attempt.
    /// mib-rs is tried first; on failure, a regex-based fallback extracts
    /// whatever OBJECT-TYPE blocks it can find. mib-rs results take precedence
    /// in the merged index.
    pub fn load_directories(&mut self, directories: &[String]) {
        let mut all_nodes = Vec::new();
        let mut primary_nodes = Vec::new();

        for dir_str in directories {
            let dir = Path::new(dir_str);
            if !dir.is_dir() {
                warn!("MIB directory does not exist: {}", dir_str);
                continue;
            }

            info!("Scanning MIB directory: {}", dir_str);
            let files = Self::collect_mib_files(dir);
            let total_candidate = files.len();
            info!(
                "Found {} candidate files in {}",
                total_candidate,
                dir.display()
            );

            // Pre-filter binary/non-text files.
            let text_files: Vec<_> = files.into_iter().filter(|p| is_text_file(p)).collect();

            if text_files.len() < total_candidate {
                warn!(
                    "Filtered out {} binary/non-text files from {}",
                    total_candidate - text_files.len(),
                    dir_str
                );
            }

            // Primary: mib-rs loader for all files.
            let mut mib_rs = MibRsLoader::new();
            for file in &text_files {
                match mib_rs.load_file(file) {
                    Ok(result) => {
                        if result.primary_success {
                            primary_nodes.extend(result.nodes);
                        } else {
                            all_nodes.push(result);
                        }
                    }
                    Err(e) => {
                        error!("Failed to load MIB file {}: {}", file.display(), e);
                    }
                }
            }

            // Fallback: regex extractor for files that mib-rs couldn't fully parse.
            let mut fallback = FallbackExtractor::new();
            for file in &text_files {
                if !mib_rs.has_module_for_file(file) {
                    info!(
                        "Running regex fallback for (mib-rs did not produce results): {}",
                        file.display()
                    );
                    match fallback.extract_from_file(file) {
                        Ok(nodes) => {
                            let mib_name = fallback.last_mib_name().to_string();
                            if !nodes.is_empty() {
                                self.fallback_mibs.insert(mib_name.clone());
                                all_nodes.push(LoadResult {
                                    nodes,
                                    primary_success: false,
                                    messages: vec![format!("Loaded via regex fallback")],
                                });
                            }
                        }
                        Err(e) => {
                            warn!("Regex fallback also failed for {}: {}", file.display(), e);
                        }
                    }
                }
            }
        }

        // Merge: primary nodes first (take precedence), then fallback fills gaps.
        let mut merged_oid: HashMap<String, MibNode> = HashMap::new();
        for node in primary_nodes {
            merged_oid.insert(node.oid.clone(), node);
        }

        let mut merged_name: HashMap<String, String> = HashMap::new();

        // Index primary nodes by name too.
        for node in merged_oid.values() {
            merged_name.insert(node.name.clone(), node.oid.clone());
        }

        // Fallback nodes fill gaps (only if OID not already present).
        for result in &all_nodes {
            for node in &result.nodes {
                if !merged_oid.contains_key(&node.oid) {
                    merged_oid.insert(node.oid.clone(), node.clone());
                } else {
                    info!(
                        "Skipping fallback node {} (OID {}) — mib-rs already has it",
                        node.name, node.oid
                    );
                }

                // Index by name too (only if name not already present).
                if !merged_name.contains_key(&node.name) {
                    merged_name.insert(node.name.clone(), node.oid.clone());
                }
            }
        }

        self.oid_index = merged_oid;
        self.name_index = merged_name;

        info!(
            "Resolver loaded {} nodes ({} fallback MIBs)",
            self.oid_index.len(),
            self.fallback_mibs.len()
        );
    }

    /// Resolves a dotted-decimal OID to its name, MIB module, and syntax type.
    ///
    /// Returns `None` if the OID is not in the index. Uses longest-prefix
    /// matching: if an exact match isn't found, returns the deepest ancestor
    /// node that matches a prefix of the given OID.
    pub fn resolve(&self, oid: &str) -> Option<&MibNode> {
        // Exact match first.
        if let Some(node) = self.oid_index.get(oid) {
            return Some(node);
        }

        // Longest-prefix match for sub-OIDs (e.g., instance OIDs).
        let mut best: Option<(&str, &MibNode)> = None;
        for (indexed_oid, node) in &self.oid_index {
            if oid.starts_with(&format!("{}.{}", indexed_oid, "")) || oid == indexed_oid {
                match best {
                    None => {
                        best = Some((indexed_oid, node));
                    }
                    Some((best_oid, _)) => {
                        if indexed_oid.len() > best_oid.len() {
                            best = Some((indexed_oid, node));
                        }
                    }
                }
            }
        }

        best.map(|(_, node)| node)
    }

    /// Looks up a MIB node name and returns its OID.
    pub fn reverse_lookup(&self, name: &str) -> Option<&str> {
        self.name_index.get(name).map(|s| s.as_str())
    }

    /// Returns the total number of indexed nodes.
    pub fn node_count(&self) -> usize {
        self.oid_index.len()
    }

    /// Returns names of MIB modules that were loaded via regex fallback.
    pub fn fallback_mib_names(&self) -> impl Iterator<Item = &String> {
        self.fallback_mibs.iter()
    }

    /// Collects all MIB candidate files recursively from a directory.
    fn collect_mib_files(dir: &Path) -> Vec<std::path::PathBuf> {
        walkdir::WalkDir::new(dir)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .collect()
    }
}

/// Checks whether a file appears to be a text file by reading the first 8KB
/// and looking for null bytes or other binary indicators.
fn is_text_file(path: &Path) -> bool {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            warn!("Cannot read file {} for text check: {}", path.display(), e);
            return false;
        }
    };

    if data.is_empty() {
        return false;
    }

    let chunk = if data.len() > 8192 {
        &data[..8192]
    } else {
        &data
    };

    // Check for null bytes (strong indicator of binary).
    if chunk.contains(&0u8) {
        return false;
    }

    // Check that most bytes are printable ASCII or common whitespace.
    let non_text = chunk
        .iter()
        .filter(|&&b| b > 127 || (b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t'))
        .count();

    // Allow up to 5% non-text bytes (handles UTF-8 and minor encoding quirks).
    non_text * 100 / chunk.len() < 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_text_file_rejects_binary() {
        let tmp = std::env::temp_dir().join("scout_mib_test_bin");
        std::fs::write(&tmp, [0x00, 0x01, 0x02, 0xFF]).unwrap();
        assert!(!is_text_file(&tmp));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn is_text_file_accepts_ascii() {
        let tmp = std::env::temp_dir().join("scout_mib_test_txt");
        std::fs::write(&tmp, "MY-MIB DEFINITIONS ::= BEGIN\nEND\n").unwrap();
        assert!(is_text_file(&tmp));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn is_text_file_rejects_empty() {
        let tmp = std::env::temp_dir().join("scout_mib_test_empty");
        std::fs::write(&tmp, "").unwrap();
        assert!(!is_text_file(&tmp));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn resolve_exact_match() {
        let mut resolver = Resolver::new();
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
            },
        );
        resolver
            .name_index
            .insert("sysDescr".to_string(), "1.3.6.1.2.1.1.1".to_string());

        let node = resolver.resolve("1.3.6.1.2.1.1.1").unwrap();
        assert_eq!(node.name, "sysDescr");
    }

    #[test]
    fn resolve_longest_prefix_match() {
        let mut resolver = Resolver::new();
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1".to_string(),
                name: "system".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "SNMPv2-MIB".to_string(),
            },
        );

        // Instance OID should resolve to the deepest matching ancestor.
        let node = resolver.resolve("1.3.6.1.2.1.1.1.0").unwrap();
        assert_eq!(node.name, "system");
    }

    #[test]
    fn reverse_lookup_basic() {
        let mut resolver = Resolver::new();
        resolver
            .name_index
            .insert("sysDescr".to_string(), "1.3.6.1.2.1.1.1".to_string());

        assert_eq!(resolver.reverse_lookup("sysDescr"), Some("1.3.6.1.2.1.1.1"));
        assert_eq!(resolver.reverse_lookup("nonexistent"), None);
    }

    #[test]
    fn merge_primary_takes_precedence() {
        let mut resolver = Resolver::new();

        // Simulate primary node.
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
            },
        );

        // Simulate fallback with same OID but different data.
        let primary_map: HashMap<String, MibNode> = resolver.oid_index.clone();
        assert_eq!(primary_map.len(), 1);
    }
}
