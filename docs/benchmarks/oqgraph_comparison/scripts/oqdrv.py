#!/usr/bin/env python3
"""OQGraph evaluation driver (runs inside oqeval:24.04).

  oqdrv.py init   <datadir> <graphdir> <spec...>      initialise a datadir and load graphs (variant A server)
  oqdrv.py check  <datadir> <graphdir> <out.json> <variant...>   full-row results per query, hashed
  oqdrv.py bench  <datadir> <graphdir> <out.json> <rounds> <order-seed> <variant...>

One mariadbd process per (round, variant). Variants: A (stock libjudy), B (Judy.h drop-in),
C (native expanse.h port), B1 (A's plugin binary run against libexpanse via LD_LIBRARY_PATH).
"""
import glob, hashlib, json, os, random, shutil, subprocess, sys, time
import pymysql

INST = "/w/inst"
SOCK = "/tmp/oq.sock"
PLUGIN_DIR = {"A": "/w/plugins/A", "B": "/w/plugins/B", "C": "/w/plugins/C", "B1": "/w/plugins/A"}
ENV_EXTRA = {"B1": {"LD_LIBRARY_PATH": "/w/compat-lib"}}
SERVER_CPUS = os.environ.get("OQ_SERVER_CPUS")   # taskset list for mariadbd, if set

WORKLOADS = {
    # id: (where-clause template, needs dest)
    "oq_bfs_from":      ("latch='breadth_first' AND origid={o}", False),
    "oq_dijkstra_from": ("latch='dijkstras' AND origid={o}", False),
    "oq_leaves_from":   ("latch='leaves' AND origid={o}", False),
    "oq_bfs_path":      ("latch='breadth_first' AND origid={o} AND destid={d}", True),
    "oq_dijkstra_path": ("latch='dijkstras' AND origid={o} AND destid={d}", True),
    "oq_bfs_to":        ("latch='breadth_first' AND destid={d}", True),
}

def start_server(datadir, variant):
    env = dict(os.environ); env.update(ENV_EXTRA.get(variant, {}))
    cmd = [f"{INST}/bin/mariadbd", "--no-defaults", "--user=root", f"--datadir={datadir}",
           f"--socket={SOCK}", "--skip-networking", f"--plugin-dir={PLUGIN_DIR[variant]}",
           "--plugin-load-add=ha_oqgraph.so", "--innodb-buffer-pool-size=4G", "--innodb-buffer-pool-size-max=4G",
           "--innodb-log-file-size=512M", "--skip-log-bin", "--performance-schema=OFF",
           "--log-error=" + datadir + "/err.log", "--innodb-fast-shutdown=1",
           "--max-allowed-packet=256M", "--local-infile=1"]
    if os.environ.get("OQ_CALLGRIND"):
        cmd = ["valgrind", "--tool=callgrind", "--instr-atstart=no", "--dump-instr=no",
               f"--callgrind-out-file={os.environ['OQ_CALLGRIND']}/{variant}.%p"] + cmd
    if SERVER_CPUS:
        cmd = ["taskset", "-c", SERVER_CPUS] + cmd
    p = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(6000):
        try:
            c = pymysql.connect(unix_socket=SOCK, user="root", local_infile=True, autocommit=True)
            return p, c
        except pymysql.err.OperationalError:
            if p.poll() is not None:
                raise SystemExit(f"mariadbd ({variant}) exited rc={p.returncode}; see {datadir}/err.log")
            time.sleep(0.1)
    raise SystemExit("mariadbd did not come up")

def server_pid(p):
    # taskset execs mariadbd in place, so the pid is the server's.
    return p.pid

EXPECT = {"A": "/libJudy.so.1.0.3", "B": "/libexpanse.so", "C": "/libexpanse.so", "B1": "/libexpanse.so"}

def assert_libs(variant, libs):
    """Fail loud if the server mapped the wrong bitset library for this variant."""
    want = EXPECT[variant]
    hits = [l for l in libs if "Judy" in l or "expanse" in l]
    if len(hits) != 1 or not hits[0].endswith(want):
        raise SystemExit(f"variant {variant}: expected one library ending {want}, mapped {hits}")

def judy_libs(pid):
    libs = set()
    for line in open(f"/proc/{pid}/maps"):
        path = line.split()[-1] if len(line.split()) >= 6 else ""
        if "Judy" in path or "expanse" in path or "ha_oqgraph" in path:
            libs.add(os.path.realpath(path))
    return sorted(libs)

def mem(pid):
    out = {}
    for line in open(f"/proc/{pid}/status"):
        k = line.split(":")[0]
        if k in ("VmRSS", "VmHWM"):
            out[k] = int(line.split()[1])
    return out

def stop_server(p, c):
    try:
        c.cursor().execute("SHUTDOWN")
    except Exception:
        pass
    try:
        p.wait(timeout=120)
    except subprocess.TimeoutExpired:
        p.kill(); p.wait()

def graphs(graphdir):
    return sorted(f[:-len(".queries.json")] for f in os.listdir(graphdir) if f.endswith(".queries.json"))

# Dijkstra on scattered 63-bit ids aborts mariadbd in every variant, stock included:
# boost's index-in-heap vector_property_map resizes to the vertex id and throws
# std::length_error (uncaught on the origid-only branch). Probed separately by `probe`.
EXCLUDE = {("er_sid", "oq_dijkstra_from"), ("er_sid", "oq_dijkstra_path")}

def queries(graphdir, name, workloads=None):
    meta = json.load(open(f"{graphdir}/{name}.queries.json"))
    for wid, (tmpl, _) in WORKLOADS.items():
        if workloads and wid not in workloads:
            continue
        if (meta["shape"], wid) in EXCLUDE:
            continue
        for i, (o, d) in enumerate(zip(meta["origins"], meta["dests"])):
            yield wid, i, f"g_{name} WHERE " + tmpl.format(o=o, d=d)

def cmd_init(datadir, graphdir, *_):
    if os.path.exists(datadir):
        shutil.rmtree(datadir)
    subprocess.run([f"{INST}/scripts/mariadb-install-db", "--no-defaults", f"--basedir={INST}",
                    f"--datadir={datadir}", "--user=root", "--auth-root-authentication-method=normal"],
                   check=True, stdout=subprocess.DEVNULL)
    p, c = start_server(datadir, "A")
    cur = c.cursor()
    cur.execute("CREATE DATABASE oq"); cur.execute("USE oq")
    for name in graphs(graphdir):
        cur.execute(f"CREATE TABLE e_{name} (origid BIGINT UNSIGNED NOT NULL, destid BIGINT UNSIGNED NOT NULL,"
                    f" weight DOUBLE NOT NULL, PRIMARY KEY (origid, destid), KEY (destid)) ENGINE=InnoDB")
        cur.execute(f"LOAD DATA LOCAL INFILE '{graphdir}/{name}.tsv' INTO TABLE e_{name}")
        cur.execute(f"CREATE TABLE g_{name} (latch VARCHAR(32) NULL, origid BIGINT UNSIGNED NULL,"
                    f" destid BIGINT UNSIGNED NULL, weight DOUBLE NULL, seq BIGINT UNSIGNED NULL,"
                    f" linkid BIGINT UNSIGNED NULL, KEY (latch, origid, destid) USING HASH,"
                    f" KEY (latch, destid, origid) USING HASH) ENGINE=OQGRAPH"
                    f" data_table='e_{name}' origid='origid' destid='destid' weight='weight'")
        cur.execute(f"SELECT COUNT(*) FROM e_{name}"); print(name, "edges", cur.fetchone()[0], flush=True)
    # OQGraph reports the backing table's row estimate to the optimizer; a stale
    # InnoDB estimate of <= 1 row makes every query "Impossible WHERE" (empty,
    # engine never called). Refresh persistent stats and check them.
    for name in graphs(graphdir):
        cur.execute(f"ANALYZE TABLE e_{name}"); cur.fetchall()
    cur.execute("SELECT TABLE_NAME, TABLE_ROWS FROM information_schema.TABLES"
                " WHERE TABLE_SCHEMA='oq' AND TABLE_NAME LIKE 'e\\_%'")
    est = dict(cur.fetchall()); print("row estimates", est, flush=True)
    stop_server(p, c)
    bad = [t for t, r in est.items() if not r or r < 100]
    if bad:
        raise SystemExit(f"stale row estimates after ANALYZE: {bad}")

def cmd_check(datadir, graphdir, out, *variants):
    res = {}
    for v in variants:
        p, c = start_server(datadir, v)
        cur = c.cursor(); cur.execute("USE oq")
        r = {"libs": judy_libs(server_pid(p)), "q": {}}
        assert_libs(v, r["libs"])
        for name in graphs(graphdir):
            for wid, i, frm in queries(graphdir, name):
                cur.execute(f"SELECT latch, origid, destid, weight, seq, linkid FROM {frm}")
                rows = cur.fetchall()
                h = hashlib.sha256(repr(rows).encode()).hexdigest()
                r["q"][f"{name}/{wid}/{i}"] = [len(rows), h]
            cur.execute(f"SELECT COUNT(*) FROM e_{name}")
            r["q"][f"{name}/edges"] = [cur.fetchone()[0], ""]
        if os.environ.get("OQ_VERTICES"):      # latch='' vertex listing: O(V^2), small graphs only
            for name in os.environ["OQ_VERTICES"].split(","):
                cur.execute(f"SELECT latch, origid, destid, weight, seq, linkid FROM g_{name} WHERE latch=''")
                rows = cur.fetchall()
                r["q"][f"{name}/vertices"] = [len(rows), hashlib.sha256(repr(rows).encode()).hexdigest()]
        r["mem"] = mem(server_pid(p))
        stop_server(p, c)
        res[v] = r
        print(v, r["libs"], len(r["q"]), "queries", flush=True)
    json.dump(res, open(out, "w"), indent=1)
    base = res[variants[0]]["q"]
    bad = 0
    for v in variants[1:]:
        for k, val in base.items():
            if res[v]["q"].get(k) != val:
                bad += 1
                print("MISMATCH", v, k, val, res[v]["q"].get(k))
    print("mismatches:", bad)
    return bad

def load_snapshot():
    with open("/proc/loadavg") as f:
        la = f.read().split()[:3]
    with open("/proc/stat") as f:
        cpu = [int(x) for x in f.readline().split()[1:]]
    return {"t": time.time(), "loadavg": la, "busy_jiffies": sum(cpu) - cpu[3] - cpu[4], "total_jiffies": sum(cpu)}

def cmd_bench(datadir, graphdir, out, rounds, order_seed, *variants):
    cells = os.environ.get("OQ_CELLS")          # "graph:workload,..." filter
    cells = set(cells.split(",")) if cells else None
    k_timed = int(os.environ.get("OQ_K", "16"))
    warm = int(os.environ.get("OQ_WARM", "1"))
    rng = random.Random(int(order_seed))
    data = {"meta": {"variants": variants, "rounds": int(rounds), "order_seed": order_seed,
                     "k_timed": k_timed, "warm_passes": warm, "server_cpus": SERVER_CPUS,
                     "pin": os.environ.get("EXPANSE_BENCH_PIN_APPLIED"),
                     "k_by_n": {k: v for k, v in os.environ.items() if k.startswith("OQ_K_")},
                     "client_affinity": sorted(os.sched_getaffinity(0)),
                     "cells_filter": sorted(cells) if cells else None}, "rounds": []}
    for r in range(int(rounds)):
        order = list(variants); rng.shuffle(order)
        for v in order:
            snap0 = load_snapshot()
            p, c = start_server(datadir, v)
            pid = server_pid(p)
            assert_libs(v, judy_libs(pid))
            cur = c.cursor(); cur.execute("USE oq")
            plan = []
            for name in graphs(graphdir):
                for wid, i, frm in queries(graphdir, name):
                    if cells and f"{name}:{wid}" not in cells:
                        continue
                    if i < int(os.environ.get(f"OQ_K_{name.rsplit('_', 1)[1]}", k_timed)):
                        plan.append((name, wid, i, f"SELECT COUNT(*) FROM {frm}"))
            # Warm-up (untimed): every query once; brings the backing tables into the buffer pool.
            for _ in range(warm):
                for _, _, _, q in plan:
                    cur.execute(q); cur.fetchall()
            m_warm = mem(pid)
            snap1 = load_snapshot()
            times = {}
            for name, wid, i, q in plan:
                t0 = time.perf_counter_ns(); cur.execute(q); n = cur.fetchone()[0]; t1 = time.perf_counter_ns()
                times.setdefault(f"{name}:{wid}", []).append([i, t1 - t0, n])
            snap2 = load_snapshot()
            m_end = mem(pid)
            libs = judy_libs(pid)
            stop_server(p, c)
            data["rounds"].append({"round": r, "variant": v, "libs": libs, "mem_warm": m_warm,
                                   "mem_end": m_end, "snap": [snap0, snap1, snap2], "times": times})
            print(f"round {r} {v} done", flush=True)
            json.dump(data, open(out, "w"))
    json.dump(data, open(out, "w"))

def cmd_probe(datadir, graphdir, out, query, *variants):
    """Run one query per variant on a fresh server; record rows hash or server death."""
    res = {}
    for v in variants:
        p, c = start_server(datadir, v)
        assert_libs(v, judy_libs(server_pid(p)))
        cur = c.cursor(); cur.execute("USE oq")
        try:
            cur.execute(query); rows = cur.fetchall()
            res[v] = {"rows": len(rows), "sha": hashlib.sha256(repr(rows).encode()).hexdigest()}
        except pymysql.err.OperationalError as e:
            p.wait(timeout=60)
            log = open(datadir + "/err.log").read().splitlines()
            res[v] = {"error": str(e), "server_rc": p.returncode,
                      "abort_frames": [l for l in log[-80:] if "throw_length_error" in l or "oqgraph" in l][:3]}
            print(v, res[v], flush=True)
            continue
        stop_server(p, c)
        print(v, res[v], flush=True)
    json.dump(res, open(out, "w"), indent=1)

def cmd_profile(datadir, graphdir, k, *variants):
    """Callgrind: one dump per cell, instrumentation on only around the cell's queries."""
    cells = os.environ.get("OQ_CELLS"); cells = set(cells.split(",")) if cells else None
    for v in variants:
        p, c = start_server(datadir, v)
        pid = server_pid(p); assert_libs(v, judy_libs(pid))
        cur = c.cursor(); cur.execute("USE oq")
        plan = {}
        for name in graphs(graphdir):
            for wid, i, frm in queries(graphdir, name):
                if (cells and f"{name}:{wid}" not in cells) or i >= int(k):
                    continue
                plan.setdefault(f"{name}:{wid}", []).append(f"SELECT COUNT(*) FROM {frm}")
        for qs in plan.values():                    # warm-up, uninstrumented
            for q in qs:
                cur.execute(q); cur.fetchall()
        cg = lambda *a: subprocess.run(["callgrind_control", *a, str(pid)], check=True, stdout=subprocess.DEVNULL)
        for cell, qs in plan.items():
            cg("-z"); cg("-i", "on")
            for q in qs:
                cur.execute(q); cur.fetchall()
            cg("-i", "off"); cg("-d", cell)
            dumps = sorted(glob.glob(f"{os.environ['OQ_CALLGRIND']}/{v}.{pid}.*"), key=os.path.getmtime)
            summ = [l for l in open(dumps[-1]) if l.startswith("totals:")]
            if not summ or int(summ[0].split()[1]) == 0:      # fail loud on an empty dump
                raise SystemExit(f"{v} {cell}: empty Callgrind dump {dumps[-1]}")
            print(v, cell, "dumped", summ[0].strip(), flush=True)
        stop_server(p, c)

if __name__ == "__main__":
    {"init": cmd_init, "check": cmd_check, "bench": cmd_bench, "probe": cmd_probe, "profile": cmd_profile}[sys.argv[1]](*sys.argv[2:])
