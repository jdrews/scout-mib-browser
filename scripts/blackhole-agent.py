#!/usr/bin/env python3
"""UDP black hole: binds a port and never answers (launch-beta/28).

The timeout e2e specs point the app at this listener: the UDP "connect"
always succeeds, no datagram ever comes back, so only a per-request
deadline can end the operation.

Usage: blackhole-agent.py <port>
"""
import socket
import sys
import time

port = int(sys.argv[1])
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
sock.bind(("127.0.0.1", port))
print(f"black hole listening on 127.0.0.1:{port}", flush=True)
while True:
    time.sleep(3600)
