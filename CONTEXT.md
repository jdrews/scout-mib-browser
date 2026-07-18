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

**Variable Binding**:
An OID paired with its live value returned from a Target by an SNMP operation. The actual data, not the schema.
_Avoid_: Result row, binding, response entry

### Operations

**Selection**:
The act of choosing a MIB Node in the UI (tree click or address bar input). Populates the address bar but does not execute anything.
_Avoid_: Navigation, pick, choose

**Operation**:
The SNMP command mode — Walk, BulkWalk, Get, GetNext, or Set. Determines what kind of request is sent to the Target and what shape the results take.
_Avoid_: Mode, action, command type

**Result Set**:
The output of an Execution — contains Variable Bindings plus any non-fatal warnings or errors collected during tolerance handling. What gets displayed in the results view and exported to files.
_Avoid_: Query result, response payload, data set

**Execution**:
Triggering an Operation against a Target via the Go button. Takes the current Selection and Operation to produce a Result Set.
_Avoid_: Run, fire, query (too generic)
