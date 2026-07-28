#!/usr/bin/env python3
"""Start snmpsim-command-responder with a recorded .snmprec file for testing."""

import argparse
import os
import shutil
import signal
import subprocess
import sys
import tempfile

DEFAULT_REC_FILE = "linux-full-walk.snmprec"
DEFAULT_COMMUNITY = "public"
DEFAULT_PORT = 11611


def find_snmppsim_data_dir():
    """Locate snmpsim's data directory from pip installation."""
    try:
        import snmpsim.confdir as confdir

        for candidate in confdir.data:
            recorded = os.path.join(candidate, "recorded")
            if os.path.isdir(recorded):
                return recorded
    except ImportError:
        pass

    print(
        "snmpsim package not found. Install with:\n"
        "  pip install snmpsim",
        file=sys.stderr,
    )
    sys.exit(1)


SNMPSIM_RECORDED_DIR = find_snmppsim_data_dir()


def find_rec_files():
    """List available .snmprec files in the recorded directory."""
    return sorted(
        f for f in os.listdir(SNMPSIM_RECORDED_DIR) if f.endswith(".snmprec")
    )


def setup_data_dir(rec_file):
    """Copy rec_file into a temp dir as public.snmprec and return (tmpdir, community)."""
    src = os.path.join(SNMPSIM_RECORDED_DIR, rec_file)
    if not os.path.isfile(src):
        print(f"Recorded file not found: {src}", file=sys.stderr)
        sys.exit(1)

    tmpdir = tempfile.mkdtemp(prefix="snmpsim-test-")
    shutil.copy2(src, os.path.join(tmpdir, f"{DEFAULT_COMMUNITY}.snmprec"))
    return tmpdir


def main():
    parser = argparse.ArgumentParser(
        description="Start snmpsim for testing the MIB browser.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
Examples:
  %(prog)s                          # default: linux-full-walk.snmprec on port 11611
  %(prog)s --port 17000             # custom port
  %(prog)s --list                   # list available .snmprec files
  %(prog)s winxp-full-walk.snmprec  # use a specific recording

The community name is always "public" (v2c).
""",
    )
    parser.add_argument(
        "rec_file",
        nargs="?",
        default=DEFAULT_REC_FILE,
        help=f"Recording file name (default: {DEFAULT_REC_FILE})",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=DEFAULT_PORT,
        help=f"UDP port for the SNMP agent (default: {DEFAULT_PORT})",
    )
    parser.add_argument(
        "--community",
        default=DEFAULT_COMMUNITY,
        help=f"SNMPv2c community name (default: {DEFAULT_COMMUNITY})",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        dest="list_files",
        help="List available .snmprec files and exit",
    )
    parser.add_argument(
        "--host",
        default="0.0.0.0",
        help="Bind address (default: 0.0.0.0)",
    )

    args = parser.parse_args()

    if args.list_files:
        files = find_rec_files()
        if not files:
            print("No .snmprec files found.")
        else:
            print(f"Available recordings in {SNMPSIM_RECORDED_DIR}:")
            default_marker = "  <-- default" if DEFAULT_REC_FILE in files else ""
            for f in files:
                marker = default_marker if f == DEFAULT_REC_FILE else ""
                print(f"  {f}{marker}")
        return 0

    tmpdir = setup_data_dir(args.rec_file)
    data_file = os.path.join(tmpdir, f"{args.community}.snmprec")

    cmd = [
        "snmpsim-command-responder",
        "--data-dir", tmpdir,
        "--agent-udpv4-endpoint", f"{args.host}:{args.port}",
        "--log-level", "info",
    ]

    print(f"Starting snmpsim:")
    print(f"  Recording : {args.rec_file}")
    print(f"  Data file : {data_file}")
    print(f"  Address   : {args.host}:{args.port}")
    print(f"  Community : {args.community} (v2c)")
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
