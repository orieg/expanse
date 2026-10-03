#!/bin/bash
set -euo pipefail

# Set or rotate the secrets the release workflows read from GitHub Environments.
#
# Usage: scripts/release_secrets.sh <command>
#
#   status                     per environment, each secret a workflow reads and
#                              whether it is set; names only, never values
#   package-signing            upload the APT/RPM repository signing key and its
#                              passphrase (generates the key if the keyring has none)
#   maven-key                  upload the Maven Central signing key and its passphrase
#   token ENV NAME             set one secret from a hidden prompt
#   rotate-deploy-keys [NAME…] replace the SSH deploy keys (default: all of them)
#   --self-test                run every path against throwaway keyrings
#
# DRY_RUN=1 uploads nothing and prints the byte count each secret would carry.
#
# A secret's value reaches `gh` on stdin from a hidden prompt, from `gpg`, or
# from a key generated here. It is never a command-line argument, so it is in
# no shell history and no process listing.
#
# A signing key is uploaded only with the passphrase that unlocks it. The
# workflows import the key into an empty keyring and sign through loopback
# pinentry (`pages.yml` `sign`, `release.yml` `package-maven`); `check_key`
# does the same here first. GitHub never returns a secret, so a mismatched pair
# would otherwise surface at the next release.
#
# `status` reads the names from the workflows themselves: a hand-kept list of
# secret names drifts from the workflows that read them.

REPO="${REPO:-orieg/expanse}"
MAVEN_FPR="${MAVEN_FPR:-995C1FA9F413909685F3E91E7509E2D8A6A63BDE}"
# The repository signing key is found by this uid comment (docs/PACKAGING.md §2.3.1).
PKG_UID_COMMENT="${PKG_UID_COMMENT:-expanse packages}"

# environment:secret:repository whose deploy key it is
DEPLOY_KEYS="
release:HOMEBREW_TAP_DEPLOY_KEY:orieg/homebrew-tap
subsplit:PHP_LIBRARY_SUBSPLIT_SSH_KEY:orieg/expanse-php-library
subsplit:PHP_SUBSPLIT_SSH_KEY:orieg/php-expanse
"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

# put ENV NAME: the value arrives on stdin.
put() {
  if [ -n "${DRY_RUN:-}" ]; then
    printf '  DRY_RUN: %s/%s <- %s bytes\n' "$1" "$2" "$(wc -c | tr -d ' ')"
  else
    gh secret set "$2" --env "$1" --repo "$REPO"
  fi
}

# ask_hidden VAR PROMPT
ask_hidden() {
  local __v=""
  printf '%s: ' "$2" >&2
  # EOF leaves the value empty, which every caller treats as "skip" or refuses.
  IFS= read -rs __v || true
  printf '\n' >&2
  printf -v "$1" '%s' "$__v"
}

kill_agent() { # kill_agent GNUPGHOME: best effort, the directory is removed next
  GNUPGHOME="$1" gpgconf --kill gpg-agent 2>/dev/null || true
}

# workflow_secrets: one "environment<TAB>name" line per secret a workflow job
# reads, `-` for a job bound to no environment. GITHUB_TOKEN is not a stored secret.
workflow_secrets() {
  local f
  for f in "$ROOT"/.github/workflows/*.yml; do
    awk '
      BEGIN { env = "-" }
      /^  [A-Za-z0-9_-]+:[ \t]*$/ { env = "-"; pending = 0 }
      /^    environment:[ \t]*[^ \t#]/ { env = $2; pending = 0; next }
      /^    environment:[ \t]*$/ { pending = 1; next }
      pending && /^      name:/ { env = $2; pending = 0 }
      {
        line = $0
        while (match(line, /secrets\.[A-Z0-9_]+/)) {
          name = substr(line, RSTART + 8, RLENGTH - 8)
          if (name != "GITHUB_TOKEN") print env "\t" name
          line = substr(line, RSTART + RLENGTH)
        }
      }
    ' "$f"
  done | sort -u
}

# export_key FPR PASSPHRASE: armored secret key on stdout.
export_key() {
  gpg --batch --pinentry-mode loopback --passphrase-fd 3 --armor \
    --export-secret-keys "$1" 3< <(printf '%s' "$2")
}

# check_key ARMORED PASSPHRASE: the pair signs in a keyring with no agent cache.
check_key() {
  local home ok=0
  home="$(mktemp -d)"
  chmod 700 "$home"
  echo "allow-loopback-pinentry" > "$home/gpg-agent.conf"
  if printf '%s\n' "$1" | GNUPGHOME="$home" gpg --batch --quiet --import 2>/dev/null &&
    printf 'probe\n' | GNUPGHOME="$home" gpg --batch --quiet --pinentry-mode loopback \
      --passphrase-fd 3 --clearsign 3< <(printf '%s' "$2") >/dev/null 2>&1; then
    ok=1
  fi
  kill_agent "$home"
  rm -rf "$home"
  [ "$ok" = 1 ]
}

# upload_key_pair ENV KEY_NAME PASSPHRASE_NAME FPR PASSPHRASE
upload_key_pair() {
  local armored
  armored="$(export_key "$4" "$5")" || die "gpg could not export $4 (wrong passphrase?)"
  case "$armored" in
    *"BEGIN PGP PRIVATE KEY BLOCK"*) ;;
    *) die "export of $4 holds no private key" ;;
  esac
  check_key "$armored" "$5" || die "key $4 does not sign with the passphrase given; nothing uploaded"
  printf '%s\n' "$armored" | put "$1" "$2"
  printf '%s' "$5" | put "$1" "$3"
}

package_key_fpr() {
  gpg --batch --list-secret-keys --with-colons 2>/dev/null | awk -F: -v c="($PKG_UID_COMMENT)" '
    /^fpr/ && fpr == "" { fpr = $10 }
    /^uid/ && index($10, c) { print fpr; exit }
    /^sec/ { fpr = "" }'
}

cmd_package_signing() {
  local fpr pp pp2 uid
  fpr="$(package_key_fpr)"
  if [ -z "$fpr" ]; then
    uid="${PKG_UID:-$(git -C "$ROOT" config user.name) ($PKG_UID_COMMENT) <$(git -C "$ROOT" config user.email)>}"
    # Sign-only and no subkey: `gpg --quick-set-expire <fingerprint>` extends
    # the primary key alone, which is the renewal docs/PACKAGING.md §2.3.1 gives.
    echo "No secret key with uid comment ($PKG_UID_COMMENT): generating \"$uid\", RSA 4096, sign-only, no subkey, 4y."
    ask_hidden pp "New passphrase for the package signing key"
    ask_hidden pp2 "Repeat"
    [ -n "$pp" ] || die "empty passphrase"
    [ "$pp" = "$pp2" ] || die "the two passphrases differ"
    gpg --batch --quiet --pinentry-mode loopback --passphrase-fd 3 \
      --quick-generate-key "$uid" rsa4096 sign 4y 3< <(printf '%s' "$pp")
    fpr="$(package_key_fpr)"
    [ -n "$fpr" ] || die "key generation left no secret key"
  else
    echo "Using key $fpr"
    ask_hidden pp "Passphrase of $fpr"
  fi
  upload_key_pair package-signing REPO_SIGNING_KEY REPO_SIGNING_PASSPHRASE "$fpr" "$pp"
  echo "Package signing key: $fpr"
}

cmd_maven_key() {
  local pp
  ask_hidden pp "Passphrase of the Maven signing key $MAVEN_FPR"
  [ -n "$pp" ] || die "empty passphrase"
  upload_key_pair release MAVEN_GPG_PRIVATE_KEY MAVEN_GPG_PASSPHRASE "$MAVEN_FPR" "$pp"
}

cmd_token() {
  local v
  [ $# -eq 2 ] || die "usage: token ENV NAME"
  ask_hidden v "$2 for the $1 environment"
  [ -n "$v" ] || die "empty value; $2 left unchanged"
  printf '%s' "$v" | put "$1" "$2"
}

# rotate_key ENV NAME TARGET_REPO: the public half is registered on TARGET_REPO
# with write access before the private half becomes the secret, so the secret
# never names a key the target does not accept. The private half exists only in
# a temporary directory. The key it replaces stays registered: remove it once
# the new one has pushed.
rotate_key() {
  local dir fp
  dir="$(mktemp -d)"
  ssh-keygen -q -t ed25519 -N '' -C "$2" -f "$dir/key"
  fp="$(ssh-keygen -l -f "$dir/key.pub" | awk '{ print $2 }')"
  if [ -n "${DRY_RUN:-}" ]; then
    echo "  DRY_RUN: would register $fp as a write deploy key on $3"
  elif ! gh api -X POST "repos/$3/keys" -f title="expanse $1 environment ($2)" \
    -f key="$(cat "$dir/key.pub")" -F read_only=false \
    --jq '"  registered deploy key id \(.id) on '"$3"'"'; then
    rm -rf "$dir"
    die "could not register a deploy key on $3; $2 left unchanged"
  fi
  if ! put "$1" "$2" < "$dir/key"; then
    rm -rf "$dir"
    die "deploy key $fp is registered on $3 but $2 was not set; remove that deploy key and re-run"
  fi
  rm -rf "$dir"
  echo "  $2 <- $fp"
  if [ -z "${DRY_RUN:-}" ]; then
    echo "  deploy keys now on $3 (remove a superseded one with: gh api -X DELETE repos/$3/keys/<id>):"
    gh api "repos/$3/keys" --jq '.[] | "    \(.id)  \(.title)"'
  fi
}

cmd_rotate_deploy_keys() {
  local entry env name target want found
  for want in "$@"; do
    found=0
    for entry in $DEPLOY_KEYS; do
      name="${entry#*:}"
      [ "${name%%:*}" = "$want" ] && found=1
    done
    [ "$found" = 1 ] || die "$want is not a deploy-key secret this script rotates"
  done
  for entry in $DEPLOY_KEYS; do
    env="${entry%%:*}"
    name="${entry#*:}"
    target="${name#*:}"
    name="${name%%:*}"
    if [ $# -gt 0 ]; then
      case " $* " in *" $name "*) ;; *) continue ;; esac
    fi
    rotate_key "$env" "$name" "$target"
  done
}

cmd_status() {
  local map env name present repo_level
  map="$(workflow_secrets)"
  repo_level="$(gh secret list --repo "$REPO" --json name --jq '.[].name')"
  for env in $(printf '%s\n' "$map" | cut -f1 | sort -u); do
    if [ "$env" = "-" ]; then
      echo "[read by a job bound to no environment]"
      present="$repo_level"
    else
      echo "[$env]"
      present="$(gh secret list --env "$env" --repo "$REPO" --json name --jq '.[].name')"
    fi
    for name in $(printf '%s\n' "$map" | awk -F'\t' -v e="$env" '$1 == e { print $2 }'); do
      if printf '%s\n' "$present" | grep -qx -- "$name"; then
        echo "  set      $name"
      elif [ "$env" != "-" ] && printf '%s\n' "$repo_level" | grep -qx -- "$name"; then
        echo "  REPO     $name (repository level only; any workflow can read it)"
      else
        echo "  unset    $name"
      fi
    done
  done
}

# --- self-test ---------------------------------------------------------------
# Throwaway keyrings and DRY_RUN throughout: it calls neither GitHub nor the
# caller's keyring. Each refusal is asserted on its message, not on a non-zero
# exit alone, which a missing tool would also produce.

st_fail() {
  printf 'self-test FAILED: %s\n' "$*" >&2
  exit 1
}

# st_refuses LABEL EXPECTED_MESSAGE STDIN COMMAND...
st_refuses() {
  local label="$1" want="$2" input="$3" out
  shift 3
  if out="$(printf '%b' "$input" | ("$@") 2>&1)"; then
    st_fail "$label: accepted, expected a refusal"
  fi
  case "$out" in
    *"$want"*) ;;
    *) st_fail "$label: refused without \"$want\": $out" ;;
  esac
  case "$out" in
    *"DRY_RUN:"*) st_fail "$label: reached an upload before refusing" ;;
  esac
}

cmd_self_test() {
  local out fpr armored names entry n last wrote=""
  export GNUPGHOME
  GNUPGHOME="$(mktemp -d)"
  chmod 700 "$GNUPGHOME"
  # Not a `local`: the EXIT trap runs after this function's scope is gone.
  ST_HOME="$GNUPGHOME"
  trap 'kill_agent "$ST_HOME"; rm -rf "$ST_HOME"' EXIT
  export DRY_RUN=1
  PKG_UID="Throwaway ($PKG_UID_COMMENT) <test@example.invalid>"

  # Generation: both secrets, from a sign-only key with no subkey.
  st_refuses "mismatched repeat" "the two passphrases differ" 'a\nb\n' cmd_package_signing
  st_refuses "empty passphrase" "empty passphrase" '\n\n' cmd_package_signing
  out="$(printf 'pp-one\npp-one\n' | cmd_package_signing 2>/dev/null)" || st_fail "key generation failed"
  case "$out" in
    *"package-signing/REPO_SIGNING_KEY <- "*"package-signing/REPO_SIGNING_PASSPHRASE <- 6 bytes"*) ;;
    *) st_fail "generation did not reach both uploads: $out" ;;
  esac
  wrote="$out"
  fpr="$(package_key_fpr)"
  [ -n "$fpr" ] || st_fail "the generated key is not found by its uid comment"
  gpg --batch --list-secret-keys --with-colons "$fpr" | awk -F: '
    /^sec/ && $12 !~ /^[scSC]+$/ { bad = 1 }
    /^ssb/ { bad = 1 }
    END { exit bad }' || st_fail "the generated key is not sign-only with no subkey"

  # An existing key: reused, and refused under the wrong passphrase.
  out="$(printf 'pp-one\n' | cmd_package_signing 2>/dev/null)" || st_fail "an existing key was refused"
  case "$out" in
    *"Using key $fpr"*"REPO_SIGNING_PASSPHRASE <- 6 bytes"*) ;;
    *) st_fail "an existing key was not reused: $out" ;;
  esac
  # Refused at export or at check_key, depending on what the agent has cached.
  st_refuses "wrong passphrase" "passphrase" 'wrong\n' cmd_package_signing

  # check_key alone: with the agent holding the passphrase, export succeeds
  # whatever was typed, and this is then the only check on the pair.
  armored="$(export_key "$fpr" pp-one)"
  check_key "$armored" pp-one || st_fail "check_key refused the right passphrase"
  if check_key "$armored" wrong; then st_fail "check_key accepted a wrong passphrase"; fi
  if check_key "not a key" pp-one; then st_fail "check_key accepted a non-key"; fi

  # The Maven pair goes to its own names.
  out="$(printf 'pp-one\n' | MAVEN_FPR="$fpr" cmd_maven_key 2>/dev/null)" || st_fail "maven-key failed"
  case "$out" in
    *"release/MAVEN_GPG_PRIVATE_KEY <- "*"release/MAVEN_GPG_PASSPHRASE <- 6 bytes"*) ;;
    *) st_fail "maven-key did not reach both uploads: $out" ;;
  esac
  wrote="$wrote
$out"
  st_refuses "maven-key empty passphrase" "empty passphrase" '\n' cmd_maven_key

  # A token is sent byte for byte, and an empty one is refused.
  out="$(printf 'tok-123\n' | cmd_token release SOME_NAME 2>/dev/null)" || st_fail "token failed"
  [ "$out" = "  DRY_RUN: release/SOME_NAME <- 7 bytes" ] || st_fail "token: $out"
  st_refuses "empty token" "empty value" '\n' cmd_token release SOME_NAME

  # Rotation: one distinct key per entry.
  out="$(cmd_rotate_deploy_keys)" || st_fail "rotation failed"
  n="$(printf '%s\n' "$out" | grep -c 'DRY_RUN: would register SHA256:')"
  [ "$n" = 3 ] || st_fail "rotation registered $n keys, expected 3"
  n="$(printf '%s\n' "$out" | awk '/would register/ { print $4 }' | sort -u | wc -l | tr -d ' ')"
  [ "$n" = 3 ] || st_fail "rotation reused a key across repositories"
  wrote="$wrote
$out"

  # Every name the commands above wrote is one a workflow job reads in that
  # environment. Taken from their output, not from a second list of names.
  names="$(workflow_secrets)"
  n=0
  for entry in $(printf '%s\n' "$wrote" | sed -n 's|^  DRY_RUN: \([a-z-]*\)/\([A-Z0-9_]*\) <- .*|\1:\2|p' | sort -u); do
    printf '%s\n' "$names" | grep -qx -- "$(printf '%s\t%s' "${entry%%:*}" "${entry#*:}")" ||
      st_fail "no workflow job in environment ${entry%%:*} reads ${entry#*:}"
    n=$((n + 1))
  done
  [ "$n" = 7 ] || st_fail "checked $n written names against the workflows, expected 7"
  if printf '%s\n' "$names" | grep -q 'GITHUB_TOKEN'; then st_fail "GITHUB_TOKEN listed as a stored secret"; fi

  # A rotation by name touches that key alone; an unknown name is refused.
  last=""
  for entry in $DEPLOY_KEYS; do last="${entry#*:}"; done
  last="${last%%:*}"
  out="$(cmd_rotate_deploy_keys "$last")" || st_fail "a named rotation failed"
  n="$(printf '%s\n' "$out" | grep -c ' <- SHA256:')"
  [ "$n" = 1 ] || st_fail "a named rotation set $n secrets, expected 1"
  case "$out" in
    *"/$last <- "*) ;;
    *) st_fail "a named rotation set another secret: $out" ;;
  esac
  st_refuses "unknown deploy key" "is not a deploy-key secret" '' cmd_rotate_deploy_keys NOT_A_KEY

  echo "release_secrets.sh --self-test: all checks passed"
}

case "${1:-}" in
  status) cmd_status ;;
  package-signing) cmd_package_signing ;;
  maven-key) cmd_maven_key ;;
  token) shift; cmd_token "$@" ;;
  rotate-deploy-keys) shift; cmd_rotate_deploy_keys "$@" ;;
  --self-test) cmd_self_test ;;
  *)
    sed -n '4,15p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
