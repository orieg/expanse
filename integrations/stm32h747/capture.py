import os, sys, termios, time, select
port, out, timeout = sys.argv[1], sys.argv[2], float(sys.argv[3])
fd = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
attr = termios.tcgetattr(fd)
attr[0] = 0; attr[1] = 0; attr[2] = termios.CS8 | termios.CREAD | termios.CLOCAL; attr[3] = 0
attr[4] = attr[5] = termios.B115200
termios.tcsetattr(fd, termios.TCSANOW, attr)
termios.tcflush(fd, termios.TCIFLUSH)
open(out, "wb").close()
print("READY", flush=True)
buf = b""; t0 = time.time(); marks = []; marks_all = marks
with open(out, "wb") as f:
    while time.time() - t0 < timeout:
        r, _, _ = select.select([fd], [], [], 0.5)
        if r:
            try: chunk = os.read(fd, 4096)
            except BlockingIOError: chunk = b""
            if chunk:
                f.write(chunk); f.flush(); buf += chunk
                for m in (b"TICK", b"TOCK"):
                    n = buf.count(m)
                    while sum(1 for x in marks if x[0] == m) < n: marks.append((m, time.time()))
        if b"DONE" in buf or b"FAULT" in buf: break
print("bytes", len(buf))
# The ST-LINK VCP can deliver bytes buffered from the previous run ahead of
# the reset; keep only what follows the last firmware banner, and recompute
# the calibration marks over that suffix so a stale TOCK cannot pair up.
BANNER = b"EXPANSE stm32h747 m7 harness"
cut = buf.rfind(BANNER)
if cut > 0:
    dropped = buf[:cut]
    buf = buf[cut:]
    with open(out, "wb") as f:
        f.write(buf)
    print("dropped %d stale bytes before the banner" % len(dropped))
    # host timestamps were recorded in arrival order; drop the marks that
    # belong to the stale prefix so TICK/TOCK pair up within this run only
    stale_ticks = dropped.count(b"TICK"); stale_tocks = dropped.count(b"TOCK")
    ticks_all = [x[1] for x in marks_all if x[0] == b"TICK"][stale_ticks:]
    tocks_all = [x[1] for x in marks_all if x[0] == b"TOCK"][stale_tocks:]
    marks = [(b"TICK", t) for t in ticks_all] + [(b"TOCK", t) for t in tocks_all]
ticks = [x[1] for x in marks if x[0] == b"TICK"]; tocks = [x[1] for x in marks if x[0] == b"TOCK"]
with open(out, "ab") as f:
    if buf and not buf.endswith(b"\n"): f.write(b"\n")
    for a, b in zip(ticks, tocks):
        line = "CALIB host_seconds=%.3f cycles=320000000\n" % (b - a)
        print(line, end=""); f.write(line.encode())
