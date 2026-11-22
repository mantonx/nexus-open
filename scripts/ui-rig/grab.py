#!/usr/bin/env python3
"""grab.py OUT.png [settle_seconds] — save the review daemon's current strip frame (640x48) plus a 2x copy."""
import os, socket, struct, sys, time
from PIL import Image
out = sys.argv[1]; settle = float(sys.argv[2]) if len(sys.argv) > 2 else 0.6
s = socket.socket(socket.AF_UNIX); s.connect(os.environ["XDG_RUNTIME_DIR"] + "/nexus-open.sock")
f = s.makefile("rwb", buffering=0); f.write(b"frames\n"); f.readline()
s.settimeout(settle); frame = None; end = time.time() + settle
def read(n):
    b = b""
    while len(b) < n:
        c = s.recv(n - len(b))
        if not c: raise EOFError
        b += c
    return b
try:
    while time.time() < end:
        n = struct.unpack("<I", read(4))[0]; frame = read(n)
except (socket.timeout, EOFError): pass
im = Image.frombytes("RGBA", (640, 48), frame).convert("RGB"); im.save(out)
