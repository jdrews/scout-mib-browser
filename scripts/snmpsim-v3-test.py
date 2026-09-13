#!/usr/bin/env python3
"""Start a snmpsim-command-responder configured with SNMPv3 USM users.

Serves the default v3 context (empty context name) from a recording, so any
SNMPv3 client can authenticate against the users below and walk the data.

The user/passphrase matrix is FIXED and shared with the Rust integration tests
(crates/scout-snmp/tests/snmpsim_integration.rs). Do not change these values
without updating that test file too.

Data files:
  - self.snmprec   -> serves the default SNMPv3 context (empty context name)
  - public.snmprec -> also serves v2c community "public" on the same endpoint

Usage:
  python3 scripts/snmpsim-v3-test.py                 # port 11700
  python3 scripts/snmpsim-v3-test.py --port 12000    # custom port
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile

DEFAULT_REC_FILE = "linux-full-walk.snmprec"
DEFAULT_PORT = 11700

# (username, auth_proto, auth_pass, priv_proto, priv_pass)
# proto "" means NONE. Passphrases must be >= 8 chars for snmpsim.
V3_USERS = [
    ("v3_noauth", "", "", "", ""),
    ("v3_md5", "MD5", "auctoritas-md5", "", ""),
    ("v3_sha", "SHA", "auctoritas-sha", "", ""),
    ("v3_sha224", "SHA224", "auctoritas-sha224", "", ""),
    ("v3_sha256", "SHA256", "auctoritas-sha256", "", ""),
    ("v3_sha384", "SHA384", "auctoritas-sha384", "", ""),
    ("v3_sha512", "SHA512", "auctoritas-sha512", "", ""),
    ("v3_des_md5", "MD5", "authpass-md5", "DES", "privpass-des"),
    ("v3_aes128_sha", "SHA", "authpass-sha", "AES128", "privpass-aes128"),
    ("v3_aes192_sha256", "SHA256", "authpass-sha256", "AES192", "privpass-aes192"),
    ("v3_aes256_sha512", "SHA512", "authpass-sha512", "AES256", "privpass-aes256"),
    # AES-192 with a short auth hash (MD5): needs the Reeder key-extension.
    ("v3_aes192_md5", "MD5", "authpass-md5ext", "AES192", "privpass-aes192ext"),
]


def find_recorded_dir():
    try:
        import snmpsim.confdir as confdir

        for candidate in confdir.data:
            recorded = os.path.join(candidate, "recorded")
            if os.path.isdir(recorded):
                return recorded
    except ImportError:
        pass
    print("snmpsim package not found. Install with: pip install snmpsim", file=sys.stderr)
    sys.exit(1)


def setup_data_dir(rec_file):
    if os.path.sep in rec_file or os.path.isfile(rec_file):
        src = os.path.abspath(rec_file)
    else:
        src = os.path.join(find_recorded_dir(), rec_file)
    if not os.path.isfile(src):
        print(f"Recorded file not found: {src}", file=sys.stderr)
        sys.exit(1)

    tmpdir = tempfile.mkdtemp(prefix="snmpsim-v3-test-")
    # self.snmprec serves the default v3 context; public.snmprec serves v2c.
    shutil.copy2(src, os.path.join(tmpdir, "self.snmprec"))
    shutil.copy2(src, os.path.join(tmpdir, "public.snmprec"))
    return tmpdir


def build_user_args():
    args = []
    for user, auth_proto, auth_pass, priv_proto, priv_pass in V3_USERS:
        args += ["--v3-user", user]
        if auth_proto:
            args += ["--v3-auth-key", auth_pass, "--v3-auth-proto", auth_proto]
        if priv_proto:
            args += ["--v3-priv-key", priv_pass, "--v3-priv-proto", priv_proto]
    return args


def main():
    parser = argparse.ArgumentParser(description="Start a v3-capable snmpsim agent.")
    parser.add_argument("rec_file", nargs="?", default=DEFAULT_REC_FILE)
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument("--host", default="0.0.0.0")
    args = parser.parse_args()

    tmpdir = setup_data_dir(args.rec_file)

    cmd = [
        "snmpsim-command-responder",
        "--log-level", "info",
        # --v3-engine-id must precede the per-engine options (data-dir, endpoint, users).
        "--v3-engine-id", "auto",
        "--data-dir", tmpdir,
        "--agent-udpv4-endpoint", f"{args.host}:{args.port}",
    ] + build_user_args()

    print(f"Starting v3 snmpsim agent on {args.host}:{args.port}")
    print(f"  Recording : {args.rec_file}")
    print(f"  Data dir  : {tmpdir}")
    print("  Users     :")
    for user, ap, _, pp, _ in V3_USERS:
        level = "noAuthNoPriv" if not ap else ("authPriv" if pp else "authNoPriv")
        print(f"    {user:<20} {level:<14} auth={ap or '-'} priv={pp or '-'}")
    print()

    try:
        proc = subprocess.run(cmd)
    except FileNotFoundError:
        print("snmpsim-command-responder not found in PATH.", file=sys.stderr)
        return 1
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)
    return proc.returncode


if __name__ == "__main__":
    main()
