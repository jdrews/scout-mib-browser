#!/usr/bin/env python3
"""Generate the synthetic snmpsim data file for the e2e grid tests.

Covers the spec's required synthetic cases (one recording, one agent):

1. ifStackTable (IF-MIB) — multi-component integer index, >500 rows:
     INDEX { ifStackHigherLayer, ifStackLowerLayer }   (both Integer)
       ifStackTable        1.3.6.1.2.1.31.1.2
       ifStackEntry        1.3.6.1.2.1.31.1.2.1
       ifStackHigherLayer  1.3.6.1.2.1.31.1.2.1.1  (not-accessible, index)
       ifStackLowerLayer   1.3.6.1.2.1.31.1.2.1.2  (not-accessible, index)
       ifStackStatus       1.3.6.1.2.1.31.1.2.1.3  (RowStatus/Integer)
   The 600 rows are (H, L) pairs with H in 1..30 and L ≡ H-1 (mod 30), so
   every pair is unique and the row count exceeds the grid's 500-row render
   chunk. Only the accessible column is recorded — a real agent never returns
   not-accessible index objects.

2. synthIpTable (SYNTH-TABLE-MIB) — two-attribute index, Integer + IpAddress:
     INDEX { synthIpRow, synthIpAddr }
       table 1.3.6.1.2.1.15432.1.1, entry ...1.1.1
       columns: synthIpStatus ...1.1.1.3 (Integer32), synthIpNote ...1.1.1.4
   12 rows (r, 10.0.r.1); synthIpNote is missing on row 7 — the missing-cell
   path.

3. synthImpTable (SYNTH-TABLE-MIB) — IMPLIED index component:
     INDEX { synthImpKey, IMPLIED synthImpIp }
       table 1.3.6.1.2.1.15432.1.2, entry ...1.2.1
       column: synthImpState ...1.2.1.3 (Integer32)
   5 rows keyed by integer only — the implied IpAddress is absent from the
   instance OID.

IMPORTANT: lines must be sorted by numeric OID order. snmpsim serves a
GETNEXT that exact-matches a record with the *next line in the file*, so an
unsorted recording breaks walk succession across multi-component indexes.

snmprec line format: OID|tag|value with tags from SnmprecGrammar.TAG_MAP
(2 = Integer32, 4 = OctetString, ...). Deterministic output; regenerate with:
    python3 scripts/gen-synthetic-ifstack.py > test/snmprec/synthetic-ifstack.snmprec
"""

import sys

# ifStackTable (IF-MIB): status column only.
IFSTACK_STATUS_OID = "1.3.6.1.2.1.31.1.2.1.3"
HIGHER_SPAN = 30
LOWER_MAX = 600

# synthIpTable (SYNTH-TABLE-MIB): Integer + IpAddress index.
SYNTH_BASE = "1.3.6.1.2.1.15432"
SYNTH_IP_STATUS_OID = f"{SYNTH_BASE}.1.1.1.3"
SYNTH_IP_NOTE_OID = f"{SYNTH_BASE}.1.1.1.4"
SYNTH_IP_ROWS = 12
SYNTH_IP_MISSING_ROW = 7

# synthImpTable (SYNTH-TABLE-MIB): IMPLIED IpAddress index component.
SYNTH_IMP_STATE_OID = f"{SYNTH_BASE}.1.2.1.3"
SYNTH_IMP_ROWS = 5


def oid_key(oid: str) -> tuple:
    return tuple(int(p) for p in oid.split("."))


def main() -> int:
    lines = []

    # 1. ifStackTable: (higher, lower) pairs — already numeric OID order.
    for higher in range(1, HIGHER_SPAN + 1):
        for lower in range(higher, LOWER_MAX + 1, HIGHER_SPAN):
            lines.append(f"{IFSTACK_STATUS_OID}.{higher}.{lower}|2|1")

    # 2. synthIpTable: row r at 10.0.r.1; note missing on one row.
    for r in range(1, SYNTH_IP_ROWS + 1):
        suffix = f".{r}.10.0.{r}.1"
        lines.append(f"{SYNTH_IP_STATUS_OID}{suffix}|2|{r}")
        if r != SYNTH_IP_MISSING_ROW:
            lines.append(f"{SYNTH_IP_NOTE_OID}{suffix}|4|note-{r}")

    # 3. synthImpTable: integer key only (implied address absent from OID).
    for k in range(1, SYNTH_IMP_ROWS + 1):
        lines.append(f"{SYNTH_IMP_STATE_OID}.{k}|2|{k * 100}")

    for line in sorted(lines, key=lambda l: oid_key(l.split("|", 1)[0])):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
