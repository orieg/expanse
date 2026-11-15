# zmap x libJudy evaluation image. Build-time network only; every run uses
# `docker run --network none`, so no container has an interface beyond lo.
FROM rust:1.98-bookworm AS base
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential cmake libgmp3-dev gengetopt libpcap-dev flex byacc \
      libjson-c-dev pkg-config libunistring-dev iproute2 python3 time procps \
      binutils && rm -rf /var/lib/apt/lists/*

# ---- Expanse: libexpanse from the recorded commit, release profile --------
FROM base AS expanse
COPY expanse /src/expanse
RUN cd /src/expanse && cargo build --release -p expanse-capi \
 && mkdir -p /opt/expanse/lib /opt/expanse/include /opt/expanse/compat \
 && cp target/release/libexpanse.so /opt/expanse/lib/ \
 && ln -s libexpanse.so /opt/expanse/lib/libJudy.so \
 && ln -s ../lib/libexpanse.so /opt/expanse/compat/libJudy.so.1 \
 && cp include/Judy.h /opt/expanse/include/ \
 && rm -rf target

# ---- zmap + harness against Expanse. No stock Judy is installed in this stage.
FROM base AS zmap-expanse
COPY --from=expanse /opt/expanse /opt/expanse
COPY zmap /src/zmap
COPY harness /src/harness
RUN test ! -e /usr/include/Judy.h \
 && cmake -S /src/zmap -B /build/expanse \
      -DCMAKE_LIBRARY_PATH=/opt/expanse/lib \
      -DCMAKE_C_FLAGS="-I/opt/expanse/include" \
      -DCMAKE_EXE_LINKER_FLAGS="-L/opt/expanse/lib -Wl,-rpath,/opt/expanse/lib" \
 && make -C /build/expanse -j8 zmap 2>&1 | tail -5 \
 && gcc -O2 -std=gnu11 -I/opt/expanse/include -I/src/zmap/lib \
      /src/harness/cachehash_bench.c /src/zmap/lib/cachehash.c \
      -L/opt/expanse/lib -Wl,-rpath,/opt/expanse/lib -lJudy \
      -o /build/expanse/cachehash_bench

# ---- final: stock libJudy from the distro package, plus the Expanse builds
FROM base
RUN apt-get update && apt-get install -y --no-install-recommends libjudy-dev \
 && rm -rf /var/lib/apt/lists/*
COPY zmap /src/zmap
COPY harness /src/harness
RUN cmake -S /src/zmap -B /build/stock \
 && make -C /build/stock -j8 zmap 2>&1 | tail -5 \
 && gcc -O2 -std=gnu11 -I/src/zmap/lib \
      /src/harness/cachehash_bench.c /src/zmap/lib/cachehash.c -lJudy \
      -o /build/stock/cachehash_bench
COPY --from=zmap-expanse /opt/expanse /opt/expanse
COPY --from=zmap-expanse /build/expanse /build/expanse
COPY scripts /scripts
