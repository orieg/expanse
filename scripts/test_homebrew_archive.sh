#!/usr/bin/env bash
# Compile and run the Homebrew formula's two `test do` programs against a
# release archive, so the archive is proven installable before the formula
# that points at it is pushed.
#
# usage: test_homebrew_archive.sh <archive.tar.gz> <expected-version>
set -euo pipefail

archive="${1:?archive path}"
want="${2:?expected version}"
[ -f "${archive}" ] || { echo "error: archive not found: ${archive}" >&2; exit 1; }

work="$(mktemp -d)"
# Under bash 3.2 (the macOS system bash) a fatal shell error such as an unbound
# variable reaches the EXIT trap with $? = 0, so a cleanup trap turns a dead
# smoke into exit 0. The trap therefore trusts a flag set on the last line,
# never the status it is handed.
finished=0
trap 'rc=$?; rm -rf "${work}"; [ "${finished}" = 1 ] || { [ "${rc}" -ne 0 ] || rc=1; echo "error: smoke did not run to completion" >&2; }; exit "${rc}"' EXIT
tar -xzf "${archive}" -C "${work}"
for f in include/expanse.h include/Judy.h lib/libexpanse.a; do
  [ -f "${work}/${f}" ] || { echo "error: archive lacks ${f}" >&2; exit 1; }
done

# Extract the heredoc bodies from the formula template itself, so this script
# and `brew test` cannot drift apart.
template="$(dirname "$0")/../extra/homebrew/expanse.rb.in"
extract() { # <file written in the formula>
  awk -v name="$1" '
    index($0, "(testpath/\"" name "\").write") { on = 1; next }
    on && /^ *EOS$/ { exit }
    on { print }
  ' "${template}" | sed 's/\\\\n/\\n/'
}
extract modern.c > "${work}/modern.c"
extract legacy.c > "${work}/legacy.c"
[ -s "${work}/modern.c" ] && [ -s "${work}/legacy.c" ] || { echo "error: test programs not found in ${template}" >&2; exit 1; }

# Static link: the dylib's install name is the build directory until Homebrew
# or MacPorts rewrites it, and this check is about the headers and symbols.
# `${libs[@]+...}` below: bash 3.2 treats an empty array as unset under `set -u`.
libs=()
case "$(uname -s)" in
  Linux) libs=(-lpthread -ldl -lm) ;;
esac
cc "${work}/modern.c" -I"${work}/include" "${work}/lib/libexpanse.a" ${libs[@]+"${libs[@]}"} -o "${work}/modern"
cc "${work}/legacy.c" -I"${work}/include" "${work}/lib/libexpanse.a" ${libs[@]+"${libs[@]}"} -o "${work}/legacy"

got="$("${work}/modern")"
case "${got}" in
  *"${want}"*) ;;
  *) echo "error: expanse_version() printed '${got}', expected it to contain '${want}'" >&2; exit 1 ;;
esac
"${work}/legacy"
echo "homebrew archive smoke: ok (${got})"
finished=1
