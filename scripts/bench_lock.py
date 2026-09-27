#!/usr/bin/env python3
"""The host-wide benchmark lock: one suite at a time per machine (#1210).

    bench_lock.py [--suite NAME] [--wait SECONDS] -- <command> [args...]

Takes the lock, runs the command, releases the lock when the command exits,
and exits with the command's status. A runner re-executes itself under it:

    if [ -z "${EXPANSE_BENCH_LOCK_HELD:-}" ]; then
      exec python3 "$REPO_ROOT/scripts/bench_lock.py" --suite <name> -- bash "$0" "$@"
    fi

## Why a wrapper, and why this lock

The lock used to be a directory made with `mkdir`, removed by a shell trap. A
trap does not run when the process is killed outright, and cancelling a
workflow run ends in SIGKILL: run 36295887129 left the directory behind, and
every later dispatch refused to start (exit 75) against a holder whose PID no
longer existed.

This lock is `flock(2)` on a file. The kernel releases it when the last file
descriptor holding it closes, which a SIGKILL does. Only this process holds
that descriptor: it is opened close-on-exec and the command runs with every
other descriptor closed, so no benchmark, daemon or `setsid` grandchild
inherits it and keeps the host locked after the run is gone. For the same
reason the command is sent SIGTERM when this process dies (Linux
`PR_SET_PDEATHSIG`), so a benchmark does not run on with the lock released.

## Paths

- `$EXPANSE_BENCH_FLOCK`: the lock. Unset, it is the host's shared lock
  `/run/expanse-bench/expanse-bench.flock` when that file exists, and
  `/tmp/expanse-bench.flock` otherwise, the path sessions outside the
  repository take by hand. It must be a regular file, and either owned by the
  caller or the shared lock root provisions for several accounts: owned by
  root, not writable by others, in a group the caller belongs to
  (`docs/CI.md`, "The benchmark lock"; `scripts/bench_host/provision.sh`).
  A symlink, or a file another non-root user created, is refused rather than
  opened: that user could replace it and hold a different inode. The file is
  never truncated, only locked.
- `<lock>.owner`: who holds it, written after the lock is taken. Advisory: a
  holder killed outright leaves it behind, so it names the last holder, not
  necessarily a live one.
- `${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}`: the old `mkdir`
  lock, kept as a **mirror** while this lock is held, so a checkout that
  predates this file still sees the host as busy. A leftover mirror whose
  owner PID is not running is reclaimed — safe, because this process holds the
  flock, so no current holder of the new lock can own it. A running owner is
  an old-style run holding the host, and this waits as it would for the flock.
  Remove the mirror once no checkout on a benchmark host predates this file.

Exit status: the command's; 75 when the lock stayed held for `--wait` seconds;
78 when the lock file cannot be used (symlink, wrong owner, not a file).
"""

from __future__ import annotations

import argparse
import errno
import fcntl
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

EX_HELD = 75
EX_CONFIG = 78
HELD_ENV = "EXPANSE_BENCH_LOCK_HELD"
# A fresh mirror directory whose owner file is not yet written is an old-style
# runner between its `mkdir` and its `printf`; one older than this is a leftover.
MIRROR_GRACE_S = 5.0
POLL_S = 0.5
_UNSAFE = re.compile(r"[^A-Za-z0-9_.:=/ +\[\]-]")


class LockConfigError(Exception):
    """The lock file exists in a form this process must not use."""


def sanitize(text: str, limit: int = 300) -> str:
    """A holder line fit for a workflow output or a markdown code fence.

    The owner record is written by whoever holds the lock, so its text reaches
    a public comment only through this: one line, a closed character set (no
    backtick, so it cannot end a fence), bounded length.
    """
    stripped = text.strip()
    line = stripped.splitlines()[0] if stripped else ""
    return _UNSAFE.sub("?", line)[:limit] or "unknown"


# The lock a host shares between its runner's service account and human
# sessions, created by root through tmpfiles.d (scripts/bench_host/provision.sh).
SHARED_LOCK = Path("/run/expanse-bench/expanse-bench.flock")


def lock_path() -> Path:
    env = os.environ.get("EXPANSE_BENCH_FLOCK")
    if env:
        return Path(env)
    return SHARED_LOCK if SHARED_LOCK.is_file() else Path("/tmp/expanse-bench.flock")


def owner_problem(st_uid: int, st_gid: int, st_mode: int, uid: int, groups: set[int]) -> str | None:
    """Why a lock file with this ownership must not be used, or None.

    The caller's own file is usable. So is the shared lock root provisions:
    a non-root account cannot create a root-owned file, so root ownership
    proves who made it, and with no write bit for others and the caller in its
    group, the group's members are the only accounts that can open it.
    """
    if st_uid == uid:
        return None
    if st_uid == 0:
        if st_mode & stat.S_IWOTH:
            return "is root-owned but writable by every user"
        if st_gid not in groups:
            return f"is the shared lock of group {st_gid}, which this user ({uid}) is not in"
        return None
    return f"belongs to uid {st_uid}, not this user ({uid}); another user created it, so it is not used"


def mirror_path() -> Path:
    legacy = os.environ.get("EXPANSE_BENCH_LOCK")
    return Path(legacy) if legacy else Path(os.environ.get("TMPDIR") or "/tmp", "expanse-bench.lock")


def open_lock(path: Path) -> int:
    """A descriptor on the lock file, refusing anything but our own regular file."""
    flags = os.O_RDWR | os.O_CREAT | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags, 0o664)
    except OSError as exc:
        if exc.errno == errno.ELOOP:
            raise LockConfigError(f"{path} is a symlink; the lock is never taken through a link") from exc
        hint = ""
        if exc.errno == errno.EACCES:
            hint = (
                "; another account created it. A host shared between accounts provides "
                f"{SHARED_LOCK} (docs/CI.md, 'The benchmark lock')"
            )
        raise LockConfigError(f"{path} cannot be opened: {exc.strerror}{hint}") from exc
    st = os.fstat(fd)
    if not stat.S_ISREG(st.st_mode):
        os.close(fd)
        raise LockConfigError(f"{path} is not a regular file")
    problem = owner_problem(st.st_uid, st.st_gid, st.st_mode, os.getuid(), set(os.getgroups()) | {os.getgid()})
    if problem:
        os.close(fd)
        raise LockConfigError(f"{path} {problem}")
    return fd


def pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True  # a live process of another user
    return True


def mirror_holder(mirror: Path) -> str | None:
    """Who holds the legacy directory, or None when it is free or reclaimable."""
    try:
        st = os.lstat(mirror)
    except FileNotFoundError:  # discipline:allow(error-swallowing): no directory means no old-style holder
        return None
    if not stat.S_ISDIR(st.st_mode):
        return f"{mirror} exists and is not a directory"
    try:
        owner = (mirror / "owner").read_text(encoding="utf-8", errors="replace")
    except OSError:
        if time.time() - st.st_mtime < MIRROR_GRACE_S:
            return f"{mirror} (being created)"
        return None
    m = re.search(r"\bpid=(\d+)", owner)
    if m and pid_alive(int(m.group(1))):
        return sanitize(owner)
    return None


def reclaim_mirror(mirror: Path) -> None:
    if mirror.is_dir() and not mirror.is_symlink():
        print(
            f"::notice::reclaiming the leftover benchmark lock directory {mirror} (its owner is not running)",
            file=sys.stderr,
        )
        shutil.rmtree(mirror, ignore_errors=True)


def owner_line(suite: str) -> str:
    start = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    run = os.environ.get("GITHUB_RUN_ID")
    line = f"suite={suite} pid={os.getpid()} start={start}" + (f" run={run}" if run else "")
    return sanitize(line, 400) + "\n"


def write_owner(path: Path, line: str) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags, 0o664)
    except OSError as exc:
        print(f"::warning::could not record the lock holder in {path}: {exc.strerror}", file=sys.stderr)
        return
    with os.fdopen(fd, "w", encoding="utf-8") as fh:
        fh.write(line)


def acquire(suite: str, wait_s: float) -> tuple[int, Path] | str:
    """`(fd, mirror)` once both locks are ours, or the holder's description."""
    path = lock_path()
    mirror = mirror_path()
    fd = open_lock(path)
    deadline = time.monotonic() + wait_s
    while True:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            try:
                rec = sanitize(Path(f"{path}.owner").read_text(encoding="utf-8", errors="replace"))
            except OSError:
                rec = "unknown (no owner record)"
            holder = f"{rec} [flock {path}, the owner record is advisory]"
        else:
            legacy = mirror_holder(mirror)
            if legacy is None:
                reclaim_mirror(mirror)
                line = owner_line(suite)
                try:
                    os.mkdir(mirror)
                except FileExistsError:
                    legacy = f"{mirror} (created concurrently)"
                else:
                    write_owner(mirror / "owner", line)
                    write_owner(Path(f"{path}.owner"), line)
                    return fd, mirror
            fcntl.flock(fd, fcntl.LOCK_UN)
            holder = f"{legacy} [legacy lock directory {mirror}]"
        if time.monotonic() >= deadline:
            os.close(fd)
            return holder
        time.sleep(POLL_S)


def release_mirror(mirror: Path) -> None:
    """Remove the mirror only while it still names this process."""
    try:
        owner = (mirror / "owner").read_text(encoding="utf-8", errors="replace")
    except OSError:  # discipline:allow(error-swallowing): no owner record means the mirror is not this process's to remove
        return
    if re.search(rf"\bpid={os.getpid()}\b", owner):
        shutil.rmtree(mirror, ignore_errors=True)


def _die_with_parent() -> None:
    """Child side: be sent SIGTERM when this wrapper dies (Linux only)."""
    try:
        import ctypes

        libc = ctypes.CDLL(None, use_errno=True)
        pr_set_pdeathsig = 1
        if libc.prctl(pr_set_pdeathsig, signal.SIGTERM) != 0:
            raise OSError(ctypes.get_errno(), "prctl(PR_SET_PDEATHSIG) failed")
    except (OSError, AttributeError) as exc:
        # The child's stderr is the run's log: say that the command could
        # outlive its lock if this wrapper is killed.
        os.write(2, f"::warning::bench_lock: the command will not be stopped if the lock holder dies ({exc})\n".encode())


def _gh_output(**kv: str) -> None:
    out = os.environ.get("GITHUB_OUTPUT")
    if not out:
        return
    with open(out, "a", encoding="utf-8") as fh:
        for k, v in kv.items():
            fh.write(f"{k}={sanitize(v)}\n")


def run_locked(suite: str, wait_s: float, cmd: list[str], github_output: bool) -> int:
    if os.environ.get(HELD_ENV):
        # An enclosing runner already holds the host (a workflow step that
        # runs a suite's run.sh, say). Taking it again would wait on itself.
        return subprocess.call(cmd, close_fds=True)
    try:
        got = acquire(suite, wait_s)
    except LockConfigError as exc:
        print(f"::error::refusing to start: the benchmark lock is unusable: {exc}", file=sys.stderr)
        if github_output:
            _gh_output(fail_reason="lock_unusable", lock_owner=str(exc))
        return EX_CONFIG
    if isinstance(got, str):
        print(f"::error::refusing to start: the benchmark lock stayed held for {wait_s:g}s by: {got}", file=sys.stderr)
        if github_output:
            _gh_output(fail_reason="lock_held", lock_owner=got)
        return EX_HELD
    fd, mirror = got
    env = dict(os.environ, **{HELD_ENV: str(lock_path())})
    preexec = _die_with_parent if sys.platform.startswith("linux") else None
    try:
        child = subprocess.Popen(cmd, close_fds=True, env=env, preexec_fn=preexec)
    except OSError as exc:
        print(f"::error::could not start {cmd[0]!r}: {exc.strerror}", file=sys.stderr)
        release_mirror(mirror)
        os.close(fd)
        return 127

    def forward(signum, _frame):
        try:
            child.send_signal(signum)
        except ProcessLookupError:  # discipline:allow(error-swallowing): the command already exited; wait() reports its status
            pass

    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, forward)
    try:
        rc = child.wait()
    finally:
        release_mirror(mirror)
        os.close(fd)
    return 128 - rc if rc < 0 else rc


# --------------------------------------------------------------------------
# self-test (Linux lint runner and macOS)
# --------------------------------------------------------------------------
def _wait_for(path: Path, timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while not path.exists():
        if time.monotonic() > deadline:
            raise AssertionError(f"{path} never appeared")
        time.sleep(0.05)


def self_test() -> int:
    me = Path(__file__).resolve()
    with tempfile.TemporaryDirectory() as tmp:
        base_env = dict(os.environ)
        for k in (HELD_ENV, "GITHUB_OUTPUT", "EXPANSE_BENCH_LOCK_WAIT"):
            base_env.pop(k, None)
        base_env["EXPANSE_BENCH_FLOCK"] = str(Path(tmp, "b.flock"))
        base_env["EXPANSE_BENCH_LOCK"] = str(Path(tmp, "b.lock"))

        def locked(*cmd: str, wait: str = "0", extra: dict | None = None, flags: tuple = ()):
            return subprocess.run(
                [sys.executable, str(me), "--suite", "t", "--wait", wait, *flags, "--", *cmd],
                env=dict(base_env, **(extra or {})), capture_output=True, text=True,
            )

        def hold(marker: Path, suite: str = "holder") -> subprocess.Popen:
            p = subprocess.Popen(
                [sys.executable, str(me), "--suite", suite, "--wait", "0", "--",
                 "sh", "-c", f"echo $$ > {marker}; exec sleep 60"],
                env=base_env, start_new_session=True,
            )
            _wait_for(marker)
            return p

        def kill_tree(p: subprocess.Popen) -> None:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            p.wait()

        # 1. A free lock runs the command, passes its status through, and
        # leaves no mirror behind.
        r = locked("sh", "-c", "exit 3")
        assert r.returncode == 3, r
        assert not Path(tmp, "b.lock").exists(), "the mirror must be removed on exit"

        # 2. A held lock refuses after the wait, names the holder, and the
        # holder's text cannot close a markdown fence. The legacy mirror is
        # held as well, so an old-style runner is refused too.
        holder = hold(Path(tmp, "r2"), suite="hold`er")
        r = locked("true", wait="1")
        assert r.returncode == EX_HELD, r
        assert "suite=hold?er" in r.stderr, r.stderr
        assert "`" not in r.stderr.split("held for")[1], r.stderr
        assert Path(tmp, "b.lock", "owner").read_text().startswith("suite=hold?er"), "mirror not written"

        # 3. The motivating defect (run 36295887129): the holder is killed
        # outright. The lock frees itself; its leftover mirror is reclaimed.
        if sys.platform.startswith("linux"):
            # Kill the wrapper alone: its command must not run on unlocked.
            child_pid = int(Path(tmp, "r2").read_text().strip())
            holder.kill()
            holder.wait()
            deadline = time.monotonic() + 5
            while pid_alive(child_pid) and time.monotonic() < deadline:
                time.sleep(0.05)
            assert not pid_alive(child_pid), "the command outlived its lock holder"
        kill_tree(holder)
        assert Path(tmp, "b.lock").exists(), "SIGKILL leaves the mirror behind, as it left the old lock"
        r = locked("true", wait="5")
        assert r.returncode == 0, r
        assert "reclaiming the leftover" in r.stderr, r.stderr

        # 4. A grandchild that escapes into its own session keeps running but
        # does not keep the lock: the descriptor is never inherited.
        pidfile = Path(tmp, "gc.pid")
        detach = "import os,sys,time; os.setsid(); open(sys.argv[1],'w').write(str(os.getpid())); time.sleep(60)"
        r = locked("sh", "-c", f"{sys.executable} -c \"{detach}\" {pidfile} </dev/null >/dev/null 2>&1 &")
        assert r.returncode == 0, r
        _wait_for(pidfile)
        try:
            gc = int(pidfile.read_text().strip() or "0")
            assert gc and pid_alive(gc), "the grandchild must still be running for this check to mean anything"
            r = locked("true", wait="2")
            assert r.returncode == 0, ("a detached grandchild kept the lock", r)
        finally:
            try:
                os.kill(int(pidfile.read_text().strip()), signal.SIGKILL)
            except (ProcessLookupError, ValueError):
                pass

        # 5. A symlink at the lock path is refused, never followed.
        link, victim = Path(tmp, "link.flock"), Path(tmp, "victim")
        victim.write_text("keep me")
        link.symlink_to(victim)
        r = locked("true", extra={"EXPANSE_BENCH_FLOCK": str(link)})
        assert r.returncode == EX_CONFIG and "symlink" in r.stderr, r
        assert victim.read_text() == "keep me"

        # 6. An old-style holder that is running holds the host; a stopped
        # one's directory is reclaimed.
        Path(tmp, "b.lock").mkdir()
        Path(tmp, "b.lock", "owner").write_text(f"suite=old pid={os.getpid()} start=x\n")
        r = locked("true", wait="1")
        assert r.returncode == EX_HELD and "legacy lock directory" in r.stderr, r
        gone = subprocess.Popen(["true"])
        gone.wait()
        Path(tmp, "b.lock", "owner").write_text(f"suite=old pid={gone.pid} start=x\n")
        r = locked("true")
        assert r.returncode == 0, r

        # 7. Nested: a runner started under a held lock does not wait on it.
        r = locked("true", extra={HELD_ENV: base_env["EXPANSE_BENCH_FLOCK"]})
        assert r.returncode == 0, r

        # 8. A refusal reaches the workflow as a named cause.
        out = Path(tmp, "gh_out")
        holder = hold(Path(tmp, "r8"))
        try:
            r = locked("true", extra={"GITHUB_OUTPUT": str(out)}, flags=("--github-output",))
            assert r.returncode == EX_HELD, r
            text = out.read_text()
            assert "fail_reason=lock_held" in text and "lock_owner=suite=holder" in text, text
        finally:
            kill_tree(holder)

    # 9. Which owners are usable. The shared lock is root-owned, so this is
    # checked as a function: the self-test does not run as root.
    me_uid, grp = 1000, {1000, 2000}
    assert owner_problem(1000, 1000, 0o100644, me_uid, grp) is None
    assert owner_problem(0, 2000, 0o100660, me_uid, grp) is None
    assert "writable by every user" in (owner_problem(0, 2000, 0o100666, me_uid, grp) or "")
    assert "not in" in (owner_problem(0, 3000, 0o100660, me_uid, grp) or "")
    assert "another user created it" in (owner_problem(1001, 2000, 0o100660, me_uid, grp) or "")

    # 10. The default path: the shared lock when the host provides it, /tmp
    # otherwise, and an explicit EXPANSE_BENCH_FLOCK over both.
    global SHARED_LOCK
    saved_env, saved_shared = os.environ.pop("EXPANSE_BENCH_FLOCK", None), SHARED_LOCK
    try:
        with tempfile.TemporaryDirectory() as tmp:
            SHARED_LOCK = Path(tmp, "shared.flock")
            assert lock_path() == Path("/tmp/expanse-bench.flock"), lock_path()
            SHARED_LOCK.write_text("")
            assert lock_path() == SHARED_LOCK, lock_path()
            os.environ["EXPANSE_BENCH_FLOCK"] = str(Path(tmp, "explicit"))
            assert lock_path() == Path(tmp, "explicit"), lock_path()
    finally:
        SHARED_LOCK = saved_shared
        os.environ.pop("EXPANSE_BENCH_FLOCK", None)
        if saved_env is not None:
            os.environ["EXPANSE_BENCH_FLOCK"] = saved_env

    assert sanitize("a`b\nsecond") == "a?b"
    print("bench_lock.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--suite", default=os.environ.get("EXPANSE_BENCH_SUITE", "unnamed"), help="name recorded as the holder")
    ap.add_argument(
        "--wait",
        type=float,
        default=float(os.environ.get("EXPANSE_BENCH_LOCK_WAIT") or 0),
        help="seconds to wait for a held lock before refusing (default: $EXPANSE_BENCH_LOCK_WAIT, else 0)",
    )
    ap.add_argument("--github-output", action="store_true", help="on refusal, write fail_reason and lock_owner to $GITHUB_OUTPUT")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("cmd", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    cmd = args.cmd[1:] if args.cmd[:1] == ["--"] else args.cmd
    if not cmd:
        ap.error("a command to run under the lock is required (after --)")
    return run_locked(args.suite, args.wait, cmd, args.github_output)


if __name__ == "__main__":
    sys.exit(main())
