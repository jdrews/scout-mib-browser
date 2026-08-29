//! SNMP Engine module — tolerant operations against a Target.
//!
//! Provides Connect, Get, GetNext, Walk, BulkWalk, Get Table, and Set operations with:
//! - Support for SNMP v1, v2c, and v3 (USM/VACM)
//! - Retry with exponential backoff on timeout (3x: 1s, 2s, 4s)
//! - EndOfMibView/NoSuchInstance treated as normal walk termination
//! - Malformed BER responses decoded with raw hex + warning for unparseable values
//! - Unexpected ASN.1 types displayed as raw value + type code

mod engine;
pub mod mock;
pub mod table;
mod tolerant;
pub mod types;

pub use engine::{SnmpEngine, TableRowSender, WalkBatchSender};
pub use mock::MockSnmpServer;
pub use table::*;
pub use types::*;
