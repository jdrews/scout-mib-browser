# Scout MIB Browser - Design Specification (Rust/Tauri)

**Date:** 2026-07-15
**Status:** Accepted — Rust/Tauri approach

## Overview

Scout is an open-source, cross-platform SNMP MIB browser. It prioritizes reading MIBs, running SNMP walks against devices, and supporting SNMP v1, v2c, and v3. The tool emphasizes high tolerance for non-compliant or malformed device responses — it never fails silently; instead, it displays whatever data was received alongside warnings.

Inspired by iReasoning MIB Browser, but free and open source (MIT).

## Architecture

Two-tier architecture: Rust backend + Tauri web frontend with system WebView.

```
+-----------------------------------------------------+
|                 UI Layer (Tauri v2)                 |
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
|   |              Backend (Pure Rust)                | |
|   |                                                 | |
|   |  +-----------+  +----------------------------+  | |
|   |  | MIB       |  | SNMP Engine                |  | |
|   |  | Resolver  |  |                            |  | |
|   |  |           |  | - Get / GetBulk            |  | |
|   |  +-----------+  | - GetNext                  |  | |
|   |                 | - Walk / BulkWalk          |  | |
|   |  +-----------+  | - Set (multi-type)         |  | |
|   |  | Export      |  | - Table retrieval        |  | |
|   |  | Writers     |  |   (detect table, fetch   |  | |
|   |  |(TSV/JSON/CSV)|  |    all rows)             |  | |
|   |  +-----------+  +----------------------------+  | |
|   +-------------------------------------------------+ |
+-----------------------------------------------------+
```

- **UI Layer**: Tauri v2 with web frontend (HTML/CSS/JS). Uses system WebView (WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux). Frontend communicates with Rust backend via Tauri commands (IPC). Results stream back as batches of ~100 items for smooth live progress.
- **Backend**: Pure Rust crates with zero UI dependency. Fully testable independently. Exposes a simple interface: `connect()`, `walk(oid)`, `set(oid, value)`, `resolve_oid(oid)`, `export(format, data)`.

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Memory safety without GC, strong type system, single binary output |
| GUI Framework | Tauri v2 | Small binary (~5-10MB), uses system WebView, mature ecosystem (109k stars), active development |
| Frontend | HTML/CSS/JS with lightweight framework | Leverages web dev skills; Svelte or vanilla JS for simplicity |
| Cross-compilation | `cross` + Tauri CI templates | Standard Rust cross-compilation; Tauri provides GitHub Actions templates |
| SNMP Client | snmp2 | Pure Rust, supports v1/v2c/v3 (USM/VACM), async via tokio, MIT/Apache-2.0 dual license |
| MIB Parser | mib-rs + custom fallback | mib-rs handles SMIv1/SMIv2 natively with diagnostics; fallback regex extractor provides partial data when mib-rs fails on malformed vendor MIBs |
| Config | config crate (TOML) | Human-friendly format, env var overrides, defaults cascade, defaults to `~/.config/scout/config.toml` |

## Core Features (MVP)

### SNMP Operations
- **Get**: Single or multiple OIDs. Returns variable bindings.
- **GetNext**: Lexicographic next OID. Useful for iterating unknown subtrees.
- **Walk / BulkWalk**: Full subtree traversal via GetNext/GetBulk. Streams results to UI table as they arrive.
- **Table retrieval**: When the user selects a MIB node marked as `TABLE`, auto-detect column OIDs, then BulkWalk each column indexed by row instance IDs. Renders as a proper grid (rows = instances, columns = OIDs).
- **Set**: Write Integer, OctetString, Gauge32, Counter32/64, IpAddress, TimeTicks, or OBJECT IDENTIFIER values to a single OID. Requires explicit confirmation dialog before execution ("Are you sure?") showing target OID, current value (if readable), and proposed new value.

### Error Tolerance
The tool must never crash or abort on non-compliant device behavior:
- Timeout errors → retry up to 3 times with exponential backoff (1s, 2s, 4s); if all retries fail, log warning and return partial results collected so far instead of aborting
- `EndOfMibView` / `NoSuchInstance` → treat as normal termination for walks; surface in the "errors" section of export
- Malformed BER responses → decode what we can, mark unparseable values with raw hex bytes and a warning flag
- Agent returns unexpected ASN.1 types → display raw value + type code rather than crashing

### MIB Resolution (Hybrid Approach)
- **Primary path**: Scan all files in user-selected directories + bundled defaults, pre-filtering out binary and non-text files. Attempt to load each into `mib-rs`. The resolver handles IMPORT/EXPORT, macro expansion, and builds a complete OID-to-name/type index with diagnostics collection instead of fail-fast.
- **Fallback path**: If mib-rs fails to parse a specific MIB file (syntax error, unsupported construct), run a regex-based extractor that pulls OBJECT-TYPE blocks, name/SYNTAX mappings, and explicit numeric OID assignments.
- **Resolution API**: `resolve(oid) -> (name, mibName, syntaxType)` and `reverse_lookup(name) -> oid`. Both paths merge into a single index; mib-rs results take precedence, fallback fills gaps.
- **Fallback indicator**: A warning banner appears at the bottom of the MIB tree when any files were loaded via regex fallback. Clicking "System Log" shows per-file details about what was extracted and what was skipped.
- Parser errors are logged to the UI log window but never block loading other MIBs. Partially parsed MIBs still contribute whatever was extracted.

### Export Formats
Results can be exported in three formats:
1. **TSV (default)**: `oid\tname\ttype\thuman_value`. One row per variable binding. No header line. Easy to grep, pipe, or open in spreadsheet software.
2. **JSON**: Full metadata envelope including target info, timestamp, root OID, and an array of entries. Values are typed natively (numbers as numbers, strings as strings). Includes an `errors` array for non-fatal issues.
3. **CSV**: Same data as TSV but comma-delimited with proper RFC 4180 quoting for values containing commas or newlines.

### Table Retrieval
When the user selects a MIB node marked as `TABLE`, auto-detect column OIDs and BulkWalk each column indexed by row instance IDs. Renders as a proper grid (rows = instances, columns = OIDs). If the device returns inconsistent row data (e.g., column A has 50 rows but column B only 48 due to timeouts or agent quirks), perform best-effort merge: show all rows, leave missing cells blank with a warning indicator.

### Connection Management
Ad-hoc connections only. User enters host, port, version, community/credentials each time they connect. No credential storage.

### System Log
- **Rotating file log**: Writes to `~/.config/scout/scout.log` with rotation (5 files, 10MB each). Records all SNMP commands sent, queries run, MIB load events, errors, and warnings. Uses `tracing` + `tracing-appender`.
- **Dockable UI pane**: Hidden by default. Toggled via "System Log" button in the status bar next to connection state. Can be docked below results or popped out as a floating window. Shows real-time log entries with severity filtering (Error/Warning/Info).

### Config Management
- TOML config file at `~/.config/scout/config.toml` managed via `config` crate
- Persists: MIB directory paths, last-used connection settings, UI state (pane visibility, splitter positions)
- Supports environment variable overrides and defaults cascade

### Testing Strategy
- **Mock SNMP server only** — all tests use a Rust-based fake agent for fast, deterministic, network-free execution
- Covers: error tolerance paths, partial result collection, table detection/assembly, fallback MIB parsing, export formatting
- No real-device integration tests in CI (manual QA covers device quirks)


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
│ ├ iso          │ │  Results View (virtualized table)          │
│ │ └ org        │ │  ──────┼─────────────────┼───────┼──────  │
│ │   └ dod      │ │  OID   │ Name            │ Type  │ Val    │
│ │     └ internet│ │ ──────┼─────────────────┼───────┼──────  │
│ │       └ ...   │ │  ...1.0│ sysDescr.0      │String │Cisco  │
│                  │ │  ...3.0│ sysUpTime.0     │Ticks  │45d..  │
│                  │ │  ──────┴─────────────────┴───────┴──────  │
│                  ├────────────────────────────────────────────┤
│                  │ [Save Results ▼] (TSV / JSON / CSV)        │
├──────────────────┴────────────────────────────────────────────┤
│ Status: Connected | 1,247 OIDs retrieved    [System Log]      │
└───────────────────────────────────────────────────────────────┘
```

**Key interactions:**
- **OID Address Bar**: Typing an OID or MIB name (e.g., `IF-MIB::ifDescr`) and pressing Enter/Go triggers the selected operation. Features dropdown autocomplete showing matching OIDs and names as you type. Selecting a node in the tree updates the bar bidirectionally. Editing the bar and pressing Go navigates the tree to that OID if loaded, or performs the SNMP operation directly.
- **Operation Dropdown**: Adjacent to the address bar. Options: Walk, BulkWalk, Get, GetNext, Set. Determines what happens when "Go" is pressed.
- **Connection Popup (⚙)**: Clicking the gear icon next to Target opens a modal popup containing:
  - Version selector: v1 / v2c / v3
  - Community string field (v1/v2c)
  - v3 fields: Username, Auth Protocol/Passphrase, Priv Protocol/Passphrase
- **MIB Tree**: Read-only hierarchy of all loaded MIBs. Right-click context menu: "Copy OID", "Copy Name".
- **Results View**: Virtualized table component supporting column sorting and text search/filter across results. For tables detected via MIB metadata, switches to grid mode (rows = instances). Virtualization handles 10k+ rows smoothly.
- **Export Button**: "Save Results" dropdown lets the user pick TSV, JSON, or CSV. Filename defaults to `<target>_<root_oid_short>_<timestamp>.<ext>`. Uses Tauri `dialog` plugin for native file save dialog.
- **MIB Directory Management**: File menu → "Add MIB Directory" opens a folder picker via Tauri `dialog` plugin; selected directories are persisted in config. File menu → "Manage MIBs" shows currently loaded MIBs with ability to unload individual files.
- **System Log Toggle**: Clicking the "System Log" button in the status bar toggles a dockable log pane below results (or pops it out as a floating window). Shows real-time entries filtered by severity.

## Project Structure

```
scout-mib-browser/
├── src-tauri/                    # Tauri Rust backend
│   ├── src/
│   │   ├── main.rs               # Tauri app initialization, command wiring
│   │   ├── commands.rs           # Tauri command handlers (IPC bridge)
│   │   ├── mib/                  # MIB resolver (mib-rs wrapper + fallback parser)
│   │   │   ├── mod.rs            # Unified OID <-> name lookup API
│   │   │   ├── loader.rs         # Primary mib-rs-based loader
│   │   │   └── fallback.rs       # Regex-based fallback extractor
│   │   ├── snmp/                 # SNMP engine (snmp2 wrapper + tolerance logic)
│   │   │   ├── mod.rs            # Connect, Get, Walk, Set operations
│   │   │   ├── tolerant.rs       # Error handling, partial result collection, retry with backoff
│   │   │   └── table.rs          # Table detection and row assembly (best-effort merge)
│   │   ├── export/               # Export writers
│   │   │   ├── mod.rs            # Format selection interface
│   │   │   ├── tsv.rs            # TSV writer (default)
│   │   │   ├── json.rs           # JSON writer with metadata envelope
│   │   │   └── csv.rs            # CSV writer with RFC 4180 quoting
│   │   └── config.rs             # TOML-based config management via `config` crate
│   ├── mibs/                     # Bundled default MIBs (~50 files: core + network + security)
│   ├── tauri.conf.json           # Tauri configuration (app metadata, windows, plugins)
│   └── Cargo.toml                # Rust dependencies and build config
├── src/                          # Web frontend
│   ├── index.html                # Main HTML entry point
│   ├── css/                      # Stylesheets
│   ├── js/                       # JavaScript application logic
│   │   ├── app.js                # Main application controller
│   │   ├── mib-tree.js           # MIB tree view component
│   │   ├── results.js            # Results table component with virtualization
│   │   ├── connection.js         # Connection panel and modal
│   │   └── export.js             # Export dialog handlers
│   └── components/               # Reusable UI components
├── docs/specs/                   # Design specs
└── README.md
```

## Build & Cross-Compilation

Native development on Linux. Cross-compilation via `cross` and Tauri CI templates:
- **Linux**: `cargo tauri build` (native) or GitHub Actions with Ubuntu runner
- **Windows**: GitHub Actions with Windows runner, or `cross build --target x86_64-pc-windows-msvc`
- **macOS**: GitHub Actions with macOS runner (required for code signing and notarization)

First build per platform takes ~5 minutes (Rust compilation + WebView dependency setup). Subsequent builds leverage Cargo incremental compilation. Binaries use LTO (`lto = true` in `Cargo.toml`) for size optimization (~5-10MB final size with WebView bundled).

## Key Differences from Go Approach

| Aspect | Go (original) | Rust/Tauri (this spec) |
|--------|---------------|------------------------|
| GUI paradigm | miqt/Qt6 native widgets via CGO | Tauri web frontend + system WebView |
| Threading model | Goroutines + channels for streaming | Tokio async runtime + Tauri state management |
| IPC mechanism | Direct function calls (same process) | Tauri commands (structured IPC with serialization) |
| Streaming results | Go channels batched to ~100 items per `mainthread.Wait()` call | Tauri `emit()` events for real-time updates, or command return with pagination |
| Config format | HCL via viper | TOML via `config` crate |
| Logging | Custom rotating file logger | `tracing` + `tracing-appender` with rotation |
| Cross-compilation | miqt-docker containers | `cross` + platform-specific CI runners |
| Binary size | ~2MB stripped | ~5-10MB (smaller than Go+Qt, larger than Go alone) |
| Memory model | GC-managed heap | Compile-time ownership, no GC pauses |

## Out of Scope (Future)
- Trap receiver / trap sender
- Network discovery tools
- Performance graphing / polling
- Device snapshots / comparison tools
- Ping / traceroute utilities
