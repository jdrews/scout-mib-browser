# Raw View (Hex + Text) of OID Results

**Date:** 2026-09-05
**Status:** Implemented — merged to main via PR #67

## Purpose

SNMP results that carry bytes (OctetString, unknown-type Raw values) were displayed as either forced text or one long `0x…` blob. This makes binary data hard to read and hides recognizable patterns — a 6-byte PhysAddress rendered as hex instead of a MAC address. This feature adds:

1. **Raw mode** in the results pane (a "Raw" toggle next to Wrap): byte values render as a Wireshark-style dump — offset, space-separated hex, ASCII column (non-printable bytes as `.`) — and scalars show their wire encoding alongside the decoded value (`0x00010000 (65536 counter32)`, OIDs with their BER bytes, IPs with packed bytes).
2. **Readable binary by default**: outside Raw mode, OctetStrings that are not text render as space-separated hex pairs instead of a continuous `0x…` string; empty octets render as `""`.
3. **Pattern recognition** for byte values: 6 bytes → MAC (`00:12:79:62:f9:40`), 4 bytes → IPv4, 16 bytes → compressed IPv6 (RFC 5952). Text wins over length-based matches, so a 4-byte string like `eth0` stays text.
4. **Inspector hex dump**: clicking a byte value (result row or grid cell) shows the full dump under the live value, with the recognized pattern named ("recognized as mac address") and, for Raw values, the ASN.1 type code.

## Design decisions

- **Recognition is length + content based, not MIB-type based.** The value's SYNTAX (PhysAddress vs OctetString) is metadata; the bytes themselves carry the shape. Text detection (≥90% printable ASCII, or valid UTF-8 with no C0 controls) takes priority over length matches.
- **Raw mode affects presentation only** — filtering, sorting, and export keep using the readable display strings, so a Raw-mode session exports the same files as before.
- **Inline dumps are capped** (16 rows / 256 bytes in the list, 64 rows / 1KB in the inspector) with a "… N more bytes" note; the inspector is the place for long values.
- **Raw values are data, not faults.** An unknown ASN.1 type arrives as `Raw { type_code, data }` with intact bytes; it is presented through the same pipeline as OctetStrings (pattern recognition, then space-separated hex) and does not set the binding's warning flag — no green row, no triangle. Exception sentinels (noSuchObject/noSuchInstance/endOfMibView) remain flagged, since those signal missing data. The type code stays visible in raw mode and the inspector.
- **The value never repeats its type.** Display strings carry no `(counter32)`/`(timeticks)` suffixes — the Type column (and the export's type field) already says it.

## Domain language

New term for `CONTEXT.md`:

**Raw View**:
The results presentation that shows byte values as a hex + ASCII dump (offset, hex pairs, printable column) and scalars with their wire encoding. A display mode of the Result Set, not a different kind of result.
_Avoid_: Hex mode, packet view, byte view

## Tests

- Unit: `src/lib/hexdump.test.ts` (dump layout, text heuristic, MAC/IPv4/IPv6 recognition incl. RFC 5952 compression edge cases, BER OID encoding), `src/lib/export.test.ts` (valueDisplay/rawValueDisplay/inspectorValueOf), `InspectorPane.test.ts` (hex-dump section, type code, row cap).
- E2E: `test/specs/raw-view.spec.ts` — MAC recognition against the pinned linux-full-walk recording (`ifPhysAddress.2 = 00127962f940`), Raw toggle on/off round-trip, scalar wire encoding, inspector dump.
