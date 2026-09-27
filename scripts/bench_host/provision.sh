#!/usr/bin/env bash
# scripts/bench_host/provision.sh — run a self-hosted bench runner under a
# dedicated, unprivileged service account.
#
# The arrangement, and the threat model it answers, are in docs/CI.md
# ("Bare-Metal Benchmark Runner", section 1). This script is that section made
# executable; where the two disagree, the doc is the specification.
#
# Run as root on the bench host, from a checkout of this repository:
#
#   provision.sh prepare --profile reference|avx512 --from-login <login> [--lock-user <login>]...
#       Account, groups, directories, lock files, sysctl, toolchain, hook, the
#       system unit (installed, NOT enabled). Idempotent. Does not touch the
#       runner that is running now.
#
#   provision.sh cutover --profile reference|avx512 --from-login <login> --name <runner-name>
#       Stops the login account's runner, deregisters it, and registers the
#       same name under the service account. Needs REMOVE_TOKEN and REG_TOKEN
#       in the environment, never on a command line (gh api -X POST repos/<owner>/<repo>/actions/runners/{remove,registration}-token).
#       Refuses while a job runs, while the bench lock is held, or while the
#       repository's main branch predates the workflow changes the service
#       account needs (EXPANSE_TOOLCHAIN in the workflow, the shared lock in
#       scripts/bench_lock.py).
#
#   provision.sh check
#       Read-only. Verifies the arrangement and prints one line per property;
#       exits non-zero on any FAIL.
#
# The scaling-governor sudoers rule is installed only when
# EXPANSE_GOVERNOR_FORMS names the helper's argument forms, one per line
# (e.g. "set performance"); without it the account has no sudo at all.

# ok/bad/todo always return 0, so `test && ok || bad` is an if-then-else here.
# shellcheck disable=SC2015
# shellcheck source-path=SCRIPTDIR
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
# shellcheck source=toolchain.env
. "$HERE/toolchain.env"

SVC=expanse-bench
LOCKGRP=expanse-benchlock
SVC_HOME=/var/lib/expanse-bench
TC=/opt/expanse-toolchain
RUNNER=/opt/actions-runner
UNIT=expanse-bench-runner.service
HOOK=/usr/local/libexec/expanse-bench/job-started
GOVERNOR=/usr/local/sbin/expanse-governor
LOCKDIR=/run/expanse-bench
FLOCK="$LOCKDIR/expanse-bench.flock"
GH_REPO_URL=https://github.com/orieg/expanse
GH_RAW=https://raw.githubusercontent.com/orieg/expanse/main
# Private, CGNAT (tailnets), link-local and ULA destinations.
DENY_NETS="10.0.0.0/8 172.16.0.0/12 192.168.0.0/16 100.64.0.0/10 169.254.0.0/16 fc00::/7 fe80::/10"

die() { echo "provision: $*" >&2; exit 1; }
say() { echo "provision: $*"; }

need_root() { [ "$(id -u)" -eq 0 ] || die "run as root"; }

PROFILE="" FROM_LOGIN="" NAME="" LOCK_USERS=()
parse_args() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --profile) PROFILE="$2"; shift 2 ;;
      --from-login) FROM_LOGIN="$2"; shift 2 ;;
      --name) NAME="$2"; shift 2 ;;
      --lock-user) LOCK_USERS+=("$2"); shift 2 ;;
      *) die "unknown argument: $1" ;;
    esac
  done
  # The commas below belong to the --labels value.
  # shellcheck disable=SC2054
  case "$PROFILE" in
    reference) LABEL_ARGS=(--labels baremetal,reference-host) ;;
    avx512) LABEL_ARGS=(--no-default-labels --labels avx512,zen5) ;;
    *) die "--profile must be 'reference' or 'avx512'" ;;
  esac
  [ -n "$FROM_LOGIN" ] || die "--from-login <login> is required (the account the runner runs as today)"
  getent passwd "$FROM_LOGIN" >/dev/null || die "no such login: $FROM_LOGIN"
  LOGIN_HOME="$(getent passwd "$FROM_LOGIN" | cut -d: -f6)"
  # The login that benchmarks by hand shares the lock by default.
  [ ${#LOCK_USERS[@]} -gt 0 ] || LOCK_USERS=("$FROM_LOGIN")
}

# `iai-callgrind-runner --version` run outside a bench reports its own version
# inside an error message, e.g. "... iai-callgrind-runner (0.16.1) is >= ...".
iai_runner_version() {
  { "$@" --version 2>&1 || true; } | grep -o '(\?[0-9][0-9.]*)\?' | tr -d '()' | head -1
}

as_svc() { runuser -u "$SVC" -- env -i HOME="$SVC_HOME" PATH="$SVC_PATH" RUSTUP_HOME="$TC/rustup" \
  CARGO_HOME="$SVC_HOME/cargo" "$@"; }
SVC_PATH="$TC/cargo/bin:$TC/bin:/usr/local/bin:/usr/bin:/bin"

# ---------------------------------------------------------------- prepare --
prepare_accounts() {
  getent group "$LOCKGRP" >/dev/null || groupadd --system "$LOCKGRP"
  if ! getent passwd "$SVC" >/dev/null; then
    useradd --system --user-group --home-dir "$SVC_HOME" --no-create-home \
      --shell /usr/sbin/nologin --groups "$LOCKGRP" "$SVC"
  fi
  passwd --lock "$SVC" >/dev/null
  # Exactly its own group plus the lock group: nothing that is root by another
  # name (sudo, docker, lxd, adm, disk, ...).
  usermod --groups "$LOCKGRP" "$SVC"
  for u in "${LOCK_USERS[@]}"; do usermod --append --groups "$LOCKGRP" "$u"; done

  install -d -o root -g root -m 0755 "$SVC_HOME"
  install -d -o "$SVC" -g "$SVC" -m 0700 "$SVC_HOME/cargo" "$SVC_HOME/cache"

  for f in /etc/cron.deny /etc/at.deny; do
    touch "$f"; grep -qx "$SVC" "$f" || echo "$SVC" >> "$f"
  done

  # Human home directories: not readable by other accounts.
  while IFS=: read -r _ _ uid _ _ home _; do
    [ "$uid" -ge 1000 ] && [ "$uid" -lt 60000 ] && [ -d "$home" ] || continue
    case "$home" in /home/*) chmod o-rwx "$home" ;; esac
  done < /etc/passwd
}

prepare_lock() {
  cat > /etc/tmpfiles.d/expanse-bench.conf <<EOF
# The host-wide benchmark lock (docs/CI.md, "The benchmark lock"). Root creates
# both files in a directory nobody else can write, so no account can replace
# the inode another account is locking.
d $LOCKDIR                           0755 root root       -
f $FLOCK                             0660 root $LOCKGRP   -
f $FLOCK.owner                       0660 root $LOCKGRP   -
EOF
  systemd-tmpfiles --create /etc/tmpfiles.d/expanse-bench.conf
  # Interactive shells, and non-interactive ssh sessions through pam_env.
  echo "export EXPANSE_BENCH_FLOCK=$FLOCK" > /etc/profile.d/expanse-bench-lock.sh
  chmod 0644 /etc/profile.d/expanse-bench-lock.sh
  if grep -q '^EXPANSE_BENCH_FLOCK=' /etc/environment; then
    sed -i "s|^EXPANSE_BENCH_FLOCK=.*|EXPANSE_BENCH_FLOCK=$FLOCK|" /etc/environment
  else
    echo "EXPANSE_BENCH_FLOCK=$FLOCK" >> /etc/environment
  fi
}

prepare_sysctl() {
  # The value every committed counter artifact records; permits per-process
  # counting and sampling, not system-wide monitoring.
  echo "kernel.perf_event_paranoid = 1" > /etc/sysctl.d/60-expanse-bench-perf.conf
  sysctl -q -p /etc/sysctl.d/60-expanse-bench-perf.conf
  local perf; perf="$(command -v perf || true)"
  if [ -n "$perf" ] && [ -n "$(getcap "$(readlink -f "$perf")" 2>/dev/null)" ]; then
    die "$(readlink -f "$perf") carries file capabilities; remove them (setcap -r) — docs/CI.md, 'Hardware counters'"
  fi
}

prepare_toolchain() {
  install -d -o root -g root -m 0755 "$TC" "$TC/bin" "$TC/lib" "$TC/include" "$TC/src"
  export RUSTUP_HOME="$TC/rustup" CARGO_HOME="$TC/cargo"
  if [ ! -x "$TC/cargo/bin/rustup" ]; then
    local init="$TC/src/rustup-init"
    curl --proto '=https' --tlsv1.2 -sSfL -o "$init" \
      https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
    curl --proto '=https' --tlsv1.2 -sSfL -o "$init.sha256" \
      https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init.sha256
    (cd "$TC/src" && sed 's|\*.*|*rustup-init|' rustup-init.sha256 | sha256sum -c -)
    chmod 0755 "$init"
    "$init" -y --no-modify-path --profile minimal --default-toolchain "$RUST_STABLE"
  fi
  "$TC/cargo/bin/rustup" toolchain install "$RUST_STABLE" --profile minimal
  "$TC/cargo/bin/rustup" default "$RUST_STABLE"
  if [ "$PROFILE" = reference ]; then
    # shellcheck disable=SC2086
    "$TC/cargo/bin/rustup" toolchain install "$RUST_NIGHTLY" --profile minimal \
      --component $RUST_NIGHTLY_COMPONENTS
  fi

  if [ "$(iai_runner_version "$TC/bin/iai-callgrind-runner")" != "$IAI_CALLGRIND_RUNNER" ]; then
    # Built from crates.io at the pinned version, never copied out of a home
    # directory a job could write.
    "$TC/cargo/bin/cargo" install --locked --force --root "$TC" \
      --version "$IAI_CALLGRIND_RUNNER" iai-callgrind-runner
  fi

  if [ "$PROFILE" = reference ]; then
    if [ "$("$TC/bin/valgrind" --version 2>/dev/null || true)" != "valgrind-$VALGRIND_VERSION" ]; then
      local tb="$TC/src/valgrind-$VALGRIND_VERSION.tar.bz2"
      curl -sSfL -o "$tb" "$VALGRIND_URL"
      echo "$VALGRIND_SHA256  $tb" | sha256sum -c -
      rm -rf "$TC/src/valgrind-$VALGRIND_VERSION"
      tar -C "$TC/src" -xjf "$tb"
      (cd "$TC/src/valgrind-$VALGRIND_VERSION" && ./configure --prefix="$TC" >/dev/null \
        && make -j"$(nproc)" >/dev/null && make install >/dev/null)
    fi

    # Stock libjudy: the object the published comparisons were measured against.
    local src="$LOGIN_HOME/.local/lib/$LIBJUDY_SONAME"
    if ! echo "$LIBJUDY_SHA256  $TC/lib/$LIBJUDY_SONAME" | sha256sum -c - >/dev/null 2>&1; then
      [ -f "$src" ] || die "no $src to copy stock libjudy from"
      echo "$LIBJUDY_SHA256  $src" | sha256sum -c - \
        || die "$src does not hash to the pinned LIBJUDY_SHA256; refusing to install it"
      install -o root -g root -m 0755 "$src" "$TC/lib/$LIBJUDY_SONAME"
      ln -sfn "$LIBJUDY_SONAME" "$TC/lib/libJudy.so.1"
      ln -sfn "$LIBJUDY_SONAME" "$TC/lib/libJudy.so"
    fi
    # The header comes out of the same job-writable directory, so it is held to
    # a pinned hash as well: a planted one would be compiled into every build.
    local hdr="$LOGIN_HOME/.local/include/Judy.h"
    if [ -f "$hdr" ] && ! echo "$LIBJUDY_HEADER_SHA256  $TC/include/Judy.h" | sha256sum -c - >/dev/null 2>&1; then
      [ -n "${LIBJUDY_HEADER_SHA256:-}" ] || die "LIBJUDY_HEADER_SHA256 is not set in toolchain.env"
      echo "$LIBJUDY_HEADER_SHA256  $hdr" | sha256sum -c - \
        || die "$hdr does not hash to the pinned LIBJUDY_HEADER_SHA256; refusing to install it"
      install -o root -g root -m 0644 "$hdr" "$TC/include/Judy.h"
    fi
  fi
  chown -R root:root "$TC"
  chmod -R go-w "$TC"
}

prepare_hook_and_unit() {
  install -d -o root -g root -m 0755 "$(dirname "$HOOK")"
  install -o root -g root -m 0755 "$HERE/job-started.sh" "$HOOK"

  cat > "/etc/systemd/system/$UNIT" <<EOF
# docs/CI.md, "Supervision". Installed by scripts/bench_host/provision.sh.
[Unit]
Description=GitHub Actions runner (expanse benchmarks, $PROFILE)
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=$SVC
Group=$SVC
SupplementaryGroups=$LOCKGRP
WorkingDirectory=$RUNNER
ExecStart=$RUNNER/run.sh
Restart=always
RestartSec=10
# A bench suite must finish rather than be cut mid-measurement.
TimeoutStopSec=30min
KillMode=process
ProtectHome=yes
PrivateTmp=yes
ProtectSystem=full
IPAddressDeny=$DENY_NETS
# NoNewPrivileges= stays unset: it disables sudo, and with it the governor helper.

[Install]
WantedBy=multi-user.target
EOF
  if [ -d /run/systemd/system ]; then
    systemctl daemon-reload
  else
    say "systemd is not running here; $UNIT written but not loaded"
  fi
}

prepare_sudoers() {
  local f=/etc/sudoers.d/expanse-bench
  if [ -z "${EXPANSE_GOVERNOR_FORMS:-}" ]; then
    rm -f "$f"
    say "no EXPANSE_GOVERNOR_FORMS: $SVC gets no sudo rule (the governor helper stays unusable to it)"
    return
  fi
  [ -x "$GOVERNOR" ] || die "$GOVERNOR is not installed; install the #1213 helper before its sudoers rule"
  [ "$(stat -c %U:%a "$GOVERNOR")" = "root:755" ] || die "$GOVERNOR must be root-owned 0755"
  local tmp; tmp="$(mktemp)"
  {
    echo "# docs/CI.md, 'Scaling governor'. Installed by scripts/bench_host/provision.sh."
    printf 'Cmnd_Alias EXPANSE_GOVERNOR ='
    local first=1
    while IFS= read -r form; do
      [ -n "$form" ] || continue
      [[ "$form" =~ ^[a-z0-9_.\ -]+$ ]] || die "governor form has characters outside [a-z0-9_. -]: $form"
      [ $first -eq 1 ] || printf ','
      printf ' \\\n    %s %s' "$GOVERNOR" "$form"
      first=0
    done <<< "$EXPANSE_GOVERNOR_FORMS"
    printf '\n%s ALL=(root) NOPASSWD: EXPANSE_GOVERNOR\n' "$SVC"
  } > "$tmp"
  visudo -cf "$tmp" >/dev/null || die "generated sudoers rule does not parse"
  install -o root -g root -m 0440 "$tmp" "$f"
  rm -f "$tmp"
}

cmd_prepare() {
  need_root; parse_args "$@"
  prepare_accounts; prepare_lock; prepare_sysctl; prepare_toolchain
  prepare_hook_and_unit; prepare_sudoers
  say "prepared ($PROFILE). The running runner is unchanged; '$0 check' reports what cutover still needs."
}

# ---------------------------------------------------------------- cutover --
cmd_cutover() {
  need_root; parse_args "$@"
  [ -n "$NAME" ] || die "--name <runner-name> is required"
  [ -n "${REMOVE_TOKEN:-}" ] && [ -n "${REG_TOKEN:-}" ] || die "REMOVE_TOKEN and REG_TOKEN must be set"

  # The service account needs the workflow to take its toolchain from the
  # runner (docs/CI.md) and the flock lock that can span accounts (#1226).
  # Fetched whole, then matched: `curl | grep -q` fails under pipefail on a
  # file this large, because grep exits at the first match and curl's next
  # write fails (exit 23).
  local wf lk
  wf="$(curl -sSfL "$GH_RAW/.github/workflows/bench_baremetal.yml")" \
    || die "could not fetch main's bench_baremetal.yml"
  case "$wf" in *EXPANSE_TOOLCHAIN*) ;; *) die "main's bench_baremetal.yml predates EXPANSE_TOOLCHAIN; merge that change first" ;; esac
  lk="$(curl -sSfL "$GH_RAW/scripts/bench_lock.py")" || die "could not fetch main's scripts/bench_lock.py"
  case "$lk" in *SHARED_LOCK*) ;; *) die "main's scripts/bench_lock.py does not accept the shared lock; merge that change first" ;; esac

  pgrep -f 'Runner.Worker' >/dev/null && die "a job is running; retry when the runner is idle"
  for l in /tmp/expanse-bench.flock "$FLOCK"; do
    [ -e "$l" ] || continue
    flock -n "$l" true || die "$l is held; retry when no benchmark runs"
  done
  [ -d /tmp/expanse-bench.lock ] && die "/tmp/expanse-bench.lock (mkdir lock) exists; a benchmark holds the host"

  local uid; uid="$(id -u "$FROM_LOGIN")"
  runuser -u "$FROM_LOGIN" -- env XDG_RUNTIME_DIR="/run/user/$uid" \
    systemctl --user disable --now gh-runner.service || true
  pgrep -u "$FROM_LOGIN" -f 'Runner.Listener' >/dev/null && die "the login's runner is still running"

  cd "$RUNNER"
  # Tokens reach config.sh through ACTIONS_RUNNER_INPUT_TOKEN, which the
  # runner reads as its --token argument and masks as a secret. A token on
  # a command line is readable by every local account through /proc.
  if [ -f .runner ]; then
    ACTIONS_RUNNER_INPUT_TOKEN="$REMOVE_TOKEN" runuser -u "$(stat -c %U .runner)" -- ./config.sh remove
  fi
  # Everything in these was written under the previous account.
  rm -rf _work _diag .env .path

  chown -R "$SVC:$SVC" "$RUNNER"
  ACTIONS_RUNNER_INPUT_TOKEN="$REG_TOKEN" runuser -u "$SVC" -- \
    env HOME="$SVC_HOME" PATH=/usr/bin:/bin ./config.sh \
    --url "$GH_REPO_URL" --name "$NAME" "${LABEL_ARGS[@]}" \
    --work _work --unattended --disableupdate
  unset REMOVE_TOKEN REG_TOKEN

  chown -R root:root "$RUNNER"
  install -d -o "$SVC" -g "$SVC" -m 0750 "$RUNNER/_work" "$RUNNER/_diag"
  chgrp "$SVC" .credentials .credentials_rsaparams
  chmod 0640 .credentials .credentials_rsaparams
  chmod 0644 .runner
  echo "$SVC_PATH" > .path
  cat > .env <<EOF
LANG=en_US.UTF-8
EXPANSE_TOOLCHAIN=$TC
RUSTUP_HOME=$TC/rustup
CARGO_HOME=$SVC_HOME/cargo
XDG_CACHE_HOME=$SVC_HOME/cache
LD_LIBRARY_PATH=$TC/lib
LIBRARY_PATH=$TC/lib
C_INCLUDE_PATH=$TC/include
EXPANSE_BENCH_FLOCK=$FLOCK
ACTIONS_RUNNER_HOOK_JOB_STARTED=$HOOK
EOF
  chmod 0644 .env .path

  systemctl enable --now "$UNIT"
  say "cut over. Run '$0 check', then the post-cutover runs in the migration issue."
}

# ------------------------------------------------------------------ check --
FAILS=0
ok()   { echo "  ok    $*"; }
bad()  { echo "  FAIL  $*"; FAILS=$((FAILS + 1)); }
todo() { echo "  todo  $*"; }

cmd_check() {
  need_root
  echo "account"
  if getent passwd "$SVC" >/dev/null; then
    ok "$SVC exists"
    local groups; groups="$(id -nG "$SVC" | tr ' ' '\n' | sort | tr '\n' ' ')"
    [ "$groups" = "$(printf '%s\n' "$SVC" "$LOCKGRP" | sort | tr '\n' ' ')" ] \
      && ok "groups: $groups" || bad "groups are '$groups' (want $SVC $LOCKGRP only)"
    [ "$(passwd -S "$SVC" | awk '{print $2}')" = L ] && ok "password locked" || bad "password not locked"
    if runuser -u "$SVC" -- sudo -n true 2>/dev/null; then bad "$SVC can run arbitrary commands through sudo"
    else ok "$SVC has no general sudo"; fi
    local rules; rules="$(sudo -l -U "$SVC" 2>/dev/null | sed -n '/may run the following/,$p' | tail -n +2 | grep -v "$GOVERNOR" | grep -v '^[[:space:]]*$' || true)"
    [ -z "$rules" ] && ok "sudo rules name only $GOVERNOR" || bad "unexpected sudo rules: $rules"
    [ -f /etc/sudoers.d/expanse-bench ] || todo "no governor sudoers rule (set EXPANSE_GOVERNOR_FORMS once #1213's helper is installed)"
    grep -qx "$SVC" /etc/cron.deny && ok "cron denied" || bad "$SVC not in /etc/cron.deny"
    [ "$(stat -c %U:%a "$SVC_HOME")" = "root:755" ] && ok "\$HOME is root-owned" || bad "$SVC_HOME is not root:755"
    local wr; wr="$(runuser -u "$SVC" -- find "$SVC_HOME" -maxdepth 1 -writable 2>/dev/null | grep -vx -e "$SVC_HOME/cargo" -e "$SVC_HOME/cache" || true)"
    [ -z "$wr" ] && ok "only cargo/ and cache/ writable in \$HOME" || bad "writable in \$HOME: $wr"
    local rh; rh="$(runuser -u "$SVC" -- find /home -mindepth 2 -maxdepth 2 -readable 2>/dev/null | head -3 || true)"
    [ -z "$rh" ] && ok "no readable entries under /home" || bad "readable under /home: $rh"
  else
    bad "$SVC does not exist (run prepare)"
  fi

  echo "toolchain"
  local d; for d in ${SVC_PATH//:/ }; do
    [ -d "$d" ] || continue
    if runuser -u "$SVC" -- test -w "$d" 2>/dev/null; then bad "PATH entry $d is writable by $SVC"; fi
  done
  ok "PATH entries checked for writability"
  local v
  v="$(as_svc rustc -V 2>/dev/null || true)"; [[ "$v" == "rustc $RUST_STABLE "* ]] && ok "$v" || bad "rustc is '$v' (want $RUST_STABLE)"
  v="$(iai_runner_version as_svc iai-callgrind-runner)"
  local lockv; lockv="$(grep -A1 '^name = "iai-callgrind"$' "$REPO/Cargo.lock" | sed -n 's/^version = "\(.*\)"/\1/p')"
  [ "$v" = "$IAI_CALLGRIND_RUNNER" ] && [ "$v" = "$lockv" ] && ok "iai-callgrind-runner $v = Cargo.lock" \
    || bad "iai-callgrind-runner '$v', pinned $IAI_CALLGRIND_RUNNER, Cargo.lock $lockv"
  if [ -x "$TC/bin/valgrind" ]; then
    v="$("$TC/bin/valgrind" --version)"; [ "$v" = "valgrind-$VALGRIND_VERSION" ] && ok "$v" || bad "valgrind is $v"
  elif command -v valgrind >/dev/null; then ok "system $(valgrind --version)"; else todo "no valgrind"; fi
  if [ -e "$TC/lib/$LIBJUDY_SONAME" ]; then
    echo "$LIBJUDY_SHA256  $TC/lib/$LIBJUDY_SONAME" | sha256sum -c - >/dev/null 2>&1 \
      && ok "stock libjudy hash pinned" || bad "stock libjudy hash differs"
  fi
  local w; w="$(runuser -u "$SVC" -- find "$TC" -writable 2>/dev/null | head -3 || true)"
  [ -z "$w" ] && ok "$TC not writable by $SVC" || bad "writable in $TC: $w"

  echo "perf"
  [ "$(sysctl -n kernel.perf_event_paranoid)" = 1 ] && ok "perf_event_paranoid=1" || bad "perf_event_paranoid=$(sysctl -n kernel.perf_event_paranoid)"
  local perf; perf="$(command -v perf || true)"
  [ -z "$perf" ] || [ -z "$(getcap "$(readlink -f "$perf")")" ] && ok "no file capabilities on perf" || bad "perf carries file capabilities"

  echo "lock"
  [ "$(stat -c %U:%G:%a "$FLOCK" 2>/dev/null)" = "root:$LOCKGRP:660" ] && ok "$FLOCK root:$LOCKGRP 0660" || bad "$FLOCK missing or wrong mode"
  [ "$(stat -c %U:%a "$LOCKDIR" 2>/dev/null)" = "root:755" ] && ok "$LOCKDIR root 0755" || bad "$LOCKDIR missing or wrong mode"
  if getent passwd "$SVC" >/dev/null && runuser -u "$SVC" -- flock -n "$FLOCK" true; then ok "$SVC can take the lock"; else bad "$SVC cannot take the lock"; fi

  echo "runner"
  if [ ! -f "$RUNNER/.runner" ] || [ "$(stat -c %U "$RUNNER/.runner")" != root ]; then
    todo "runner not cut over (run '$0 cutover')"
  else
    w="$(runuser -u "$SVC" -- find "$RUNNER" -maxdepth 1 -writable 2>/dev/null | grep -vx -e "$RUNNER/_work" -e "$RUNNER/_diag" || true)"
    [ -z "$w" ] && ok "runner install not writable by $SVC" || bad "writable in $RUNNER: $w"
    grep -q '^EXPANSE_TOOLCHAIN=' "$RUNNER/.env" 2>/dev/null && ok ".env provides EXPANSE_TOOLCHAIN" || bad ".env lacks EXPANSE_TOOLCHAIN"
    systemctl is-active --quiet "$UNIT" && ok "$UNIT active" || bad "$UNIT not active"
    [ "$(ps -o user= -p "$(pgrep -f 'Runner.Listener' | head -1)" 2>/dev/null | tr -d ' ')" = "$SVC" ] \
      && ok "Runner.Listener runs as $SVC" || bad "Runner.Listener does not run as $SVC"
  fi
  [ -f "/etc/systemd/system/$UNIT" ] && grep -q '^IPAddressDeny=' "/etc/systemd/system/$UNIT" \
    && ok "unit denies private networks" || bad "unit missing or lacks IPAddressDeny"

  echo
  [ "$FAILS" -eq 0 ] && echo "check: no failures" || { echo "check: $FAILS failure(s)"; exit 1; }
}

case "${1:-}" in
  prepare) shift; cmd_prepare "$@" ;;
  cutover) shift; cmd_cutover "$@" ;;
  check) shift; cmd_check "$@" ;;
  *) sed -n '2,/^$/p' "$0"; exit 2 ;;
esac
