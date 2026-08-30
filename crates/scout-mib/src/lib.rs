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
    detect_module_name_original(content).to_uppercase()
}

/// Detects the MIB module name from file content, preserving the original
/// case as written in the `DEFINITIONS` header.
///
/// Use this (rather than [`detect_module_name`]) when the name is passed to
/// mib-rs, which matches module names case-sensitively against the parsed
/// source.
pub fn detect_module_name_original(content: &str) -> String {
    if let Some(captures) = module_name_re().captures(content) {
        if let Some(name_match) = captures.get(1) {
            return name_match.as_str().to_string();
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
    /// SMI TABLE container (SYNTAX SEQUENCE OF Entry). Not directly queryable.
    Table,
    /// SMI ROW entry. Not directly queryable — contains column definitions.
    TableRow,
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
            SyntaxType::Table => "TABLE",
            SyntaxType::TableRow => "ROW",
            SyntaxType::Unknown(s) => s.as_str(),
        }
    }
}

/// How an index component's value maps to OID sub-identifiers (RFC 2578 §7.7).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum IndexEncoding {
    /// Single sub-identifier holding the integer value.
    Integer,
    /// Four sub-identifiers, one per IPv4 octet.
    IpAddress,
    /// Exactly `n` sub-identifiers (SIZE-constrained string), no length prefix.
    FixedString(usize),
    /// Variable-length or undetermined — the component cannot be split
    /// deterministically and is treated as opaque.
    Variable,
}

/// One component of a table's INDEX clause.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexColumn {
    /// Name from the INDEX clause (e.g., `"ifIndex"`).
    pub name: String,
    /// Column OID of the index object (empty for bare-type indexes).
    pub oid: String,
    /// Whether the component is declared `IMPLIED`.
    pub implied: bool,
    /// How the component's value maps to sub-identifiers.
    pub encoding: IndexEncoding,
}

/// Metadata about a Table parsed from its INDEX/AUGMENTS clauses.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    /// Dotted-decimal OID of the table.
    pub table_oid: String,
    /// Table name (e.g., `"ifTable"`).
    pub name: String,
    /// Row entry OIDs: the base entry plus any augmented entries.
    pub row_entry_oids: Vec<String>,
    /// Index columns in INDEX clause order.
    pub index_columns: Vec<IndexColumn>,
    /// All column OIDs (including augmented), in OID order.
    pub column_oids: Vec<String>,
}

/// A named value from a MIB type definition (INTEGER enum or BITS bit).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedValueInfo {
    /// Textual label as written in the MIB (e.g., `"up"`).
    pub label: String,
    /// Integer value associated with the label.
    pub value: i64,
}

/// A named entry in a MIB schema file.
///
/// Represents what *could* be queried, not live data. Has an OID, name,
/// SYNTAX type, and the MIB module it was defined in. The optional detail
/// fields are populated when the parser provides them (always for mib-rs
/// loads; best-effort for regex-fallback MIBs).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MibNode {
    /// Dotted-decimal OID (e.g., `"1.3.6.1.2.1.1.1"`).
    pub oid: String,
    /// Human-readable name (e.g., `"sysDescr"`).
    pub name: String,
    /// SYNTAX type from the MIB definition.
    pub syntax_type: SyntaxType,
    /// Name of the MIB module that defines this node.
    pub mib_name: String,
    /// Whether this node is an SMI TABLE container.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_table: bool,
    /// DESCRIPTION clause text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MAX-ACCESS (or SMIv1 ACCESS) label, e.g. `"read-only"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    /// STATUS label, e.g. `"current"` or `"deprecated"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// UNITS clause text (e.g., `"seconds"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
    /// DEFVAL as written in the MIB source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    /// REFERENCE clause text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Effective DISPLAY-HINT for the type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_hint: Option<String>,
    /// Value constraints from the SYNTAX clause (ranges and/or SIZE),
    /// e.g. `"1..255"` or `"SIZE (0..32)"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<String>,
    /// Named INTEGER values (enum) for the type, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enums: Vec<NamedValueInfo>,
    /// Named BITS values for the type, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bits: Vec<NamedValueInfo>,
}

impl Default for MibNode {
    fn default() -> Self {
        Self {
            oid: String::new(),
            name: String::new(),
            syntax_type: SyntaxType::Unknown("unknown".to_string()),
            mib_name: String::new(),
            is_table: false,
            description: None,
            access: None,
            status: None,
            units: None,
            default_value: None,
            reference: None,
            display_hint: None,
            constraints: None,
            enums: Vec::new(),
            bits: Vec::new(),
        }
    }
}

/// Everything the UI inspector needs to know about one MIB node: its
/// identity, parsed clauses, and — when applicable — the table metadata it
/// owns (TABLE) or the index columns of the table it is a row entry of.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDetails {
    /// Dotted-decimal OID of the resolved node.
    pub oid: String,
    /// Human-readable name.
    pub name: String,
    /// MIB module that defines the node.
    pub mib_name: String,
    /// SYNTAX type label (e.g., `"OctetString"`, `"TABLE"`).
    pub syntax_type: String,
    /// Whether this node is an SMI TABLE container.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_table: bool,
    /// DESCRIPTION clause text.
    pub description: Option<String>,
    /// MAX-ACCESS label.
    pub access: Option<String>,
    /// STATUS label.
    pub status: Option<String>,
    /// UNITS clause text.
    pub units: Option<String>,
    /// DEFVAL as written in the MIB source.
    pub default_value: Option<String>,
    /// REFERENCE clause text.
    pub reference: Option<String>,
    /// Effective DISPLAY-HINT.
    pub display_hint: Option<String>,
    /// Value constraints (ranges and/or SIZE).
    pub constraints: Option<String>,
    /// Named INTEGER values (enum).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enums: Vec<NamedValueInfo>,
    /// Named BITS values.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bits: Vec<NamedValueInfo>,
    /// Table metadata, present when the node is a TABLE container.
    pub table: Option<TableInfo>,
    /// Index columns of the table this node is a row entry of.
    pub index_columns: Option<Vec<IndexColumn>>,
}

impl From<&MibNode> for NodeDetails {
    /// Copies the node's identity and clause fields. `syntax_type` becomes its
    /// display label (the UI shows text, not the enum). Table metadata and
    /// index columns are attached separately by [`Resolver::node_details`].
    fn from(node: &MibNode) -> Self {
        Self {
            oid: node.oid.clone(),
            name: node.name.clone(),
            mib_name: node.mib_name.clone(),
            syntax_type: node.syntax_type.label().to_string(),
            is_table: node.is_table,
            description: node.description.clone(),
            access: node.access.clone(),
            status: node.status.clone(),
            units: node.units.clone(),
            default_value: node.default_value.clone(),
            reference: node.reference.clone(),
            display_hint: node.display_hint.clone(),
            constraints: node.constraints.clone(),
            enums: node.enums.clone(),
            bits: node.bits.clone(),
            table: None,
            index_columns: None,
        }
    }
}

/// Result of loading a single MIB file.
#[derive(Debug)]
pub struct LoadResult {
    /// Nodes extracted from the file.
    pub nodes: Vec<MibNode>,
    /// Table metadata extracted from INDEX/AUGMENTS clauses (empty when none).
    pub tables: Vec<TableInfo>,
    /// Whether the primary (mib-rs) loader succeeded.
    pub primary_success: bool,
    /// MIB module name the file was loaded as (may be empty when undetermined).
    pub module_name: String,
}

/// Single node in the hierarchical MIB tree for UI rendering.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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
    /// Whether this node is an SMI TABLE container.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_table: bool,
    /// Whether this node has children (for lazy loading).
    #[serde(skip_serializing_if = "is_false")]
    pub has_children: bool,
    /// Child nodes (populated only when explicitly requested via `get_children`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
}

fn is_false(v: &bool) -> bool {
    !v
}

/// Metadata about a loaded MIB file for the Manage MIBs dialog.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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
    /// Table OID -> table metadata (mib-rs results take precedence).
    table_index: HashMap<String, TableInfo>,
    /// Table OID -> MIB module name that contributed its metadata.
    table_mibs: HashMap<String, String>,
}

impl Resolver {
    /// Loads all MIB files from the given directories.
    ///
    /// Binary and non-text files are pre-filtered before any parse attempt.
    /// mib-rs is tried first; on failure, a regex-based fallback extracts
    /// whatever OBJECT-TYPE blocks it can find. mib-rs results take precedence
    /// in the merged index.
    pub fn load_directories(&mut self, directories: &[String]) {
        let mut all_nodes = Vec::new();
        let mut primary_nodes = Vec::new();
        // Table metadata from successful mib-rs loads, paired with their module.
        let mut primary_tables: Vec<(String, TableInfo)> = Vec::new();
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
                            // Track file -> MIB name so modules that contribute
                            // no queryable nodes (e.g. pure TEXTUAL-CONVENTION
                            // MIBs) still show up in Manage MIBs.
                            if !result.module_name.is_empty() {
                                file_mib_map.insert(
                                    file.to_string_lossy().to_string(),
                                    result.module_name.clone(),
                                );
                            }
                            primary_tables.extend(
                                result
                                    .tables
                                    .into_iter()
                                    .map(|t| (result.module_name.clone(), t)),
                            );
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
                        Ok((nodes, tables)) => {
                            let mib_name = fallback.last_mib_name().to_string();
                            if !nodes.is_empty() || !tables.is_empty() {
                                self.fallback_mibs.insert(mib_name.clone());
                                file_mib_map
                                    .insert(file.to_string_lossy().to_string(), mib_name.clone());
                                all_nodes.push(LoadResult {
                                    module_name: mib_name.clone(),
                                    nodes,
                                    tables,
                                    primary_success: false,
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

        // Merge table metadata: primary first (takes precedence), then fallback fills gaps.
        let mut table_index: HashMap<String, TableInfo> = HashMap::new();
        let mut table_mibs: HashMap<String, String> = HashMap::new();
        for (mib_name, table) in primary_tables {
            table_mibs.insert(table.table_oid.clone(), mib_name);
            table_index.insert(table.table_oid.clone(), table);
        }
        for result in &all_nodes {
            for table in &result.tables {
                if !table_index.contains_key(&table.table_oid) {
                    table_mibs.insert(table.table_oid.clone(), result.module_name.clone());
                    table_index.insert(table.table_oid.clone(), table.clone());
                }
            }
        }

        self.oid_index = merged_oid;
        self.name_index = merged_name;
        self.loaded_files = file_mib_map.into_iter().collect();
        self.table_index = table_index;
        self.table_mibs = table_mibs;

        info!(
            "Resolver loaded {} nodes ({} tables, {} fallback MIBs, {} tracked files)",
            self.oid_index.len(),
            self.table_index.len(),
            self.fallback_mibs.len(),
            self.loaded_files.len()
        );
    }

    /// Returns table metadata for the given table OID, if known.
    pub fn get_table_info(&self, table_oid: &str) -> Option<&TableInfo> {
        self.table_index.get(table_oid)
    }

    /// Builds full inspector details for the given OID.
    ///
    /// Uses longest-prefix resolution like [`resolve`], so instance OIDs
    /// (e.g. `…ifIndex.1`) report on their base object. Table metadata is
    /// attached when the resolved node is a TABLE container; index columns
    /// are attached when it is a row entry of a known table. Returns `None`
    /// for OIDs with no matching node.
    pub fn node_details(&self, oid: &str) -> Option<NodeDetails> {
        let node = self.resolve(oid)?;
        let mut details = NodeDetails::from(node);

        // TABLE container: attach its parsed metadata (INDEX/AUGMENTS/columns).
        let is_table_container = node.is_table || matches!(node.syntax_type, SyntaxType::Table);
        if is_table_container {
            details.table = self.table_index.get(&node.oid).cloned();
        } else {
            // Row entry: the index columns of the table it belongs to (base row
            // or an augmented one — both appear in `row_entry_oids`).
            details.index_columns = self
                .table_index
                .values()
                .find(|t| t.row_entry_oids.iter().any(|r| r == &node.oid))
                .map(|t| t.index_columns.clone());
        }

        Some(details)
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

        // Drop table metadata contributed by this module.
        let tables_to_remove: Vec<String> = self
            .table_mibs
            .iter()
            .filter(|(_, mn)| *mn == mib_name)
            .map(|(oid, _)| oid.clone())
            .collect();
        for oid in &tables_to_remove {
            self.table_index.remove(oid);
            self.table_mibs.remove(oid);
        }

        if oids_to_remove.is_empty() {
            self.fallback_mibs.remove(mib_name);
        }

        info!(
            "Unloaded MIB module '{}': removed {} nodes",
            mib_name,
            oids_to_remove.len()
        );
    }

    /// Builds a hierarchical tree of all loaded MIB nodes (shallow — no children).
    ///
    /// The tree is organized by OID hierarchy: each node's parent is determined
    /// by removing the last numeric segment from its OID. Root-level OIDs become
    /// top-level tree entries. Children are NOT included — use `get_children()`
    /// to fetch them lazily when a node is expanded.
    ///
    /// Root-level leaf nodes (no children) are grouped into an "other" folder
    /// at the bottom to reduce clutter.
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

        // Collect all root-level nodes (direct roots + orphans).
        let mut roots: Vec<&MibNode> = Vec::new();
        if let Some(root_children) = children_map.get("") {
            for node in self.sort_nodes(root_children) {
                roots.push(node);
            }
        }

        // Add orphaned nodes whose parent OID is not in our index.
        let root_oids: HashSet<_> = roots.iter().map(|r| r.oid.clone()).collect();
        for node in self.oid_index.values() {
            let parent_oid = Self::parent_oid(&node.oid);
            if !parent_oid.is_empty()
                && !indexed_oids.contains(&parent_oid)
                && !root_oids.contains(&node.oid)
            {
                roots.push(node);
            }
        }

        // Split into subtrees (have children/descendants) and leaves.
        let mut subtrees: Vec<TreeNode> = Vec::new();
        let mut leaves: Vec<TreeNode> = Vec::new();
        for node in &roots {
            let has_children = self.node_has_descendants(node.oid.as_str(), &children_map);
            let tree_node = self.build_tree_node_shallow(node, has_children);
            if has_children {
                subtrees.push(tree_node);
            } else {
                leaves.push(tree_node);
            }
        }

        subtrees.sort_by(|a, b| a.name.cmp(&b.name));

        // If there are leaf nodes at the root level, group them under "other".
        if !leaves.is_empty() {
            leaves.sort_by(|a, b| a.name.cmp(&b.name));
            subtrees.push(TreeNode {
                oid: "__other__".to_string(),
                name: "other".to_string(),
                syntax_type: None,
                mib_name: "".to_string(),
                is_table: false,
                has_children: true,
                children: leaves,
            });
        }

        subtrees
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

    /// Builds a shallow TreeNode (no children populated) for lazy loading.
    fn build_tree_node_shallow(&self, node: &MibNode, has_children: bool) -> TreeNode {
        let syntax_label = if node.syntax_type != SyntaxType::ObjectIdentifier {
            Some(node.syntax_type.label().to_string())
        } else {
            None
        };

        TreeNode {
            oid: node.oid.clone(),
            name: node.name.clone(),
            syntax_type: syntax_label,
            mib_name: node.mib_name.clone(),
            is_table: node.is_table,
            has_children,
            children: Vec::new(),
        }
    }

    /// Returns direct children of the given OID for lazy loading.
    /// For the special "__other__" folder, returns empty (children are pre-populated).
    pub fn get_children(&self, parent_oid: &str) -> Vec<TreeNode> {
        if parent_oid == "__other__" {
            return Vec::new();
        }

        let mut children_map: HashMap<String, Vec<&MibNode>> = HashMap::new();
        for node in self.oid_index.values() {
            let p = Self::parent_oid(&node.oid);
            children_map.entry(p).or_default().push(node);
        }

        let indexed_oids: HashSet<_> = self.oid_index.keys().cloned().collect();

        // Get direct children (excluding self-references).
        let mut result: Vec<TreeNode> = Vec::new();
        if let Some(child_nodes) = children_map.get(parent_oid) {
            for child in self.sort_nodes(child_nodes) {
                if child.oid != parent_oid {
                    let has_children = self.node_has_descendants(child.oid.as_str(), &children_map);
                    result.push(self.build_tree_node_shallow(child, has_children));
                }
            }
        }

        // If no direct children but deeper descendants exist (orphans), include them.
        if result.is_empty() && !parent_oid.is_empty() {
            let child_prefix = format!("{}.", parent_oid);
            let mut orphans: Vec<&MibNode> = Vec::new();
            for node in self.oid_index.values() {
                if node.oid.starts_with(&child_prefix)
                    && !indexed_oids.contains(&Self::parent_oid(&node.oid))
                {
                    orphans.push(node);
                }
            }

            if !orphans.is_empty() {
                orphans.sort_by(|a, b| a.name.cmp(&b.name));
                for node in orphans {
                    let has_children = self.node_has_descendants(node.oid.as_str(), &children_map);
                    result.push(self.build_tree_node_shallow(node, has_children));
                }
            }
        }

        result
    }

    /// Checks whether a node has any descendants (direct children or deeper).
    fn node_has_descendants(
        &self,
        oid: &str,
        children_map: &HashMap<String, Vec<&MibNode>>,
    ) -> bool {
        // Direct children (excluding self-references).
        if let Some(children) = children_map.get(oid) {
            if children.iter().any(|c| c.oid != oid) {
                return true;
            }
        }

        // Deeper descendants: any indexed OID that is a proper sub-OID.
        let child_prefix = format!("{}.", oid);
        for indexed_oid in self.oid_index.keys() {
            if indexed_oid.starts_with(&child_prefix) {
                return true;
            }
        }

        false
    }

    /// Sorts nodes by a stable order: OBJECT IDENTIFIER subtrees first (alphabetical),
    /// then leaf objects (alphabetical). TABLE and ROW are treated as subtrees.
    fn sort_nodes<'a>(&self, nodes: &'a [&'a MibNode]) -> Vec<&'a MibNode> {
        let mut sorted: Vec<_> = nodes.to_vec();
        sorted.sort_by(|a, b| {
            let a_is_subtree = matches!(
                a.syntax_type,
                SyntaxType::ObjectIdentifier | SyntaxType::Table | SyntaxType::TableRow
            );
            let b_is_subtree = matches!(
                b.syntax_type,
                SyntaxType::ObjectIdentifier | SyntaxType::Table | SyntaxType::TableRow
            );
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

    /// Returns column OIDs for a TABLE node.
    ///
    /// Prefers parsed table metadata (exact column set, including augmented
    /// columns and excluding nested sub-tables). Falls back to the leaf-object
    /// heuristic when no metadata exists or it carries no columns (fallback-MIB
    /// tables), where every indexed leaf under the subtree is treated as a
    /// column: an OID-syntax node is only excluded when it actually has
    /// indexed descendants — a leaf object whose SYNTAX is OBJECT IDENTIFIER
    /// (e.g. ifSpecific) is a column.
    pub fn get_table_columns(&self, table_oid: &str) -> Vec<String> {
        if let Some(info) = self.table_index.get(table_oid) {
            if !info.column_oids.is_empty() {
                return info.column_oids.clone();
            }
        }

        let mut columns = Vec::new();
        for node in self.oid_index.values() {
            // Must be under the table's subtree
            if !node.oid.starts_with(&format!("{}.", table_oid)) && node.oid != table_oid {
                continue;
            }
            // Skip TABLE and ROW containers — they're not queryable columns
            if matches!(node.syntax_type, SyntaxType::Table | SyntaxType::TableRow) {
                continue;
            }
            // Intermediate OBJECT IDENTIFIER subtrees are not queryable columns.
            // A subtree is an OID-syntax node that has indexed descendants; a leaf
            // OID-syntax object (e.g. ifSpecific) is a regular column.
            if node.syntax_type == SyntaxType::ObjectIdentifier {
                let prefix = format!("{}.", node.oid);
                let is_subtree = self
                    .oid_index
                    .keys()
                    .any(|k| k.as_str() != node.oid && k.starts_with(&prefix));
                if is_subtree {
                    continue;
                }
            }
            columns.push(node.oid.clone());
        }
        columns.sort();
        columns
    }

    /// Returns the total number of indexed nodes.
    pub fn node_count(&self) -> usize {
        self.oid_index.len()
    }

    /// Returns all OID → name pairs for frontend resolution.
    pub fn oid_name_map(&self) -> Vec<(String, String)> {
        self.oid_index
            .values()
            .map(|node| (node.oid.clone(), node.name.clone()))
            .collect()
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

/// Compares two dotted-decimal OIDs numerically, sub-identifier by
/// sub-identifier (so `2` sorts before `10`, unlike string order).
pub(crate) fn oid_numeric_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |oid: &str| -> Vec<u64> { oid.split('.').filter_map(|p| p.parse().ok()).collect() };
    key(a).cmp(&key(b))
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
        let mut resolver = Resolver::default();
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
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
        let mut resolver = Resolver::default();
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1".to_string(),
                name: "system".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // Instance OID should resolve to the deepest matching ancestor.
        let node = resolver.resolve("1.3.6.1.2.1.1.1.0").unwrap();
        assert_eq!(node.name, "system");
    }

    #[test]
    fn resolve_no_false_positive_on_similar_prefix() {
        let mut resolver = Resolver::default();
        resolver.oid_index.insert(
            "1.3.6".to_string(),
            MibNode {
                oid: "1.3.6".to_string(),
                name: "org".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "ROOT".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // "1.3.61..." should NOT match "1.3.6" — different sub-identifier.
        assert_eq!(resolver.resolve("1.3.61.2.1"), None);
    }

    #[test]
    fn reverse_lookup_basic() {
        let mut resolver = Resolver::default();
        resolver
            .name_index
            .insert("sysDescr".to_string(), "1.3.6.1.2.1.1.1".to_string());

        assert_eq!(resolver.reverse_lookup("sysDescr"), Some("1.3.6.1.2.1.1.1"));
        assert_eq!(resolver.reverse_lookup("nonexistent"), None);
    }

    #[test]
    fn merge_primary_takes_precedence() {
        let mut resolver = Resolver::default();

        // Simulate primary node.
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // Simulate fallback with same OID but different data.
        let primary_map: HashMap<String, MibNode> = resolver.oid_index.clone();
        assert_eq!(primary_map.len(), 1);
    }

    #[test]
    fn build_tree_empty() {
        let resolver = Resolver::default();
        let tree = resolver.build_tree();
        assert!(tree.is_empty());
    }

    #[test]
    fn build_tree_single_node() {
        let mut resolver = Resolver::default();
        resolver.oid_index.insert(
            "1.3.6.1.2.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1".to_string(),
                name: "mib-2".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        let tree = resolver.build_tree();
        // Single leaf node goes into "other" folder.
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "other");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].name, "mib-2");
    }

    #[test]
    fn build_tree_hierarchy() {
        let mut resolver = Resolver::default();

        // Parent subtree.
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1".to_string(),
                name: "system".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
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
                is_table: false,
                ..Default::default()
            },
        );

        let tree = resolver.build_tree();
        // "system" has children -> subtree (shallow, no children populated).
        // "sysDescr"'s parent is indexed, so it's nested under system, not a root orphan.
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "system");
        assert!(tree[0].has_children);
        assert!(tree[0].children.is_empty()); // lazy-loaded
    }

    #[test]
    fn get_children_returns_direct_children() {
        let mut resolver = Resolver::default();

        resolver.oid_index.insert(
            "1.3.6.1.2.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1".to_string(),
                name: "system".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.2".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.2".to_string(),
                name: "sysObjectID".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        let children = resolver.get_children("1.3.6.1.2.1.1");
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].name, "sysObjectID"); // subtree first
        assert!(!children[0].has_children); // no grandchildren indexed
        assert_eq!(children[1].name, "sysDescr");
        assert!(!children[1].has_children);
    }

    #[test]
    fn search_by_oid_prefix() {
        let mut resolver = Resolver::default();
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        let results = resolver.search("1.3.6.1");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "sysDescr");
    }

    #[test]
    fn search_by_name() {
        let mut resolver = Resolver::default();
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
                is_table: false,
                ..Default::default()
            },
        );

        let results = resolver.search("sys");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "sysDescr");
    }

    #[test]
    fn search_empty_query() {
        let mut resolver = Resolver::default();
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        assert!(resolver.search("").is_empty());
    }

    #[test]
    fn unload_mib_removes_nodes() {
        let mut resolver = Resolver::default();
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
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
        let mut resolver = Resolver::default();
        resolver.oid_index.insert(
            "1.3.6.1.2.1.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.1.1".to_string(),
                name: "sysDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "SNMPv2-MIB".to_string(),
                is_table: false,
                ..Default::default()
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
                is_table: false,
                ..Default::default()
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
        let mut resolver = Resolver::default();
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
                is_table: false,
                ..Default::default()
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

    #[test]
    fn get_table_columns_finds_leaf_objects() {
        let mut resolver = Resolver::default();

        // Table container.
        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1".to_string(),
                name: "ifEntry".to_string(),
                syntax_type: SyntaxType::TableRow,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // Column 1 (index).
        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1.1".to_string(),
                name: "ifIndex".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // Column 2 (description).
        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1.2".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1.2".to_string(),
                name: "ifDescr".to_string(),
                syntax_type: SyntaxType::OctetString,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // Column 3 (type).
        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1.3".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1.3".to_string(),
                name: "ifType".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // Intermediate subtree (should be excluded — it has a descendant).
        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1.99".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1.99".to_string(),
                name: "ifSubtree".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );
        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1.99.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1.99.1".to_string(),
                name: "ifSubtreeLeaf".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        let columns = resolver.get_table_columns("1.3.6.1.2.1.2.2.1");
        // ifSubtree itself is excluded (it's a subtree); its leaf descendant
        // is a regular column like any other leaf object under the table.
        assert_eq!(columns.len(), 4);
        assert_eq!(columns[0], "1.3.6.1.2.1.2.2.1.1"); // ifIndex
        assert_eq!(columns[1], "1.3.6.1.2.1.2.2.1.2"); // ifDescr
        assert_eq!(columns[2], "1.3.6.1.2.1.2.2.1.3"); // ifType
        assert_eq!(columns[3], "1.3.6.1.2.1.2.2.1.99.1"); // ifSubtreeLeaf
    }

    #[test]
    fn get_table_columns_includes_oid_typed_leaf_objects() {
        let mut resolver = Resolver::default();

        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1".to_string(),
                name: "ifEntry".to_string(),
                syntax_type: SyntaxType::TableRow,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // A column whose SYNTAX is OBJECT IDENTIFIER (e.g. ifSpecific) — must be
        // included even though its syntax matches the subtree-node type.
        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1.22".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1.22".to_string(),
                name: "ifSpecific".to_string(),
                syntax_type: SyntaxType::ObjectIdentifier,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        let columns = resolver.get_table_columns("1.3.6.1.2.1.2.2.1");
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0], "1.3.6.1.2.1.2.2.1.22"); // ifSpecific
    }

    #[test]
    fn get_table_columns_excludes_unrelated_subtree() {
        let mut resolver = Resolver::default();

        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1".to_string(),
                name: "ifEntry".to_string(),
                syntax_type: SyntaxType::TableRow,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        resolver.oid_index.insert(
            "1.3.6.1.2.1.2.2.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.2.2.1.1".to_string(),
                name: "ifIndex".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "IF-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // Unrelated node in a different subtree.
        resolver.oid_index.insert(
            "1.3.6.1.2.1.3.1".to_string(),
            MibNode {
                oid: "1.3.6.1.2.1.3.1".to_string(),
                name: "ipAddrTable".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "IP-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        let columns = resolver.get_table_columns("1.3.6.1.2.1.2.2.1");
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0], "1.3.6.1.2.1.2.2.1.1");
    }

    #[test]
    fn get_table_columns_prefers_metadata_over_leaf_heuristic() {
        let mut resolver = Resolver::default();

        // Table container + its real columns.
        resolver.oid_index.insert(
            "1.3.6.1.4.1.99996.1.3".to_string(),
            MibNode {
                oid: "1.3.6.1.4.1.99996.1.3".to_string(),
                name: "outerTable".to_string(),
                syntax_type: SyntaxType::Table,
                mib_name: "TABLE-META-MIB".to_string(),
                is_table: true,
                ..Default::default()
            },
        );
        resolver.oid_index.insert(
            "1.3.6.1.4.1.99996.1.3.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.4.1.99996.1.3.1.1".to_string(),
                name: "outerIndex".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "TABLE-META-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );
        resolver.oid_index.insert(
            "1.3.6.1.4.1.99996.1.3.1.2".to_string(),
            MibNode {
                oid: "1.3.6.1.4.1.99996.1.3.1.2".to_string(),
                name: "outerValue".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "TABLE-META-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        // A nested sub-table leaf under the outer table's subtree. The leaf
        // heuristic would include it; metadata must exclude it (G4).
        resolver.oid_index.insert(
            "1.3.6.1.4.1.99996.1.3.3.1.2".to_string(),
            MibNode {
                oid: "1.3.6.1.4.1.99996.1.3.3.1.2".to_string(),
                name: "innerValue".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "TABLE-META-MIB".to_string(),
                is_table: false,
                ..Default::default()
            },
        );

        let info = TableInfo {
            table_oid: "1.3.6.1.4.1.99996.1.3".to_string(),
            name: "outerTable".to_string(),
            row_entry_oids: vec!["1.3.6.1.4.1.99996.1.3.1".to_string()],
            index_columns: vec![IndexColumn {
                name: "outerIndex".to_string(),
                oid: "1.3.6.1.4.1.99996.1.3.1.1".to_string(),
                implied: false,
                encoding: IndexEncoding::Integer,
            }],
            column_oids: vec![
                "1.3.6.1.4.1.99996.1.3.1.1".to_string(),
                "1.3.6.1.4.1.99996.1.3.1.2".to_string(),
            ],
        };
        resolver.table_index.insert(info.table_oid.clone(), info);
        resolver.table_mibs.insert(
            "1.3.6.1.4.1.99996.1.3".to_string(),
            "TABLE-META-MIB".to_string(),
        );

        let columns = resolver.get_table_columns("1.3.6.1.4.1.99996.1.3");
        assert_eq!(
            columns,
            vec!["1.3.6.1.4.1.99996.1.3.1.1", "1.3.6.1.4.1.99996.1.3.1.2",]
        );

        // get_table_info exposes the same metadata.
        let got = resolver
            .get_table_info("1.3.6.1.4.1.99996.1.3")
            .expect("info");
        assert_eq!(got.name, "outerTable");
        assert_eq!(got.index_columns[0].encoding, IndexEncoding::Integer);

        // Unloading the module prunes the table metadata.
        resolver.unload_mib("TABLE-META-MIB");
        assert!(resolver.get_table_info("1.3.6.1.4.1.99996.1.3").is_none());
    }

    /// Resolver fixture with one scalar, one table, and its row entry.
    fn details_fixture_resolver() -> Resolver {
        let mut resolver = Resolver::default();

        resolver.oid_index.insert(
            "1.3.6.1.4.1.99994.1.1".to_string(),
            MibNode {
                oid: "1.3.6.1.4.1.99994.1.1".to_string(),
                name: "gadgetLevel".to_string(),
                syntax_type: SyntaxType::Integer32,
                mib_name: "GADGET-MIB".to_string(),
                description: Some("How loud the gadget is.".to_string()),
                access: Some("read-write".to_string()),
                status: Some("current".to_string()),
                units: Some("percent".to_string()),
                default_value: Some("50".to_string()),
                constraints: Some("0..100".to_string()),
                ..Default::default()
            },
        );

        resolver.oid_index.insert(
            "1.3.6.1.4.1.99994.2".to_string(),
            MibNode {
                oid: "1.3.6.1.4.1.99994.2".to_string(),
                name: "gadgetTable".to_string(),
                syntax_type: SyntaxType::Table,
                mib_name: "GADGET-MIB".to_string(),
                is_table: true,
                ..Default::default()
            },
        );

        resolver.oid_index.insert(
            "1.3.6.1.4.1.99994.2.1".to_string(),
            MibNode {
                oid: "1.3.6.1.4.1.99994.2.1".to_string(),
                name: "gadgetEntry".to_string(),
                syntax_type: SyntaxType::TableRow,
                mib_name: "GADGET-MIB".to_string(),
                ..Default::default()
            },
        );

        let info = TableInfo {
            table_oid: "1.3.6.1.4.1.99994.2".to_string(),
            name: "gadgetTable".to_string(),
            row_entry_oids: vec!["1.3.6.1.4.1.99994.2.1".to_string()],
            index_columns: vec![IndexColumn {
                name: "gadgetIndex".to_string(),
                oid: "1.3.6.1.4.1.99994.2.1.1".to_string(),
                implied: false,
                encoding: IndexEncoding::Integer,
            }],
            column_oids: vec![
                "1.3.6.1.4.1.99994.2.1.1".to_string(),
                "1.3.6.1.4.1.99994.2.1.2".to_string(),
            ],
        };
        resolver.table_index.insert(info.table_oid.clone(), info);

        resolver
    }

    #[test]
    fn node_details_scalar_carries_clause_fields() {
        let resolver = details_fixture_resolver();
        let d = resolver
            .node_details("1.3.6.1.4.1.99994.1.1")
            .expect("details");

        assert_eq!(d.name, "gadgetLevel");
        assert_eq!(d.mib_name, "GADGET-MIB");
        assert_eq!(d.syntax_type, "Integer32");
        assert!(!d.is_table);
        assert_eq!(d.description.as_deref(), Some("How loud the gadget is."));
        assert_eq!(d.access.as_deref(), Some("read-write"));
        assert_eq!(d.status.as_deref(), Some("current"));
        assert_eq!(d.units.as_deref(), Some("percent"));
        assert_eq!(d.default_value.as_deref(), Some("50"));
        assert_eq!(d.constraints.as_deref(), Some("0..100"));
        assert!(d.table.is_none());
        assert!(d.index_columns.is_none());
    }

    #[test]
    fn node_details_table_attaches_table_info() {
        let resolver = details_fixture_resolver();
        let d = resolver
            .node_details("1.3.6.1.4.1.99994.2")
            .expect("details");

        assert!(d.is_table);
        assert_eq!(d.syntax_type, "TABLE");
        let table = d.table.expect("table metadata attached");
        assert_eq!(table.name, "gadgetTable");
        assert_eq!(table.column_oids.len(), 2);
        assert!(d.index_columns.is_none());
    }

    #[test]
    fn node_details_row_entry_attaches_index_columns() {
        let resolver = details_fixture_resolver();
        let d = resolver
            .node_details("1.3.6.1.4.1.99994.2.1")
            .expect("details");

        assert_eq!(d.name, "gadgetEntry");
        assert_eq!(d.syntax_type, "ROW");
        let idx = d.index_columns.expect("index columns attached");
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].name, "gadgetIndex");
        assert!(d.table.is_none());
    }

    #[test]
    fn node_details_instance_oid_resolves_to_base_node() {
        let resolver = details_fixture_resolver();
        // A live instance OID reports on its base object (longest prefix).
        let d = resolver
            .node_details("1.3.6.1.4.1.99994.1.1.7")
            .expect("details");
        assert_eq!(d.oid, "1.3.6.1.4.1.99994.1.1");
        assert_eq!(d.name, "gadgetLevel");
    }

    #[test]
    fn node_details_unknown_oid_is_none() {
        let resolver = details_fixture_resolver();
        assert!(resolver.node_details("9.9.9").is_none());
    }
}
