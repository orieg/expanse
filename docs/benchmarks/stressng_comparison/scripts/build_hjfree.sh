#!/usr/bin/env bash
# ==============================================================================
# The leak intervention's two stress-ng builds (README.md, "The hashjudy cells
# on main"), each build.sh's stress-ng build from a copy of the same checkout
# of the frozen commit, in one container run:
#   b-hjctl   the source unchanged (the control: the same toolchain build);
#   b-hjfree  one change, a `JLFA` on each array in `hashjudy_destroy` after
#             the `JLMU` stress-ng already calls there.
# Nothing else in the source, the image, the compiler or the flags differs
# between the two. Untimed; it takes no bench lock of its own, so run it under
# one.
#
#   build_hjfree.sh
# ==============================================================================
set -euo pipefail
SNG_D="${SNG_D:-/var/tmp/stressng-eval}"
STRESS_NG_COMMIT=d5f8133a0cc5d2fa91695c6ac5c28f5cc4a3bdf5
cd "${SNG_D}"
[ "$(git -C stressng-src/stress-ng rev-parse HEAD)" = "${STRESS_NG_COMMIT}" ] \
  || { echo "stressng-src/stress-ng is not at ${STRESS_NG_COMMIT}" >&2; exit 1; }
[ -z "$(git -C stressng-src/stress-ng status --porcelain --untracked-files=no)" ] \
  || { echo "stressng-src/stress-ng has local changes" >&2; exit 1; }
rm -rf b-hjctl b-hjfree
git -C stressng-src/stress-ng archive --prefix=b-hjctl/ HEAD | tar -x
git -C stressng-src/stress-ng archive --prefix=b-hjfree/ HEAD | tar -x
# The one edit: the line must occur exactly once, inside hashjudy_destroy.
python3 - b-hjfree/stress-sparsematrix.c <<'PY'
import re, sys
path = sys.argv[1]
src = open(path).read()
needle = "\t\tJLMU(ret, table->hash_table[i]);\n"
if src.count(needle) != 1:
    sys.exit(f"expected the hashjudy_destroy JLMU line once, found {src.count(needle)}")
fn = re.search(r"static void hashjudy_destroy\(.*?\n}\n", src, re.S)
if not fn or needle not in fn.group(0):
    sys.exit("the JLMU line is not inside hashjudy_destroy")
added = "\t\t{\n\t\t\tWord_t freed;\n\n\t\t\tJLFA(freed, table->hash_table[i]);\n\t\t\t(void)freed;\n\t\t}\n"
open(path, "w").write(src.replace(needle, needle + added))
PY
# Exactly the six added lines, in that one file, and nothing removed.
d="$(diff -r b-hjctl b-hjfree | grep -c '^>' || true)"
r="$(diff -r b-hjctl b-hjfree | grep -c '^<' || true)"
f="$(diff -rq b-hjctl b-hjfree | wc -l)"
[ "${d}" = 6 ] && [ "${r}" = 0 ] && [ "${f}" = 1 ] \
  || { echo "the intervention is not the six-line JLFA edit (added ${d}, removed ${r}, files ${f})" >&2; exit 1; }
diff -r b-hjctl b-hjfree || true
docker run --rm -v "${SNG_D}:/w" -w /w -e HOST_UID="$(id -u)" ubuntu:22.04 bash -c '
  apt-get update -qq >/dev/null && apt-get install -y -qq build-essential >/dev/null 2>&1
  gcc --version | head -1 > build-hj-gcc.txt
  for b in b-hjctl b-hjfree; do
    make -C "$b" -j"$(nproc)" CFLAGS=-I/w/stock/usr/include LDFLAGS=-L/w/stock/usr/lib/x86_64-linux-gnu \
      > "build-$b.log" 2>&1
  done
  chown -R "$HOST_UID:$HOST_UID" /w/b-hjctl /w/b-hjfree /w/build-b-hjctl.log /w/build-b-hjfree.log /w/build-hj-gcc.txt'
cat build-hj-gcc.txt
for b in b-hjctl b-hjfree; do
  readelf -d "${b}/stress-ng" | grep -q 'NEEDED.*libJudy.so.1' \
    || { echo "${b}/stress-ng was not linked against libJudy.so.1" >&2; exit 1; }
done
sha256sum b-stockbuild/stress-ng b-hjctl/stress-ng b-hjfree/stress-ng
