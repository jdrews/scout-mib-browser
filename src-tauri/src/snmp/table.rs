use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use super::tolerant::binding_from_snmp;
use super::types::*;

/// A single cell in a table grid result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCell {
    /// The SNMP value at this position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<VariableBinding>,
    /// Whether this cell is missing (column didn't return data for this row).
    pub missing: bool,
}

/// A single row in a table grid, keyed by its instance suffix.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRow {
    /// The instance suffix that identifies this row (e.g., "1" or "192.168.1.1.1").
    pub instance_id: String,
    /// Column OID -> cell data mapping.
    #[serde(flatten)]
    pub cells: BTreeMap<String, TableCell>,
}

/// Result of a table retrieval operation — pivoted grid of rows and columns.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableResult {
    /// The table's root OID.
    pub table_oid: String,
    /// Column OIDs that were walked (in order).
    pub columns: Vec<String>,
    /// Rows indexed by instance ID.
    pub rows: Vec<TableRow>,
    /// Total number of unique instances found across all columns.
    pub total_rows: usize,
    /// Number of cells that are missing due to inconsistent column data.
    pub missing_cells: usize,
    /// Non-fatal warnings collected during retrieval.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<SnmpWarning>,
    /// Whether the operation completed fully or returned partial results.
    pub partial: bool,
}

/// Extracts the instance suffix from a variable binding OID given a column base OID.
///
/// For example, if `column_oid` is "1.3.6.1.2.1.2.2.1.2" and `binding_oid` is
/// "1.3.6.1.2.1.2.2.1.2.7", returns "7".
pub fn extract_instance_suffix(column_oid: &str, binding_oid: &str) -> Option<String> {
    if binding_oid == column_oid {
        return Some("0".to_string());
    }
    if let Some(suffix) = binding_oid.strip_prefix(&format!("{}.", column_oid)) {
        Some(suffix.to_string())
    } else {
        None
    }
}

/// Assembles a grid of rows from per-column walk results.
///
/// Performs best-effort merge: all unique instance IDs are collected, and missing
/// cells are marked with `missing: true`. Returns warnings for columns that had
/// fewer rows than the maximum.
pub fn assemble_table_grid(
    column_oid: String,
    column_results: HashMap<String, Vec<VariableBinding>>,
) -> TableResult {
    let columns: Vec<String> = column_results.keys().cloned().collect();

    // Collect all unique instance IDs and map column -> instance -> binding.
    let mut instance_set: std::collections::BTreeSet<String> = BTreeSet::new();
    let mut col_instance_map: HashMap<String, HashMap<String, VariableBinding>> = HashMap::new();

    for (col_oid, bindings) in &column_results {
        let mut inst_map: HashMap<String, VariableBinding> = HashMap::new();
        for binding in bindings {
            if let Some(suffix) = extract_instance_suffix(col_oid, &binding.oid) {
                inst_map.insert(suffix.clone(), binding.clone());
                instance_set.insert(suffix);
            }
        }
        col_instance_map.insert(col_oid.clone(), inst_map);
    }

    // Build rows from the sorted set of instance IDs.
    let mut rows: Vec<TableRow> = Vec::new();
    let mut missing_cells: usize = 0;
    let mut warnings: Vec<SnmpWarning> = Vec::new();

    for instance_id in &instance_set {
        let mut cells: BTreeMap<String, TableCell> = BTreeMap::new();
        for col_oid in &columns {
            if let Some(inst_map) = col_instance_map.get(col_oid) {
                if let Some(binding) = inst_map.get(instance_id) {
                    cells.insert(
                        col_oid.clone(),
                        TableCell {
                            value: Some(binding.clone()),
                            missing: false,
                        },
                    );
                } else {
                    missing_cells += 1;
                    cells.insert(
                        col_oid.clone(),
                        TableCell {
                            value: None,
                            missing: true,
                        },
                    );
                }
            } else {
                missing_cells += 1;
                cells.insert(
                    col_oid.clone(),
                    TableCell {
                        value: None,
                        missing: true,
                    },
                );
            }
        }
        rows.push(TableRow {
            instance_id: instance_id.clone(),
            cells,
        });
    }

    // Generate warnings for columns with inconsistent row counts.
    let max_rows = instance_set.len();
    for (col_oid, inst_map) in &col_instance_map {
        if inst_map.len() < max_rows {
            warnings.push(SnmpWarning {
                kind: "inconsistent-rows".to_string(),
                message: format!(
                    "Column {} has {} rows (expected {})",
                    col_oid,
                    inst_map.len(),
                    max_rows
                ),
                oid: Some(col_oid.clone()),
            });
        }
    }

    TableResult {
        table_oid: column_oid,
        columns,
        rows,
        total_rows: instance_set.len(),
        missing_cells,
        warnings,
        partial: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_instance_suffix_basic() {
        assert_eq!(
            extract_instance_suffix("1.3.6.1.2.1.2.2.1.2", "1.3.6.1.2.1.2.2.1.2.7"),
            Some("7".to_string())
        );
    }

    #[test]
    fn extract_instance_suffix_multidot() {
        assert_eq!(
            extract_instance_suffix("1.3.6.1.2.1.2.2.1.3", "1.3.6.1.2.1.2.2.1.3.192.168.1.1"),
            Some("192.168.1.1".to_string())
        );
    }

    #[test]
    fn extract_instance_suffix_exact_match() {
        assert_eq!(
            extract_instance_suffix("1.3.6.1.2.1.1.1", "1.3.6.1.2.1.1.1"),
            Some("0".to_string())
        );
    }

    #[test]
    fn extract_instance_suffix_no_match() {
        assert_eq!(
            extract_instance_suffix("1.3.6.1.2.1.2", "1.3.6.1.2.1.3.1"),
            None
        );
    }

    #[test]
    fn assemble_grid_consistent_data() {
        let col_oid = "1.3.6.1.2.1.2.2.1".to_string();
        let mut results: HashMap<String, Vec<VariableBinding>> = HashMap::new();

        // Column 2 (ifDescr): rows 1, 2, 3
        results.insert(
            format!("{}.2", col_oid),
            vec![
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.2.1".to_string(),
                    snmp2::Value::OctetString(b"eth0"),
                ),
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.2.2".to_string(),
                    snmp2::Value::OctetString(b"eth1"),
                ),
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.2.3".to_string(),
                    snmp2::Value::OctetString(b"lo"),
                ),
            ],
        );

        // Column 4 (ifMtu): rows 1, 2, 3
        results.insert(
            format!("{}.4", col_oid),
            vec![
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.4.1".to_string(),
                    snmp2::Value::Integer(1500),
                ),
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.4.2".to_string(),
                    snmp2::Value::Integer(1500),
                ),
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.4.3".to_string(),
                    snmp2::Value::Integer(65536),
                ),
            ],
        );

        let grid = assemble_table_grid(col_oid.clone(), results);

        assert_eq!(grid.total_rows, 3);
        assert_eq!(grid.missing_cells, 0);
        assert_eq!(grid.warnings.len(), 0);
        assert_eq!(grid.rows.len(), 3);

        // Check row 1 has both columns with values.
        let row1 = &grid.rows[0];
        assert_eq!(row1.instance_id, "1");
        assert!(!row1.cells.values().any(|c| c.missing));
    }

    #[test]
    fn assemble_grid_inconsistent_data() {
        let col_oid = "1.3.6.1.2.1.2.2.1".to_string();
        let mut results: HashMap<String, Vec<VariableBinding>> = HashMap::new();

        // Column 2 (ifDescr): rows 1, 2, 3
        results.insert(
            format!("{}.2", col_oid),
            vec![
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.2.1".to_string(),
                    snmp2::Value::OctetString(b"eth0"),
                ),
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.2.2".to_string(),
                    snmp2::Value::OctetString(b"eth1"),
                ),
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.2.3".to_string(),
                    snmp2::Value::OctetString(b"lo"),
                ),
            ],
        );

        // Column 4 (ifMtu): only rows 1, 2 (row 3 missing due to timeout)
        results.insert(
            format!("{}.4", col_oid),
            vec![
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.4.1".to_string(),
                    snmp2::Value::Integer(1500),
                ),
                binding_from_snmp(
                    "1.3.6.1.2.1.2.2.1.4.2".to_string(),
                    snmp2::Value::Integer(1500),
                ),
            ],
        );

        let grid = assemble_table_grid(col_oid.clone(), results);

        assert_eq!(grid.total_rows, 3);
        assert_eq!(grid.missing_cells, 1);
        assert_eq!(grid.warnings.len(), 1);

        // Row 3 should have a missing cell for column 4.
        let row3 = &grid.rows[2];
        assert_eq!(row3.instance_id, "3");
        let col4_oid = format!("{}.4", col_oid);
        assert!(row3.cells.get(&col4_oid).unwrap().missing);
    }

    #[test]
    fn assemble_grid_empty() {
        let col_oid = "1.3.6.1.2.1.2.2.1".to_string();
        let results: HashMap<String, Vec<VariableBinding>> = HashMap::new();

        let grid = assemble_table_grid(col_oid, results);

        assert_eq!(grid.total_rows, 0);
        assert_eq!(grid.missing_cells, 0);
        assert!(grid.rows.is_empty());
    }
}
