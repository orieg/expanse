#!/usr/bin/env bash
# scripts/bench_host/migrate.sh — move one self-hosted bench runner to the
# service account, from a workstation, in one command.
#
#   scripts/bench_host/migrate.sh <ssh-host> <reference|avx512> <runner-name> [login]
#
# On <ssh-host>, through sudo:
#   1. a root-owned checkout of main at /opt/expanse-provision (never a
#      directory the runner's jobs can write);
#   2. provision.sh prepare (leaves the running runner alone);
#   3. provision.sh cutover, with a remove and a registration token minted
#      here by `gh` and passed over ssh's stdin, never on a command line;
#   4. provision.sh check.
# [login] is the account the runner runs as today (default: the ssh user).
# Needs: `gh` authenticated with admin on orieg/expanse, and sudo on the host.
# See docs/CI.md, "Bare-Metal Benchmark Runner".

# Host-side values (DIR, PROFILE, NAME) are expanded here on purpose.
# shellcheck disable=SC2029
set -euo pipefail

[ $# -ge 3 ] || { sed -n '2,/^$/p' "$0"; exit 2; }
HOST="$1" PROFILE="$2" NAME="$3" LOGIN="${4:-}"
REPO=orieg/expanse
DIR=/opt/expanse-provision

case "$PROFILE" in reference|avx512) ;; *) echo "profile must be reference or avx512" >&2; exit 2 ;; esac
[[ "$NAME" =~ ^[A-Za-z0-9_.-]+$ ]] || { echo "bad runner name: $NAME" >&2; exit 2; }
gh api "repos/$REPO/actions/runners" --jq '.runners[].name' | grep -qx "$NAME" \
  || { echo "no runner named $NAME is registered on $REPO" >&2; exit 1; }

echo "== $HOST: checkout and prepare"
ssh "$HOST" "set -e
  L=\"${LOGIN:-\$(id -un)}\"
  if [ -d $DIR/.git ]; then sudo git -C $DIR fetch -q origin main && sudo git -C $DIR reset -q --hard origin/main
  else sudo git clone -q https://github.com/$REPO.git $DIR; fi
  sudo chown -R root:root $DIR && sudo chmod -R go-w $DIR
  echo \"checkout at \$(sudo git -C $DIR rev-parse --short HEAD)\"
  sudo $DIR/scripts/bench_host/provision.sh prepare --profile $PROFILE --from-login \"\$L\""

echo "== $HOST: cutover"
remove="$(gh api -X POST "repos/$REPO/actions/runners/remove-token" --jq .token)"
register="$(gh api -X POST "repos/$REPO/actions/runners/registration-token" --jq .token)"
# The tokens travel on stdin all the way into the root shell that runs
# provision.sh: no argv on this machine or the host ever carries them.
printf '%s\n%s\n' "$remove" "$register" | ssh "$HOST" "set -e
  L=\"${LOGIN:-\$(id -un)}\"
  exec sudo sh -c 'read -r REMOVE_TOKEN; read -r REG_TOKEN; export REMOVE_TOKEN REG_TOKEN
    exec $DIR/scripts/bench_host/provision.sh cutover --profile $PROFILE --from-login \"\$1\" --name $NAME' sh \"\$L\""
unset remove register

echo "== $HOST: check"
ssh "$HOST" "sudo $DIR/scripts/bench_host/provision.sh check"
gh api "repos/$REPO/actions/runners" --jq ".runners[] | select(.name==\"$NAME\") | \"\(.name) \(.status)\""
