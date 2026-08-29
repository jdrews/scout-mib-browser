use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::types::*;

/// Wire-level encoding strategy for one index component.
///
/// Local mirror of `scout_mib::IndexEncoding` so the pure SNMP crate stays
/// independent of the MIB crate; the app crate maps between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexEncoding {
    /// One sub-identifier (Integer32 and friends).
    Integer,
    /// Four sub-identifiers, each 0-255.
    IpAddress,
    /// A fixed number of sub-identifiers (OCTET STRING with SIZE(n)).
    FixedString(usize),
    /// Variable length — consumes the remainder of the suffix as opaque data.
    Variable,
}

/// One index component from a table's INDEX clause, in clause order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexColumnSpec {
    /// Component name (e.g., "ifIndex").
    pub name: String,
    /// Whether the component is marked IMPLIED (absent from instance OIDs).
    pub implied: bool,
    /// How many sub-identifiers this component consumes.
    pub encoding: IndexEncoding,
}

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

// NOTE: these structs intentionally serialize snake_case (no rename_all) to
// match the frontend's TableRowData/TableResult contract in src/lib/types.ts.
// `cells` must NOT be flattened — the frontend reads row.cells[colOid].

/// A single row in a table grid, keyed by its instance suffix.
#[derive(Debug, Clone, Serialize)]
pub struct TableRow {
    /// The instance suffix that identifies this row (e.g., "1" or "192.168.1.1").
    pub instance_id: String,
    /// Column OID -> cell data mapping.
    pub cells: BTreeMap<String, TableCell>,
    /// Decoded index component values, aligned with the table's INDEX clause
    /// order (`None` = IMPLIED component). Empty when the table has no index
    /// metadata or the suffix is undecodable — the UI then shows the raw
    /// instance suffix instead.
    pub index_values: Vec<Option<String>>,
}

/// Result of a table retrieval operation — pivoted grid of rows and columns.
#[derive(Debug, Clone, Serialize)]
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
    binding_oid
        .strip_prefix(&format!("{}.", column_oid))
        .map(|suffix| suffix.to_string())
}

/// Decodes an instance suffix into per-component index values.
///
/// Consumes sub-identifiers left-to-right according to each component's
/// encoding: Integer takes 1, IpAddress takes 4 (each 0-255), FixedString(n)
/// takes n (each 0-255), Variable consumes the remainder as an opaque string.
/// IMPLIED components are skipped — they are not present in the instance OID
/// at all — and yield `None`. Returns `None` when the suffix does not match
/// the index structure (undecodable), in which case callers fall back to the
/// raw suffix.
pub fn decode_instance_suffix(
    suffix: &str,
    columns: &[IndexColumnSpec],
) -> Option<Vec<Option<String>>> {
    let parts: Vec<&str> = suffix.split('.').collect();
    let mut pos = 0usize;
    let mut values: Vec<Option<String>> = Vec::with_capacity(columns.len());

    for col in columns {
        if col.implied {
            values.push(None);
            continue;
        }
        match col.encoding {
            IndexEncoding::Integer => {
                let part = parts.get(pos)?;
                part.parse::<u64>().ok()?;
                values.push(Some(part.to_string()));
                pos += 1;
            }
            IndexEncoding::IpAddress => {
                if pos + 4 > parts.len() {
                    return None;
                }
                let mut octets = Vec::with_capacity(4);
                for p in &parts[pos..pos + 4] {
                    let v: u32 = p.parse().ok()?;
                    if v > 255 {
                        return None;
                    }
                    octets.push(v.to_string());
                }
                values.push(Some(octets.join(".")));
                pos += 4;
            }
            IndexEncoding::FixedString(n) => {
                let n = n.max(1);
                if pos + n > parts.len() {
                    return None;
                }
                for p in &parts[pos..pos + n] {
                    let v: u32 = p.parse().ok()?;
                    if v > 255 {
                        return None;
                    }
                }
                values.push(Some(parts[pos..pos + n].join(".")));
                pos += n;
            }
            IndexEncoding::Variable => {
                // Opaque remainder — only correct when this is the final
                // component, which it must be to leave nothing behind.
                if pos >= parts.len() {
                    return None;
                }
                values.push(Some(parts[pos..].join(".")));
                pos = parts.len();
            }
        }
    }

    (pos == parts.len()).then_some(values)
}

/// Assembles a grid of rows from a single subtree walk of the table.
///
/// Bindings arrive in walk (encounter) order, so rows are emitted in first-
/// encounter order — numeric index order, with no string sorting (G3). A
/// binding is assigned to a row only if its OID starts with one of the
/// requested column OIDs; anything else (e.g., nested sub-table data) is
/// excluded from the grid (G4). Missing cells are marked `missing: true` and
/// columns with fewer rows than the maximum produce an `inconsistent-rows`
/// warning. When index metadata is provided, each row's suffix is decoded
/// into per-component values; if any row fails to decode, all rows fall back
/// to the raw instance suffix (empty `index_values`).
pub fn assemble_table_walk(
    table_oid: String,
    column_oids: Vec<String>,
    bindings: Vec<VariableBinding>,
    index_columns: &[IndexColumnSpec],
) -> TableResult {
    let mut rows: Vec<TableRow> = Vec::new();
    let mut row_index: HashMap<String, usize> = HashMap::new();
    let mut col_counts: HashMap<String, usize> = HashMap::new();

    for binding in &bindings {
        // Pivot on known columns only — longest column-OID prefix wins.
        let mut matched: Option<(String, String)> = None;
        for col in &column_oids {
            if binding.oid == *col {
                continue; // bare column OID — not a row instance
            }
            let Some(suffix) = binding.oid.strip_prefix(&format!("{col}.")) else {
                continue;
            };
            let better = match &matched {
                None => true,
                Some((m, _)) => col.len() > m.len(),
            };
            if better {
                matched = Some((col.clone(), suffix.to_string()));
            }
        }
        let Some((col_oid, suffix)) = matched else {
            continue; // nested sub-table or unrelated data (G4)
        };

        col_counts.insert(
            col_oid.clone(),
            col_counts.get(&col_oid).copied().unwrap_or(0) + 1,
        );
        let row_pos = match row_index.get(&suffix) {
            Some(p) => *p,
            None => {
                let p = rows.len();
                row_index.insert(suffix.clone(), p);
                rows.push(TableRow {
                    instance_id: suffix,
                    cells: BTreeMap::new(),
                    index_values: Vec::new(),
                });
                p
            }
        };
        rows[row_pos].cells.insert(
            col_oid.clone(),
            TableCell {
                value: Some(binding.clone()),
                missing: false,
            },
        );
    }

    // Fill missing cells and count them.
    let total_rows = rows.len();
    let mut missing_cells = 0;
    for row in &mut rows {
        for col in &column_oids {
            if !row.cells.contains_key(col) {
                missing_cells += 1;
                row.cells.insert(
                    col.clone(),
                    TableCell {
                        value: None,
                        missing: true,
                    },
                );
            }
        }
    }

    // Warnings for columns with fewer rows than the maximum.
    let mut warnings: Vec<SnmpWarning> = Vec::new();
    for col in &column_oids {
        let n = col_counts.get(col).copied().unwrap_or(0);
        if total_rows > 0 && n < total_rows {
            warnings.push(SnmpWarning {
                kind: "inconsistent-rows".to_string(),
                message: format!("Column {} has {} rows (expected {})", col, n, total_rows),
                oid: Some(col.clone()),
            });
        }
    }

    // Decode index values per row; all-or-nothing so the grid never mixes
    // decoded and raw index columns.
    if !index_columns.is_empty() && !rows.is_empty() {
        let decoded: Vec<Option<Vec<Option<String>>>> = rows
            .iter()
            .map(|r| decode_instance_suffix(&r.instance_id, index_columns))
            .collect();
        if decoded.iter().all(Option::is_some) {
            for (row, values) in rows.iter_mut().zip(decoded.into_iter()) {
                row.index_values = values.expect("checked all Some above");
            }
        }
    }

    TableResult {
        table_oid,
        columns: column_oids,
        rows,
        total_rows,
        missing_cells,
        warnings,
        partial: false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::tolerant::binding_from_snmp;
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

    fn spec(name: &str, implied: bool, encoding: IndexEncoding) -> IndexColumnSpec {
        IndexColumnSpec {
            name: name.to_string(),
            implied,
            encoding,
        }
    }

    #[test]
    fn decode_integer_index() {
        let cols = [spec("ifIndex", false, IndexEncoding::Integer)];
        assert_eq!(
            decode_instance_suffix("7", &cols),
            Some(vec![Some("7".to_string())])
        );
    }

    #[test]
    fn decode_multi_attribute_index() {
        let cols = [
            spec("idx", false, IndexEncoding::Integer),
            spec("addr", false, IndexEncoding::IpAddress),
        ];
        assert_eq!(
            decode_instance_suffix("3.192.168.1.10", &cols),
            Some(vec![
                Some("3".to_string()),
                Some("192.168.1.10".to_string())
            ])
        );
    }

    #[test]
    fn decode_implied_component_skipped_not_consumed() {
        // An IMPLIED component's value is not present in the instance OID at
        // all (RFC 2578 §7.7) — it renders blank and consumes no sub-ids, so
        // the suffix starts at the second component's value.
        let cols = [
            spec("mac", true, IndexEncoding::FixedString(6)),
            spec("port", false, IndexEncoding::Integer),
        ];
        assert_eq!(
            decode_instance_suffix("7", &cols),
            Some(vec![None, Some("7".to_string())])
        );

        // A suffix that still carries the implied value is a structure
        // mismatch — undecodable.
        assert_eq!(decode_instance_suffix("48.230.175.16.96.1.7", &cols), None);
    }

    #[test]
    fn decode_fixed_string_and_variable_tail() {
        let cols = [
            spec("tag", false, IndexEncoding::FixedString(2)),
            spec("rest", false, IndexEncoding::Variable),
        ];
        assert_eq!(
            decode_instance_suffix("104.101.5.9.7", &cols),
            Some(vec![Some("104.101".to_string()), Some("5.9.7".to_string())])
        );
    }

    #[test]
    fn decode_rejects_out_of_range_ip_octet() {
        let cols = [spec("addr", false, IndexEncoding::IpAddress)];
        assert_eq!(decode_instance_suffix("256.1.1.1", &cols), None);
    }

    #[test]
    fn decode_rejects_too_few_subidentifiers() {
        let cols = [spec("idx", false, IndexEncoding::Integer)];
        // Non-numeric suffix — undecodable, caller falls back to raw.
        assert_eq!(decode_instance_suffix("abc", &cols), None);
        let ip_cols = [spec("addr", false, IndexEncoding::IpAddress)];
        assert_eq!(decode_instance_suffix("1.2.3", &ip_cols), None);
    }

    #[test]
    fn decode_rejects_leftover_subidentifiers() {
        // Integer index but suffix has two components — structure mismatch.
        let cols = [spec("idx", false, IndexEncoding::Integer)];
        assert_eq!(decode_instance_suffix("1.2", &cols), None);
    }

    #[test]
    fn assemble_walk_rows_in_encounter_order() {
        // G3 regression: 12 rows on an integer index must keep walk order
        // (2 before 10), not lexicographic string order.
        let table_oid = "1.3.6.1.4.1.99997.1".to_string();
        let col_oid = format!("{table_oid}.1.1");
        let mut bindings: Vec<VariableBinding> = Vec::new();
        for i in 1..=12 {
            bindings.push(binding_from_snmp(
                format!("{col_oid}.{i}"),
                snmp2::Value::Integer(i),
            ));
        }

        let grid = assemble_table_walk(table_oid, vec![col_oid], bindings, &[]);

        assert_eq!(grid.total_rows, 12);
        let order: Vec<&str> = grid.rows.iter().map(|r| r.instance_id.as_str()).collect();
        assert_eq!(order, (1..=12).map(|i| i.to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn assemble_walk_pivots_multiple_columns() {
        let table_oid = "1.3.6.1.2.1.2.2.1".to_string();
        let col2 = format!("{table_oid}.2"); // ifDescr
        let col4 = format!("{table_oid}.4"); // ifMtu

        // Column-major walk order: all of col2, then all of col4.
        let bindings = vec![
            binding_from_snmp(format!("{col2}.1"), snmp2::Value::OctetString(b"eth0")),
            binding_from_snmp(format!("{col2}.2"), snmp2::Value::OctetString(b"eth1")),
            binding_from_snmp(format!("{col4}.1"), snmp2::Value::Integer(1500)),
            binding_from_snmp(format!("{col4}.2"), snmp2::Value::Integer(65536)),
        ];

        let grid = assemble_table_walk(table_oid, vec![col2.clone(), col4.clone()], bindings, &[]);

        assert_eq!(grid.total_rows, 2);
        assert_eq!(grid.missing_cells, 0);
        assert!(grid.warnings.is_empty());
        let row1 = &grid.rows[0];
        assert_eq!(row1.instance_id, "1");
        assert!(!row1.cells.values().any(|c| c.missing));
    }

    #[test]
    fn assemble_walk_missing_cell_flagged() {
        let table_oid = "1.3.6.1.2.1.2.2.1".to_string();
        let col2 = format!("{table_oid}.2");
        let col4 = format!("{table_oid}.4");

        // Row 3 exists in col2 but not col4 (e.g., timeout on that binding).
        let bindings = vec![
            binding_from_snmp(format!("{col2}.1"), snmp2::Value::OctetString(b"eth0")),
            binding_from_snmp(format!("{col2}.2"), snmp2::Value::OctetString(b"eth1")),
            binding_from_snmp(format!("{col2}.3"), snmp2::Value::OctetString(b"lo")),
            binding_from_snmp(format!("{col4}.1"), snmp2::Value::Integer(1500)),
            binding_from_snmp(format!("{col4}.2"), snmp2::Value::Integer(1500)),
        ];

        let grid = assemble_table_walk(table_oid, vec![col2.clone(), col4.clone()], bindings, &[]);

        assert_eq!(grid.total_rows, 3);
        assert_eq!(grid.missing_cells, 1);
        assert_eq!(grid.warnings.len(), 1);
        assert_eq!(grid.warnings[0].kind, "inconsistent-rows");
        let row3 = &grid.rows[2];
        assert!(row3.cells.get(&col4).unwrap().missing);
    }

    #[test]
    fn assemble_walk_excludes_nested_subtable_data() {
        // G4 regression: bindings under a nested sub-table are not columns of
        // the outer table and must not create misaligned rows.
        let table_oid = "1.3.6.1.4.1.99997.2".to_string();
        let col = format!("{table_oid}.1.1"); // outer column
        let nested_col = format!("{table_oid}.3.1.1"); // sub-table column

        let bindings = vec![
            binding_from_snmp(format!("{col}.1"), snmp2::Value::Integer(10)),
            binding_from_snmp(format!("{nested_col}.9"), snmp2::Value::Integer(99)),
            binding_from_snmp(format!("{col}.2"), snmp2::Value::Integer(20)),
        ];

        let grid = assemble_table_walk(table_oid, vec![col], bindings, &[]);

        assert_eq!(grid.total_rows, 2);
        let ids: Vec<&str> = grid.rows.iter().map(|r| r.instance_id.as_str()).collect();
        assert_eq!(ids, vec!["1", "2"]);
    }

    #[test]
    fn assemble_walk_decodes_index_values() {
        let table_oid = "1.3.6.1.4.1.99997.3".to_string();
        let col = format!("{table_oid}.1.1");
        let bindings = vec![
            binding_from_snmp(format!("{col}.5"), snmp2::Value::Integer(5)),
            binding_from_snmp(format!("{col}.6"), snmp2::Value::Integer(6)),
        ];
        let cols = [spec("idx", false, IndexEncoding::Integer)];

        let grid = assemble_table_walk(table_oid, vec![col], bindings, &cols);

        assert_eq!(grid.rows[0].index_values, vec![Some("5".to_string())]);
        assert_eq!(grid.rows[1].index_values, vec![Some("6".to_string())]);
    }

    #[test]
    fn assemble_walk_undecodable_falls_back_to_raw() {
        // Integer index metadata but a non-numeric suffix: all rows fall back
        // to the raw instance column (empty index_values).
        let table_oid = "1.3.6.1.4.1.99997.4".to_string();
        let col = format!("{table_oid}.1.1");
        let bindings = vec![binding_from_snmp(
            format!("{col}.abc"),
            snmp2::Value::Integer(1),
        )];
        let cols = [spec("idx", false, IndexEncoding::Integer)];

        let grid = assemble_table_walk(table_oid, vec![col], bindings, &cols);

        assert_eq!(grid.total_rows, 1);
        assert!(grid.rows[0].index_values.is_empty());
    }

    #[test]
    fn assemble_walk_empty() {
        let grid = assemble_table_walk(
            "1.3.6.1.2.1.2.2.1".to_string(),
            vec!["1.3.6.1.2.1.2.2.1.2".to_string()],
            Vec::new(),
            &[],
        );

        assert_eq!(grid.total_rows, 0);
        assert_eq!(grid.missing_cells, 0);
        assert!(grid.rows.is_empty());
    }
}
