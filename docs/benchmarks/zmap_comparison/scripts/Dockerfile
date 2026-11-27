# zmap x libJudy evaluation image. Build-time network only; every run uses
# `docker run --network none`, so no container has an interface beyond lo.
FROM rust:1.98-bookworm AS base
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential cmake libgmp3-dev gengetopt libpcap-dev flex byacc \
      libjson-c-dev pkg-config libunistring-dev iproute2 python3 time procps \
      binutils && rm -rf /var/lib/apt/lists/*

# ---- Expanse: libexpanse from the recorded commit, release profile --------
# `expanse` is the measured commit (arm EXPANSE). `expanse_base` is the commit
# it is compared with (arm MAIN, /opt/main); a record with no such comparison
# passes the same tree for both and runs the default two arms.
FROM base AS expanse
COPY expanse /src/expanse
COPY expanse_base /src/main
RUN for v in expanse main; do cd /src/$v && cargo build --release -p expanse-capi \
 && mkdir -p /opt/$v/lib /opt/$v/include /opt/$v/compat \
 && cp target/release/libexpanse.so /opt/$v/lib/ \
 && ln -s libexpanse.so /opt/$v/lib/libJudy.so \
 && ln -s ../lib/libexpanse.so /opt/$v/compat/libJudy.so.1 \
 && cp include/Judy.h /opt/$v/include/ \
 && rm -rf target || exit 1; done

# ---- zmap + harness against Expanse. No stock Judy is installed in this stage.
FROM base AS zmap-expanse
COPY --from=expanse /opt/expanse /opt/expanse
COPY --from=expanse /opt/main /opt/main
COPY zmap /src/zmap
COPY harness /src/harness
RUN test ! -e /usr/include/Judy.h \
 && for v in expanse main; do cmake -S /src/zmap -B /build/$v \
      -DCMAKE_LIBRARY_PATH=/opt/$v/lib \
      -DCMAKE_C_FLAGS="-I/opt/$v/include" \
      -DCMAKE_EXE_LINKER_FLAGS="-L/opt/$v/lib -Wl,-rpath,/opt/$v/lib" \
 && make -C /build/$v -j8 zmap 2>&1 | tail -5 \
 && gcc -O2 -std=gnu11 -I/opt/$v/include -I/src/zmap/lib \
      /src/harness/cachehash_bench.c /src/zmap/lib/cachehash.c \
      -L/opt/$v/lib -Wl,-rpath,/opt/$v/lib -lJudy \
      -o /build/$v/cachehash_bench || exit 1; done

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
COPY --from=zmap-expanse /opt/main /opt/main
COPY --from=zmap-expanse /build/main /build/main
COPY scripts /scripts
