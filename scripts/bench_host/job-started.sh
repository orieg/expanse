#!/usr/bin/env bash
# scripts/bench_host/job-started.sh — the self-hosted bench runner's
# job-started hook (docs/CI.md, "The job-started hook").
#
# Installed root-owned by provision.sh and named by the runner's
# ACTIONS_RUNNER_HOOK_JOB_STARTED. It runs as the service account before every
# job and removes what a previous job could have left in that account's own
# writable state. It cannot reach anything else, and does not need to: the
# account can write nothing else.

set -euo pipefail

me="$(id -u)"
[ "$me" -ne 0 ] || { echo "job-started: refusing to run as root" >&2; exit 1; }

# 1. Processes of this account outside the runner's own tree. Our ancestors are
#    the Runner.Worker / Runner.Listener chain that started this hook; our
#    descendants are the commands below. Everything else of this uid was left
#    running by an earlier job.
keep=" $$ "
p=$$
while [ "$p" -gt 1 ]; do
  p="$(ps -o ppid= -p "$p" | tr -d ' ')"
  [ -n "$p" ] || break
  keep="$keep$p "
done
stray=""
for pid in $(pgrep -u "$me" || true); do
  case "$keep" in *" $pid "*) continue ;; esac
  # A descendant of this hook (the pipeline running this loop).
  pp="$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ' || true)"
  [ "$pp" = "$$" ] && continue
  stray="$stray $pid"
done
if [ -n "$stray" ]; then
  echo "job-started: terminating processes a previous job left:$stray"
  # shellcheck disable=SC2086
  kill -TERM $stray 2>/dev/null || true
  sleep 2
  # shellcheck disable=SC2086
  kill -KILL $stray 2>/dev/null || true
fi

# 2. CARGO_HOME and the cache. Extracted crate sources in CARGO_HOME are built
#    by the next job and not re-verified against Cargo.lock, so none are kept.
wipe() {
  local dir="$1" want="$2"
  [ "$dir" = "$want" ] || { echo "job-started: $dir is not $want; not wiping it" >&2; exit 1; }
  [ -d "$dir" ] && [ -O "$dir" ] || { echo "job-started: $dir missing or not ours" >&2; exit 1; }
  find "$dir" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
}
wipe "${CARGO_HOME:-}" /var/lib/expanse-bench/cargo
wipe "${XDG_CACHE_HOME:-}" /var/lib/expanse-bench/cache

# 3. Shared-memory and temporary files this account owns. /tmp and /var/tmp are
#    private to the runner's unit (PrivateTmp=yes), so this touches nothing of
#    another account's.
for d in /dev/shm /tmp /var/tmp; do
  find "$d" -mindepth 1 -maxdepth 1 -user "$me" -exec rm -rf -- {} + 2>/dev/null || true
done

echo "job-started: account state reset"
