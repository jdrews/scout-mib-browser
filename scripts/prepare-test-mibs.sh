#!/bin/bash
set -euo pipefail

# One-time preparation of the curated e2e MIB set in test/mibs/.
#
# Copies the real MIBs from the vendored ireasoning reference and writes the
# intentionally malformed BROKEN-MIB (which exercises the regex-fallback
# loading path). Idempotent — safe to re-run. The suite does not depend on
# /usr/share/snmp/mibs being present.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_MIBS="$REPO_ROOT/references/ireasoning/mibbrowser/mibs"
DST_MIBS="$REPO_ROOT/test/mibs"

if [ ! -d "$SRC_MIBS" ]; then
  echo "error: reference MIB directory not found: $SRC_MIBS" >&2
  exit 1
fi

mkdir -p "$DST_MIBS"

for mib in SNMPv2-MIB SNMPv2-SMI SNMPv2-TC IF-MIB; do
  cp "$SRC_MIBS/$mib" "$DST_MIBS/$mib"
done

cat > "$DST_MIBS/BROKEN-MIB" <<'EOF'
-- BROKEN-MIB
--
-- Intentionally malformed MIB module used by the e2e suite to exercise the
-- regex-fallback loading path (crates/scout-mib/src/fallback.rs).
--
-- A stray numeric literal precedes the module header (as seen in some vendor
-- exports), which mib-rs's strict parser rejects: it fails to produce the
-- module and the resolver falls back to the regex extractor. The OBJECT-TYPE
-- blocks below are kept well-formed so the fallback still recovers them.

123
BROKEN-MIB DEFINITIONS ::= BEGIN

IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, OCTET STRING
        FROM SNMPv2-SMI

brokenMibObjects OBJECT IDENTIFIER ::= { brokenMib 1 }

brokenThing OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "A well-formed object inside a malformed module."
    ::= { brokenMibObjects 1 }
EOF

# Two files that define the SAME module name ("DUP-MIB") under different file
# names. Regression fixture for the Manage MIBs page: its row list must be
# keyed per file (not per module name) or duplicate names hang the page on
# "Loading..." (Svelte each_key_duplicate).
cat > "$DST_MIBS/DUP-MIB-A" <<'EOF'
-- DUP-MIB (part 1 of 2) — e2e regression fixture.
--
-- Both DUP-MIB-A and DUP-MIB-B define the SAME MIB module name "DUP-MIB".
-- This mirrors real directories that contain several versions of one module
-- (e.g. RFC1623.MIB + RFC1643.MIB + EtherLike-MIB all defining EtherLike-MIB).
-- The Manage MIBs page must list BOTH rows without hanging on "Loading...":
-- its {#each} key must be unique per row (file path), not per module name.

DUP-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, enterprises
        FROM SNMPv2-SMI;

dupMibA MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "Duplicate-module fixture, part A."
    ::= { enterprises 99201 }

dupMibAObjects OBJECT IDENTIFIER ::= { dupMibA 1 }

dupThingA OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Fixture object A."
    ::= { dupMibAObjects 1 }
END
EOF

cat > "$DST_MIBS/DUP-MIB-B" <<'EOF'
-- DUP-MIB (part 2 of 2) — e2e regression fixture.
--
-- See DUP-MIB-A: this file defines the SAME module name "DUP-MIB" under a
-- different file name, so loaded_mibs() reports two entries sharing one
-- mibName. The Manage MIBs page must render both rows (keyed by file
-- path) instead of hanging on "Loading...".

DUP-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, enterprises
        FROM SNMPv2-SMI;

dupMibB MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "Duplicate-module fixture, part B."
    ::= { enterprises 99202 }

dupMibBObjects OBJECT IDENTIFIER ::= { dupMibB 1 }

dupThingB OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Fixture object B."
    ::= { dupMibBObjects 1 }
END
EOF

# Explicit-file fixture: a MIB that lives OUTSIDE test/mibs and is added via
# the `mib.files` config (the Manage MIBs page "File(s)…" source type). The
# e2e harness seeds it in the config so startup loads it and the page can
# verify it persists across an Unload All + reload.
EXTRA_DIR="$REPO_ROOT/test/mibs-extra"
mkdir -p "$EXTRA_DIR"
cat > "$EXTRA_DIR/EXTRA-MIB" <<'EOF'
-- EXTRA-MIB — e2e explicit-file fixture.
--
-- Loaded via the `mib.files` config entry (not a configured directory),
-- exercising the persistent explicit-file source type.

EXTRA-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, enterprises
        FROM SNMPv2-SMI;

extraMib MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "Test"
    CONTACT-INFO "test@test.com"
    DESCRIPTION "Explicit-file fixture."
    ::= { enterprises 99203 }

extraMibObjects OBJECT IDENTIFIER ::= { extraMib 1 }

extraThing OBJECT-TYPE
    SYNTAX Integer32
    MAX-ACCESS read-only
    STATUS current
    DESCRIPTION "Fixture object."
    ::= { extraMibObjects 1 }
END
EOF

echo "Prepared test MIBs in $DST_MIBS:"
ls -1 "$DST_MIBS"
echo "Prepared explicit-file fixture in $EXTRA_DIR:"
ls -1 "$EXTRA_DIR"
