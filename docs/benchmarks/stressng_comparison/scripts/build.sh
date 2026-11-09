#!/usr/bin/env bash
# Builds the inputs of METHODOLOGY.md §2 into $SNG_D (default /var/tmp/stressng-eval):
#   stock/                 the jammy libjudy packages, extracted (no install)
#   b-stockbuild/stress-ng stress-ng d5f8133a built against the distro Judy.h
#   expcompat/lib          libexpanse from this checkout, in the libjudy-compat layout
# Untimed. Run it outside any timed window; it takes no bench lock.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
SNG_D="${SNG_D:-/var/tmp/stressng-eval}"
STRESS_NG_COMMIT=d5f8133a0cc5d2fa91695c6ac5c28f5cc4a3bdf5
JAMMY=http://archive.ubuntu.com/ubuntu/pool/universe/j/judy
mkdir -p "${SNG_D}/deb" "${SNG_D}/stock"
cd "${SNG_D}"

# Stock libJudy: the two jammy packages, extracted.
for d in libjudydebian1_1.0.5-5_amd64.deb libjudy-dev_1.0.5-5_amd64.deb; do
  [ -f "deb/${d}" ] || curl -fsSL -o "deb/${d}" "${JAMMY}/${d}"
  dpkg-deb -x "deb/${d}" stock
done

# stress-ng at the frozen commit, built in ubuntu:22.04 against the distro header.
[ -d stressng-src/stress-ng ] || git clone -q https://github.com/ColinIanKing/stress-ng stressng-src/stress-ng
git -C stressng-src/stress-ng checkout -q "${STRESS_NG_COMMIT}"
rm -rf b-stockbuild && cp -a stressng-src/stress-ng b-stockbuild
docker run --rm -v "${SNG_D}:/w" -w /w/b-stockbuild -e HOST_UID="$(id -u)" ubuntu:22.04 bash -c '
  apt-get update -qq >/dev/null && apt-get install -y -qq build-essential >/dev/null 2>&1
  make -j"$(nproc)" CFLAGS=-I/w/stock/usr/include LDFLAGS=-L/w/stock/usr/lib/x86_64-linux-gnu \
    > ../build-stockbuild.log 2>&1
  chown -R "$HOST_UID:$HOST_UID" /w/b-stockbuild /w/build-stockbuild.log'
readelf -d b-stockbuild/stress-ng | grep -q 'NEEDED.*libJudy.so.1' \
  || { echo "stress-ng was not linked against libJudy.so.1" >&2; exit 1; }

# Expanse, with the SONAME the release job sets, in the libjudy-compat layout.
export PATH="${HOME}/.cargo/bin:${PATH}"
(cd "${REPO_ROOT}" && EXPANSE_LINK_SONAME=1 CARGO_TARGET_DIR="${SNG_D}/expanse-target" \
  cargo build -q --release -p expanse-capi)
rm -rf expcompat && mkdir -p expcompat/lib
cp "${SNG_D}/expanse-target/release/libexpanse.so" expcompat/lib/libexpanse.so.1.0.0
ln -s libexpanse.so.1.0.0 expcompat/lib/libexpanse.so.1
ln -s libexpanse.so.1 expcompat/lib/libexpanse.so
ln -s libexpanse.so.1 expcompat/lib/libJudy.so.1
readelf -d expcompat/lib/libexpanse.so.1.0.0 | grep -q 'SONAME.*libexpanse.so.1' \
  || { echo "libexpanse has no DT_SONAME libexpanse.so.1" >&2; exit 1; }
sha256sum b-stockbuild/stress-ng stock/usr/lib/x86_64-linux-gnu/libJudy.so.1.0.3 expcompat/lib/libexpanse.so.1.0.0
