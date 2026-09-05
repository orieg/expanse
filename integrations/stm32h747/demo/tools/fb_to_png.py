#!/usr/bin/env python3
"""Convert a raw ARGB8888 800x480 framebuffer dump (little-endian words, so
bytes B,G,R,A) to PNG. Used by grab_frame.sh."""
import sys
from PIL import Image

W, H = 800, 480
raw = open(sys.argv[1], "rb").read()
assert len(raw) >= W * H * 4, f"short dump: {len(raw)} bytes"
img = Image.frombuffer("RGBA", (W, H), raw[: W * H * 4], "raw", "BGRA", 0, 1).convert("RGB")
img.save(sys.argv[2])
print(f"wrote {sys.argv[2]} ({W}x{H})")
