#!/usr/bin/env bash
# scripts/bench_host/runner_check.sh — does this self-hosted runner still have
# the isolation docs/CI.md ("Threat model") requires?
#
# A tripwire for a misconfigured host, not a security control: a workflow a
# collaborator edits on their own branch can drop the step that calls it.
#
# Findings are fatal when the runner provides its own toolchain
# (EXPANSE_TOOLCHAIN set by the service account's .env) and a `::warning::`
# otherwise. The bench workflows refuse a runner without EXPANSE_TOOLCHAIN
# before calling this, so there they are always fatal. Exit 1 = fatal findings. Paths under $HOME are printed as `~`
# because the job log is public (AGENTS.md §7).
#
#   runner_check.sh [--self-test]

set -uo pipefail

findings() {
  local d t p r g
  local IFS=:
  for d in $PATH; do
    [ -n "$d" ] && [ -d "$d" ] && [ -w "$d" ] && echo "PATH entry $d is writable by this account"
  done
  unset IFS
  for t in cargo rustc valgrind iai-callgrind-runner perf; do
    p="$(command -v "$t" 2>/dev/null)" || continue
    r="$(readlink -f "$p")"
    [ -w "$r" ] && echo "$t resolves to $r, which this account can rewrite"
  done
  if command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
    echo "sudo -n true succeeds: a job can run anything as root"
  fi
  for g in $(id -nG); do
    case "$g" in
      sudo|wheel|admin|docker|lxd|libvirt|disk|adm|systemd-journal)
        echo "member of group '$g', which reaches root or other accounts' data" ;;
    esac
  done
}

redact() { if [ -n "${HOME:-}" ]; then sed "s|$HOME|~|g"; else cat; fi; }

main() {
  local out level
  out="$(findings | redact)"
  if [ -z "$out" ]; then
    echo "runner hardening: no findings"
    return 0
  fi
  if [ -n "${EXPANSE_TOOLCHAIN:-}" ]; then level=error; else level=warning; fi
  while IFS= read -r line; do
    echo "::$level::runner hardening: $line (docs/CI.md, Bare-Metal Benchmark Runner, Threat model)"
  done <<< "$out"
  [ "$level" = warning ] && return 0
  return 1
}

self_test() {
  local tmp rc log
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  mkdir -p "$tmp/bin" "$tmp/home"
  chmod 0755 "$tmp/bin"
  # A writable PATH entry under $HOME: warning without EXPANSE_TOOLCHAIN,
  # fatal with it, and printed with $HOME redacted either way.
  log="$(HOME="$tmp" PATH="$tmp/bin:/usr/bin:/bin" EXPANSE_TOOLCHAIN="" main 2>&1)"; rc=$?
  [ $rc -eq 0 ] || { echo "self-test: unmigrated host must not fail (rc=$rc)"; return 1; }
  grep -q '^::warning::runner hardening: PATH entry ~/bin is writable' <<< "$log" \
    || { echo "self-test: missing redacted warning:"; echo "$log"; return 1; }
  log="$(HOME="$tmp" PATH="$tmp/bin:/usr/bin:/bin" EXPANSE_TOOLCHAIN=/opt/x main 2>&1)"; rc=$?
  [ $rc -eq 1 ] || { echo "self-test: migrated host with a finding must fail (rc=$rc)"; return 1; }
  grep -q '^::error::runner hardening: PATH entry ~/bin is writable' <<< "$log" \
    || { echo "self-test: missing error line:"; echo "$log"; return 1; }
  grep -q "$tmp" <<< "$log" && { echo "self-test: \$HOME leaked into the log"; return 1; }
  # A tool that resolves into a writable location.
  printf '#!/bin/sh\n' > "$tmp/bin/cargo"; chmod 0755 "$tmp/bin/cargo"
  chmod 0555 "$tmp/bin"
  log="$(HOME="$tmp/home" PATH="$tmp/bin:/usr/bin:/bin" EXPANSE_TOOLCHAIN=/opt/x main 2>&1)"; rc=$?
  chmod 0755 "$tmp/bin"
  grep -q '::error::runner hardening: cargo resolves to' <<< "$log" \
    || { echo "self-test: writable tool not reported:"; echo "$log"; return 1; }
  echo "runner_check self-test: ok"
}

case "${1:-}" in
  --self-test) self_test ;;
  "") main ;;
  *) echo "usage: $0 [--self-test]" >&2; exit 2 ;;
esac
