# Scout MIB Browser - Design Specification

**Date:** 2026-07-13
**Status:** Approved by user

## Overview

Scout is an open-source, cross-platform SNMP MIB browser. It prioritizes reading MIBs, running SNMP walks against devices, and supporting SNMP v1, v2c, and v3. The tool emphasizes high tolerance for non-compliant or malformed device responses — it never fails silently; instead, it displays whatever data was received alongside warnings.

Inspired by iReasoning MIB Browser, but free and open source (MIT).

## Architecture

Two-tier architecture: pure Go backend + miqt/Qt6 GUI frontend.

```
+-----------------------------------------------------+
|                   UI Layer (miqt/Qt6)               |
|                                                     |
|   +-----------+  +-------------+  +---------------+ |
|   | MIB Tree  |  | Connection  |  | Results View  | |
|   | View      |  | Panel       |  | (Table/List)  | |
|   +-----+----+  +------+------+  +-------+-------+ |
|         |               |                   |       |
|         v               v                   v       |
|   +-------------------------------------------------+ |
|   |              Application Controller             | |
|   +------------------+------------------------------+ |
|                    |                                  |
|                    v                                  |
|   +-------------------------------------------------+ |
|   |              Backend (Pure Go)                  | |
|   |                                                 | |
|   |  +-----------+  +----------------------------+  | |
|   |  | MIB       |  | SNMP Engine                |  | |
|   |  | Resolver  |  |                            |  | |
|   |  |           |  | - Get / GetBulk            |  | |
|   |  +-----------+  | - GetNext                  |  | |
|   |                 | - Walk / BulkWalk          |  | |
|   |  +-----------+  | - Set (Int/String)         |  | |
|   |  | Export      |  | - Table retrieval         |  | |
|   |  | Writers     |  |   (detect table, fetch    |  | |
|   |  |(TSV/JSON/CSV)|  |    all rows)             |  | |
|   |  +-----------+  +----------------------------+  | |
|   +-------------------------------------------------+ |
+-----------------------------------------------------+
```

- **UI Layer**: miqt/Qt6 widgets. All UI runs on the Qt main thread (`runtime.LockOSThread()`). Backend calls execute in goroutines; results marshal back via `mainthread.Wait()`.
- **Backend**: Pure Go packages with zero Qt dependency. Fully testable independently. Exposes a simple interface: `Connect()`, `Walk(oid)`, `Set(oid, value)`, `ResolveOID(oid)`, `Export(format, data)`.

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Go | Fast compile, strong SNMP ecosystem, single binary output |
| GUI Framework | miqt (Qt6 bindings for Go via CGO) | Native look-and-feel, mature widget set, MIT license |
| Cross-compilation | miqt-docker | Prebuilt Docker containers per platform; one-liner builds for Linux/Windows/macOS |
| SNMP Client | gosnmp | Pure Go, supports v1/v2c/v3 (USM/VACM), 1.3k stars, actively maintained |
| MIB Parser | gosmi + custom fallback | gosmi handles SMIv1/SMIv2 natively; fallback regex extractor provides partial data when gosmi fails on malformed vendor MIBs |

## Core Features (MVP)

### SNMP Operations
- **Get**: Single or multiple OIDs. Returns variable bindings.
- **GetNext**: Lexicographic next OID. Useful for iterating unknown subtrees.
- **Walk / BulkWalk**: Full subtree traversal via GetNext/GetBulk. Streams results to UI table as they arrive.
- **Table retrieval**: When the user selects a MIB node marked as `TABLE`, auto-detect column OIDs, then BulkWalk each column indexed by row instance IDs. Renders as a proper grid (rows = instances, columns = OIDs).
- **Set**: Write Integer or OctetString values to a single OID.

### Error Tolerance
The tool must never crash or abort on non-compliant device behavior:
- Timeout errors → log warning, return partial results collected so far instead of aborting
- `EndOfMibView` / `NoSuchInstance` → treat as normal termination for walks; surface in the "errors" section of export
- Malformed BER responses → decode what we can, mark unparseable values with raw hex bytes and a warning flag
- Agent returns unexpected ASN.1 types → display raw value + type code rather than crashing

### MIB Resolution (Hybrid Approach)
- **Primary path**: Load all `.my` files from user-selected directories + bundled defaults into a `gosmi.Module`. Gosmi resolves IMPORT/EXPORT, macro expansion, and builds a complete OID-to-name/type index.
- **Fallback path**: If gosmi fails to parse a specific MIB file (syntax error, unsupported construct), run a regex-based extractor that pulls OBJECT-TYPE blocks, name/SYNTAX mappings, and explicit numeric OID assignments.
- **Resolution API**: `Resolve(oid) -> (name, mibName, syntaxType)` and `ReverseLookup(name) -> oid`. Both paths merge into a single index; gosmi results take precedence, fallback fills gaps.
- Parser errors are logged to the UI log window but never block loading other MIBs. Partially parsed MIBs still contribute whatever was extracted.

### Export Formats
Results can be exported in three formats:
1. **TSV (default)**: `oid\tname\ttype\thuman_value`. One row per variable binding. No header line. Easy to grep, pipe, or open in spreadsheet software.
2. **JSON**: Full metadata envelope including target info, timestamp, root OID, and an array of entries. Values are typed natively (numbers as numbers, strings as strings). Includes an `errors` array for non-fatal issues.
3. **CSV**: Same data as TSV but comma-delimited with proper RFC 4180 quoting for values containing commas or newlines.

### Connection Management
Ad-hoc connections only. User enters host, port, version, community/credentials each time they connect. No credential storage.

## UI Layout

```
┌───────────────────────────────────────────────────────────────┐
│ [Menu] File | View | Help                                      │
├───────────────────────────────────────────────────────────────┤
│ OID: [1.3.6.1.2.1.1.1.0  SNMPv2-MIB::sysDescr.0        ] [▼] [Go]│ <-- Address bar
├──────────────────┬────────────────────────────────────────────┤
│                  │  Target: [192.168.1.1] Port: [161] [⚙]    │
│ MIB Tree View    ├────────────────────────────────────────────┤
│ ┌──────────────┐ │                                            │
│ ├ iso          │ │  Results View (QTableView, virtualized)    │
│ │ └ org        │ │  ┌──────┬─────────────────┬───────┬──────┐ │
│ │   └ dod      │ │  │ OID  │ Name            │ Type  │ Val  │ │
│ │     └ internet│ │  ├──────┼─────────────────┼───────┼──────┤ │
│ │       └ ...   │ │  │...1.0│ sysDescr.0      │String │Cisco │ │
│                  │ │  │...3.0│ sysUpTime.0     │Ticks  │45d.. │ │
│                  │ │  └──────┴─────────────────┴───────┴──────┘ │
│                  ├────────────────────────────────────────────┤
│                  │ [Save Results ▼] (TSV / JSON / CSV)        │
├──────────────────┴────────────────────────────────────────────┤
│ Status: Connected | 1,247 OIDs retrieved                     │
└───────────────────────────────────────────────────────────────┘
```

**Key interactions:**
- **OID Address Bar**: Typing an OID or MIB name (e.g., `IF-MIB::ifDescr`) and pressing Enter/Go triggers the selected operation. Selecting a node in the tree updates the bar bidirectionally. Editing the bar and pressing Go navigates the tree to that OID if loaded, or performs the SNMP operation directly.
- **Operation Dropdown**: Adjacent to the address bar. Options: Walk, BulkWalk, Get, GetNext, Set. Determines what happens when "Go" is pressed.
- **Connection Popup (⚙)**: Clicking the gear icon next to Target opens a modal popup containing:
  - Version selector: v1 / v2c / v3
  - Community string field (v1/v2c)
  - v3 fields: Username, Auth Protocol/Passphrase, Priv Protocol/Passphrase
- **MIB Tree**: Read-only hierarchy of all loaded MIBs. Right-click context menu: "Copy OID", "Copy Name".
- **Results View**: `QTableView` backed by a custom `QAbstractTableModel`. Supports column sorting and text search/filter across results. For tables detected via MIB metadata, switches to grid mode (rows = instances). Qt handles virtualized rendering natively; we'll benchmark with 10k+ OIDs to confirm smoothness.
- **Export Button**: "Save Results" dropdown lets the user pick TSV, JSON, or CSV. Filename defaults to `<target>_<root_oid_short>_<timestamp>.<ext>`.

## Project Structure

```
scout-mib-browser/
├── cmd/scout/              # Main entry point + miqt UI layer
│   ├── main.go             # Qt app initialization, controller wiring
│   └── ui/                 # Qt widgets, models, views
├── internal/
│   ├── mib/                # MIB resolver (gosmi wrapper + fallback parser)
│   │   ├── resolver.go     # Unified OID <-> name lookup API
│   │   ├── gosmi_loader.go # Primary gosmi-based loader
│   │   └── fallback.go     # Regex-based fallback extractor
│   ├── snmp/               # SNMP engine (gosnmp wrapper + tolerance logic)
│   │   ├── engine.go       # Connect, Get, Walk, Set operations
│   │   ├── tolerant.go     # Error handling, partial result collection
│   │   └── table.go        # Table detection and row assembly
│   └── export/             # Export writers
│       ├── tsv.go          # TSV writer (default)
│       ├── json.go         # JSON writer with metadata envelope
│       └── csv.go          # CSV writer with RFC 4180 quoting
├── mibs/                   # Bundled default MIBs (IF-MIB, SNMPv2-MIB, etc.)
├── docs/specs/             # Design specs
└── go.mod
```

## Build & Cross-Compilation

Native development on Linux. Cross-compilation via `miqt-docker`:
- **Linux**: `go build -ldflags "-s -w"` (native) or `miqt-docker linux -build`
- **Windows**: `miqt-docker win64-qt6-static -windows-build`
- **macOS**: `miqt-docker macos -build`

First build per platform takes ~10 minutes (Qt compilation). Subsequent builds leverage Go cache bind-mounts for fast iteration. Binaries are stripped via `-ldflags "-s -w"` and optionally compressed with UPX (~2MB final size).

## Out of Scope (Future)
- Trap receiver / trap sender
- Network discovery tools
- Performance graphing / polling
- SNMPv3 USM user management
- Device snapshots / comparison tools
- Ping / traceroute utilities
