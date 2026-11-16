#!/usr/bin/env python3
"""One zmap process per call, inside the scan netns. Prints one JSON line.

Records: wall/user/sys/maxrss (GNU time on the zmap pid itself, since
`ip netns exec` execs in place), per-thread CPU (schedstat ns, last sample
before each thread exits), host load before, host busy-CPU delta across the
run, zmap's metadata JSON, and the sha256 of the sorted output rows.
"""
import hashlib, json, os, subprocess, sys, time

def busy_jiffies():
    with open("/proc/stat") as f:
        v = [int(x) for x in f.readline().split()[1:]]
    idle = v[3] + v[4]
    return sum(v[:8]) - idle, sum(v[:8])

def main():
    arm, variant, dedup, probes, rate, seed, window, outdir = sys.argv[1:9]
    tag = f"{arm}-{os.getpid()}"
    out = os.path.join(outdir, f"{tag}.csv"); meta = os.path.join(outdir, f"{tag}.meta.json")
    tfile = os.path.join(outdir, f"{tag}.time")
    gw = open("/tmp/gwmac").read().strip()
    zmap = [f"/build/{variant.split('+')[0]}/src/zmap"]
    env = dict(os.environ)
    if variant == "stock+swap":   # stock-built binary, libJudy.so.1 -> libexpanse.so
        env["LD_LIBRARY_PATH"] = "/opt/expanse/compat"
    cmd = ["/usr/bin/time", "-f", "%e %U %S %M", "-o", tfile,
           "ip", "netns", "exec", "scan"] + zmap + [
        "-i", "veth0", "-S", "192.0.2.1", "-G", gw, "-p", "80,443,8080,8443",
        "-r", rate, "-P", probes, "-e", seed, "-c", "2", "-T", "1",
        "-b", "/scripts/empty_blocklist.conf", "--disable-syslog", "-q", "-v", "1",
        "-o", out, "-f", "saddr,sport,classification,success,repeat",
        "--output-filter=success = 0 || success = 1", "-m", meta,
        "--dedup-method", dedup] + (["--dedup-window-size", window] if dedup == "window" else []) + [
        "198.18.0.0/15"]
    with open("/proc/loadavg") as f:
        load_before = f.read().split()[:3]
    b0, t0 = busy_jiffies()
    p = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    zpid = None
    threads = {}
    while p.poll() is None:
        if zpid is None:
            try:
                with open(f"/proc/{p.pid}/task/{p.pid}/children") as f:
                    c = f.read().split()
                zpid = int(c[0]) if c else None
            except OSError:
                pass
        if zpid is not None:
            try:
                for tid in os.listdir(f"/proc/{zpid}/task"):
                    try:
                        with open(f"/proc/{zpid}/task/{tid}/schedstat") as f:
                            threads[int(tid)] = int(f.read().split()[0])
                    except OSError:
                        pass
            except OSError:
                pass
        time.sleep(0.02)
    b1, t1 = busy_jiffies()
    err = p.stderr.read().decode(errors="replace")
    ncpu = os.cpu_count()
    wall, user, sys_, maxrss = open(tfile).read().split()[-4:]
    rec = {"arm": arm, "variant": variant, "dedup": dedup, "probes": int(probes),
           "rate": int(rate), "seed": int(seed), "window": int(window),
           "rc": p.returncode, "wall_s": float(wall), "user_s": float(user),
           "sys_s": float(sys_), "maxrss_kb": int(maxrss),
           "zpid": zpid, "thread_cpu_ns": {str(k): threads[k] for k in sorted(threads)},
           "load_before": load_before,
           "host_busy_core_eq": (b1 - b0) / max(1, (t1 - t0)) * ncpu}
    try:
        m = json.load(open(meta))
        for k in ("packets_sent", "pcap_recv", "pcap_drop", "pcap_ifdrop", "validation_passed",
                  "validation_failed", "success_total", "success_unique", "failure_total",
                  "targets_scanned", "send_to_failures"):
            rec[k] = m.get(k)
    except Exception as e:
        rec["meta_error"] = str(e)
    try:
        rows = open(out).read().splitlines()
        hdr, body = rows[0], sorted(rows[1:])
        rec["rows"] = len(body)
        rec["out_sha256"] = hashlib.sha256(("\n".join([hdr] + body)).encode()).hexdigest()
        if os.environ.get("KEEP_OUTPUT") != "1":
            os.remove(out)
    except Exception as e:
        rec["out_error"] = str(e)
    if p.returncode != 0:
        rec["stderr_tail"] = err[-2000:]
    print(json.dumps(rec), flush=True)

main()
