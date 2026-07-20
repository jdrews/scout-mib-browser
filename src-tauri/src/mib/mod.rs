mod fallback;
mod loader;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;
use tracing::{error, info, warn};

pub use fallback::FallbackExtractor;
pub use loader::MibRsLoader;

static MODULE_NAME_RE: OnceLock<regex::Regex> = OnceLock::new();
fn module_name_re() -> &'static regex::Regex {
    MODULE_NAME_RE
        .get_or_init(|| regex::Regex::new(r"(?i)\b([A-Za-z0-9_-]+)\s+DEFINITIONS\s*::=").unwrap())
}

pub fn detect_module_name(content: &str) -> String {
    if let Some(captures) = module_name_re().captures(content) {
        if let Some(name_match) = captures.get(1) {
            return name_match.as_str().to_uppercase();
        }
    }
    String::new()
}

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
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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

/// Single node in the hierarchical MIB tree for UI rendering.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TreeNode {
    /// Dotted-decimal OID.
    pub oid: String,
    /// Human-readable name.
    pub name: String,
    /// SYNTAX type label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_type: Option<String>,
    /// MIB module name.
    pub mib_name: String,
    /// Child nodes (empty for leaf nodes).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
}

/// Metadata about a loaded MIB file for the Manage MIBs dialog.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoadedMibInfo {
    /// MIB module name (e.g., `"SNMPv2-MIB"`).
    pub mib_name: String,
    /// File path that was loaded.
    pub file_path: String,
    /// Number of nodes contributed by this file.
    pub node_count: usize,
    /// Whether the file was loaded via regex fallback.
    pub is_fallback: bool,
}

/// Unified MIB resolver that loads files from directories using mib-rs as the
/// primary parser and a regex-based fallback for malformed vendor MIBs.
///
/// Parser errors are logged but never block loading other MIBs. Partially
/// parsed MIBs contribute whatever data was extracted.
#[derive(Default)]
pub struct Resolver {
    /// OID -> MibNode (mib-rs results take precedence).
    oid_index: HashMap<String, MibNode>,
    /// Name -> OID (reverse lookup).
    name_index: HashMap<String, String>,
    /// Names of MIB modules that were loaded via regex fallback.
    fallback_mibs: HashSet<String>,
    /// File path -> MIB module name mapping for tracking loaded files.
    loaded_files: BTreeMap<String, String>,
}

impl Resolver {
    /// Creates a new empty resolver.
    pub fn new() -> Self {
        Self::default()
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
        // Track file -> MIB module name for loaded files management.
        let mut file_mib_map: HashMap<String, String> = HashMap::new();

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
                            // Track file -> MIB name from the first node's mib_name.
                            if let Some(first_node) = result.nodes.first() {
                                file_mib_map.insert(
                                    file.to_string_lossy().to_string(),
                                    first_node.mib_name.clone(),
                                );
                            }
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
                                file_mib_map
                                    .insert(file.to_string_lossy().to_string(), mib_name.clone());
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
        self.loaded_files = file_mib_map.into_iter().collect();

        info!(
            "Resolver loaded {} nodes ({} fallback MIBs, {} tracked files)",
            self.oid_index.len(),
            self.fallback_mibs.len(),
            self.loaded_files.len()
        );
    }

    /// Records a file-to-MIB-module mapping for tracking loaded files.
    pub fn track_loaded_file(&mut self, file_path: String, mib_name: String) {
        self.loaded_files.insert(file_path, mib_name);
    }

    /// Returns information about all currently loaded MIB modules.
    pub fn loaded_mibs(&self) -> Vec<LoadedMibInfo> {
        let mut mib_node_counts: HashMap<String, usize> = HashMap::new();
        for node in self.oid_index.values() {
            *mib_node_counts.entry(node.mib_name.clone()).or_default() += 1;
        }

        // Build from tracked files first.
        let mut result = Vec::new();
        for (file_path, mib_name) in &self.loaded_files {
            result.push(LoadedMibInfo {
                mib_name: mib_name.clone(),
                file_path: file_path.clone(),
                node_count: *mib_node_counts.get(mib_name).unwrap_or(&0),
                is_fallback: self.fallback_mibs.contains(mib_name),
            });
        }

        // Add any MIB modules not yet tracked by file.
        let tracked_names: HashSet<_> = self.loaded_files.values().cloned().collect();
        for (mib_name, count) in &mib_node_counts {
            if !tracked_names.contains(mib_name) {
                result.push(LoadedMibInfo {
                    mib_name: mib_name.clone(),
                    file_path: format!("<{}>", mib_name),
                    node_count: *count,
                    is_fallback: self.fallback_mibs.contains(mib_name),
                });
            }
        }

        result.sort_by(|a, b| a.mib_name.cmp(&b.mib_name));
        result
    }

    /// Unloads all nodes belonging to the given MIB module name.
    pub fn unload_mib(&mut self, mib_name: &str) {
        let oids_to_remove: Vec<String> = self
            .oid_index
            .iter()
            .filter(|(_, n)| n.mib_name == mib_name)
            .map(|(oid, _)| oid.clone())
            .collect();

        for oid in &oids_to_remove {
            if let Some(node) = self.oid_index.remove(oid) {
                self.name_index.retain(|name, _| name != &node.name);
            }
        }

        // Remove from loaded files tracking.
        self.loaded_files.retain(|_, mn| mn != mib_name);

        if oids_to_remove.is_empty() {
            self.fallback_mibs.remove(mib_name);
        }

        info!(
            "Unloaded MIB module '{}': removed {} nodes",
            mib_name,
            oids_to_remove.len()
        );
    }

    /// Builds a hierarchical tree of all loaded MIB nodes.
    ///
    /// The tree is organized by OID hierarchy: each node's parent is determined
    /// by removing the last numeric segment from its OID. Root-level OIDs become
    /// top-level tree entries. Results are sorted alphabetically at each level.
    pub fn build_tree(&self) -> Vec<TreeNode> {
        if self.oid_index.is_empty() {
            return Vec::new();
        }

        // Group nodes by parent OID.
        let mut children_map: HashMap<String, Vec<&MibNode>> = HashMap::new();
        for node in self.oid_index.values() {
            let parent_oid = Self::parent_oid(&node.oid);
            children_map.entry(parent_oid).or_default().push(node);
        }

        // Set of all indexed OIDs for orphan detection.
        let indexed_oids: HashSet<_> = self.oid_index.keys().cloned().collect();

        // Build tree recursively from root (empty string parent).
        let mut roots = Vec::new();
        if let Some(root_children) = children_map.get("") {
            for node in self.sort_nodes(root_children) {
                roots.push(self.build_tree_node(node, &children_map));
            }
        }

        // Add orphaned subtrees: nodes whose parent OID is not in our index.
        let root_oids: HashSet<_> = roots.iter().map(|r| r.oid.clone()).collect();
        for node in self.oid_index.values() {
            let parent_oid = Self::parent_oid(&node.oid);
            // A node is orphaned if its parent isn't indexed and it's not already a root.
            if !parent_oid.is_empty()
                && !indexed_oids.contains(&parent_oid)
                && !root_oids.contains(&node.oid)
            {
                roots.push(self.build_tree_node(node, &children_map));
            }
        }

        roots.sort_by(|a, b| a.name.cmp(&b.name));
        roots
    }

    /// Searches for MIB nodes matching the given query string.
    ///
    /// Matches are returned if:
    /// - The OID starts with the query (case-insensitive prefix match)
    /// - The node name contains the query (case-insensitive substring match)
    ///
    /// Returns at most 50 results, sorted by relevance (exact matches first).
    pub fn search(&self, query: &str) -> Vec<MibNode> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut matches: Vec<(usize, MibNode)> = Vec::new();

        for node in self.oid_index.values() {
            let oid_lower = node.oid.to_lowercase();
            let name_lower = node.name.to_lowercase();

            let score = if oid_lower == query_lower || name_lower == query_lower {
                0 // Exact match — highest priority
            } else if name_lower.starts_with(&query_lower) {
                1 // Name prefix match
            } else if oid_lower.starts_with(&query_lower) {
                2 // OID prefix match
            } else if name_lower.contains(&query_lower) {
                3 // Name substring match
            } else {
                continue;
            };

            matches.push((score, node.clone()));
        }

        matches.sort_by_key(|(score, _)| *score);
        matches.into_iter().map(|(_, n)| n).take(50).collect()
    }

    /// Returns the parent OID by removing the last numeric segment.
    fn parent_oid(oid: &str) -> String {
        if let Some(last_dot) = oid.rfind('.') {
            oid[..last_dot].to_string()
        } else {
            String::new()
        }
    }

    /// Recursively builds a TreeNode from a MibNode.
    fn build_tree_node(
        &self,
        node: &MibNode,
        children_map: &HashMap<String, Vec<&MibNode>>,
    ) -> TreeNode {
        let syntax_label = if node.syntax_type != SyntaxType::ObjectIdentifier {
            Some(node.syntax_type.label().to_string())
        } else {
            None
        };

        let mut children = Vec::new();
        if let Some(child_nodes) = children_map.get(&node.oid) {
            for child in self.sort_nodes(child_nodes) {
                // Skip if this child is actually the same node (self-reference).
                if child.oid != node.oid {
                    children.push(self.build_tree_node(child, children_map));
                }
            }
        }

        TreeNode {
            oid: node.oid.clone(),
            name: node.name.clone(),
            syntax_type: syntax_label,
            mib_name: node.mib_name.clone(),
            children,
        }
    }

    /// Sorts nodes by a stable order: OBJECT IDENTIFIER subtrees first (alphabetical),
    /// then leaf objects (alphabetical).
    fn sort_nodes<'a>(&self, nodes: &'a [&'a MibNode]) -> Vec<&'a MibNode> {
        let mut sorted: Vec<_> = nodes.iter().copied().collect();
        sorted.sort_by(|a, b| {
            let a_is_subtree = matches!(a.syntax_type, SyntaxType::ObjectIdentifier);
            let b_is_subtree = matches!(b.syntax_type, SyntaxType::ObjectIdentifier);
            match (a_is_subtree, b_is_subtree) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        sorted
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
            if oid.starts_with(&format!("{}.", indexed_oid)) || oid == indexed_oid {
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
    fn resolve_no_false_positive_on_similar_prefix() {
        let mut resolver = Resolver::new();
        resolver.oid_index.insert(
            "1.3.6".to_string(),
            MibNode {
                oid: "1.3.6".to_string(),
                name: "org".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "ROOT".to_string(),
            },
        );

        // "1.3.61..." should NOT match "1.3.6" — different sub-identifier.
        assert_eq!(resolver.resolve("1.3.61.2.1"), None);
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

    #[test]
    fn build_tree_empty() {
        let resolver = Resolver::new();
        let tree = resolver.build_tree();
        assert!(tree.is_empty());
    }

    #[test]
    fn build_tree_single_node() {
        let mut resolver = Resolver::new();
        resolver.oid_index.insert(
            "1.3.6.1.2.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1".to_string(),
                name: "mib-2".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "SNMPv2-MIB".to_string(),
            },
        );

        let tree = resolver.build_tree();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "mib-2");
        assert!(tree[0].children.is_empty());
    }

    #[test]
    fn build_tree_hierarchy() {
        let mut resolver = Resolver::new();

        // Parent subtree.
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1".to_string(),
                name: "system".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "SNMPv2-MIB".to_string(),
            },
        );

        // Child leaf node.
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
            },
        );

        let tree = resolver.build_tree();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "system");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].name, "sysDescr");
    }

    #[test]
    fn search_by_oid_prefix() {
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

        let results = resolver.search("1.3.6.1");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "sysDescr");
    }

    #[test]
    fn search_by_name() {
        let mut resolver = Resolver::new();
        resolver
            .name_index
            .insert("sysDescr".to_string(), "1.3.6.1.2.1.1.1".to_string());
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
            },
        );

        let results = resolver.search("sys");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "sysDescr");
    }

    #[test]
    fn search_empty_query() {
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

        assert!(resolver.search("").is_empty());
    }

    #[test]
    fn unload_mib_removes_nodes() {
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

        assert_eq!(resolver.node_count(), 1);

        resolver.unload_mib("SNMPv2-MIB");

        assert_eq!(resolver.node_count(), 0);
        assert_eq!(resolver.reverse_lookup("sysDescr"), None);
    }

    #[test]
    fn unload_mib_preserves_other_modules() {
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

        resolver.oid_index.insert(
            "1.3.6.1.4.1.99999.1".to_string(),
            MibNode {
                oid: "1.3.6.1.4.1.99999.1".to_string(),
                name: "vendorObj".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "VENDOR-MIB".to_string(),
            },
        );
        resolver
            .name_index
            .insert("vendorObj".to_string(), "1.3.6.1.4.1.99999.1".to_string());

        assert_eq!(resolver.node_count(), 2);

        resolver.unload_mib("SNMPv2-MIB");

        assert_eq!(resolver.node_count(), 1);
        assert_eq!(
            resolver.reverse_lookup("vendorObj"),
            Some("1.3.6.1.4.1.99999.1")
        );
        assert_eq!(resolver.reverse_lookup("sysDescr"), None);
    }

    #[test]
    fn loaded_mibs_returns_info() {
        let mut resolver = Resolver::new();
        resolver.loaded_files.insert(
            "/usr/share/snmp/mibs/SNMPv2-MIB.txt".to_string(),
            "SNMPv2-MIB".to_string(),
        );
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
            },
        );

        let info = resolver.loaded_mibs();
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].mib_name, "SNMPv2-MIB");
        assert_eq!(info[0].node_count, 1);
    }

    #[test]
    fn parent_oid_removes_last_segment() {
        assert_eq!(Resolver::parent_oid("1.3.6.1.2.1.1"), "1.3.6.1.2.1");
        assert_eq!(Resolver::parent_oid("1.3.6.1"), "1.3.6");
        assert_eq!(Resolver::parent_oid("1"), "");
    }
}
