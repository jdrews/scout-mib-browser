# Scout MIB Browser

An SNMP MIB browser that reads MIB files, queries network devices, and displays results with high tolerance for malformed responses.

## Language

### Core Concepts

**Target**:
The SNMP device being queried — its address (host + port) and credentials combined into one concept. What the user points at.
_Avoid_: Agent, host, device, endpoint

**MIB Node**:
A named entry in a MIB schema file — has an OID, name, SYNTAX type, and metadata. Represents what *could* be queried, not live data.
_Avoid_: Schema node, definition, tree item

**Folder Collapse**:
The tree rendering rule that absorbs a run of empty folders — folders whose children are all subfolders — into their children: each child takes the parent's place with a dot-joined display name (`folder1.folder2`). Only the front pair of a run merges, so a rendered row carries at most two name parts; longer runs keep their tail as nested rows, which keeps meaningful names (tables, objects) fully visible. The merged row keeps the innermost real node's OID, so selecting it selects that node and expanding it loads its real children. TABLE containers are never absorbed — Get Table targets them by OID.
_Avoid_: Flattening, path naming, folder merging

**Variable Binding**:
An OID paired with its live value returned from a Target by an SNMP operation. The actual data, not the schema.
_Avoid_: Result row, binding, response entry

**Inspector**:
The pane that reports on one OID — its MIB Node metadata (name, type, clauses) and, when the OID was chosen from a Result Set, the live value too. Its subject comes from a Selection or from clicking a Variable Binding, so it may be an *instance* OID (e.g. `…sysDescr.0`) that is not itself a MIB Node; such OIDs resolve to their base node for metadata.
_Avoid_: Details panel, node info, properties

### Operations

**Selection**:
The act of choosing a MIB Node in the UI (tree click or address bar input). Populates the address bar but does not execute anything.
_Avoid_: Navigation, pick, choose

**Operation**:
The SNMP command mode — Walk, BulkWalk, Get, GetNext, Get Table, or Set. Determines what kind of request is sent to the Target and what shape the results take.
_Avoid_: Mode, action, command type

### Tables

**Table**:
A MIB node defined as a TABLE (SEQUENCE OF) — an ordered set of rows keyed by its INDEX clause. Distinguished from a plain object in the tree.
_Avoid_: Grid, dataset, table OID (when meaning the concept)

**Table Row**:
One instance of a Table, identified by its index suffix (the OIDs appended to each column's base). Rows keep walk order; they are never re-sorted by the engine.
_Avoid_: Record, entry, line

**Index Column**:
One component of a table's INDEX clause, rendered as its own narrow sortable column in the grid. An IMPLIED index component is absent from instance OIDs and renders blank with an "(implied)" tooltip.
_Avoid_: Key column, primary key (SQL connotations)

**Get Table**:
The Operation that retrieves a whole Table in one pass — a single subtree walk of the table's columns, streamed as progress, pivoted into rows in the backend. The only path to grid results; Walk/BulkWalk on a table stay flat.
_Avoid_: Fetch table, pull table, table query

**Result Set**:
The output of an Execution — contains Variable Bindings plus any non-fatal warnings or errors collected during tolerance handling. What gets displayed in the results view and exported to files.
_Avoid_: Query result, response payload, data set

**Raw View**:
A display mode of a Result Set that shows byte values as a hex + ASCII dump (offset, space-separated hex pairs, printable column with `.` for non-printable bytes) and scalars alongside their wire encoding (hex form, BER OID bytes, packed IP bytes). Byte values are also recognized by shape wherever they display: 6 bytes as a MAC address, 4 as IPv4, 16 as compressed IPv6; text wins over length-based matches.
_Avoid_: Hex mode, packet view, byte view

**Hex View**:
A modal showing the full, uncapped hex + ASCII dump of one byte-carrying Variable Binding — name, OID, BER type code, recognized interpretation, and a copy-to-clipboard action. Opened by right-clicking a value cell (flat result row or table grid cell), via the "… more bytes" hint when an inline Raw View dump overflows its row cap, or from the Hex view button in the Inspector's live-value block.
_Avoid_: Hex dialog, byte inspector, hex editor

**Execution**:
Triggering an Operation against a Target via the Go button. Takes the current Selection and Operation to produce a Result Set.
_Avoid_: Run, fire, query (too generic)
