# gtkwave × JudySL/JudyHS/Judy1: libexpanse as a drop-in for the waveform viewer

GTKWave 3.3.116 (Ubuntu 24.04's `gtkwave` 3.3.116-1build2 source, configured as its `debian/rules` does, `--enable-gtk3 --enable-judy`) uses libJudy in three places when `_WAVE_HAVE_JUDY` is defined. `vcd2fst` keys VCD scope names in a JudySL array (`src/helpers/vcd2fst.c`). The FST writer in `src/helpers/fst/fstapi.c` hashes every signal's compressed value-change block in a JudyHS array to find dynamic aliases, and keys source-stem paths in another. The viewer itself keeps its symbol table and facility tree in JudySL and a selection set in Judy1 (`src/symbol.c`, `src/tree.c`). This suite builds that source three ways and compares what the project's own tools produce. It is the evaluation record behind the gtkwave row of the top-level README's [Verified in real projects](../../../README.md#verified-in-real-projects) table.

This is a **local evaluation**, run on a laptop in a container. It has no pre-registration, no wall-clock measurement and no `/benchmark` token: its harness builds gtkwave and its GTK 3 dependencies in a container, which the bare-metal workflow does not provide. No independent, non-LLM review of it has been done.

## Candidate selection

The candidates, in preference order, were gtkwave, php-memprof, SaunaFS and OpenLI. gtkwave was taken first, built against the distro libJudy in an Ubuntu 24.04 container from `apt-get source gtkwave` with its declared `Build-Depends`, and was kept. The other three were not attempted, so this record says nothing about them.

## Arms

| Arm | Build | Library mapped at run time |
|---|---|---|
| A | stock: `/usr/include/Judy.h`, `-lJudy` (Ubuntu 24.04 `libjudy-dev` 1.0.5-5.1build1) | `libJudy.so.1` → `libJudy.so.1.0.3` |
| B | the same source rebuilt with `-I include/` (this repository's `Judy.h`) and `-lJudy` resolving to `libexpanse.so.1` through a development link | `libexpanse.so.1` |
| B1 | A's binaries, unchanged, with `LD_LIBRARY_PATH` pointing at `libJudy.so.1 → libexpanse.so.1`, the `libjudy-compat` link the release workflow ships | `libexpanse.so.1` |

[`build_evidence.txt`](results/local_m1_4af9e927/build_evidence.txt) records which `Judy.h` each include path resolves, the library each arm's `vcd2fst`, `fst2vcd` and `gtkwave` binaries map under `ldd`, that both builds compiled with `-D_WAVE_HAVE_JUDY`, and that neither build log carries a warning mentioning Judy (13 compiler warnings in each, the same count). libexpanse was built with `EXPANSE_LINK_SONAME=1`, as the release build is.

## Record

| Record | libexpanse | Host | What it holds |
|---|---|---|---|
| [`results/local_m1_4af9e927/`](results/local_m1_4af9e927/) | `4af9e927` | Apple M1 (4P+4E), 16 GB, Docker Desktop Linux VM with 4 vCPUs, Ubuntu 24.04 aarch64 container | the test-suite runs, the conversion and viewer outputs' hashes per arm, twelve Callgrind profiles, and peak RSS per process |

- `tests/`: `make check` in both builds.
- `corr/compare.txt`: the sha256 of every converted output in each arm and whether the three agree; `inputs.sha256` the four inputs; `stock_vs_stock.txt` whether two stock conversions of one input are byte-identical (they are, so a cross-arm byte difference would be a real one).
- `gui/`: the viewer's facility listings, their hashes per arm (`compare.txt`) and exit codes (`rc.txt`); the `des` and `transaction` listings are committed, the 32,803-line `syn_large` listing by hash only.
- `callgrind/*.out`: the raw profiles. `rss/*`: one `ru_maxrss` per (workload, arm). `summary.txt`: what `scripts/summarize.py` prints over all of them. `load.txt`: the container's load average at the start, before the Callgrind phase and at the end. `toolchain.txt`: package and compiler versions.

## Results (`4af9e927`)

*(measured: Apple M1 4P+4E, 16 GB, Docker Desktop Linux VM with 4 vCPUs, Ubuntu 24.04 aarch64 container, libexpanse `4af9e927`, gtkwave 3.3.116-1build2; [`results/local_m1_4af9e927/`](results/local_m1_4af9e927/))*

**Test suite.** gtkwave 3.3.116 ships no automated tests. `make check` exits 0 in both builds and runs nothing: no `.trs` or `test-suite.log` is produced in either tree, and the logs contain only directory traversal (`tests/`). The project's own exercise is therefore its tools, below.

**Correctness.** Every output is byte-identical across A, B and B1:

- `fst2vcd` on the project's example dumps `examples/des.fst` and `examples/transaction.fst`, `vcd2fst` back to FST, and `fst2vcd` again: six outputs.
- `vcd2fst` on two synthetic VCDs from `scripts/gen_vcd.py` (4,096 signals over 2,000 timesteps, and 32,768 signals over 500, three scope levels, each signal given, with probability 0.3 when the widths match, a copy of an earlier signal's waveform so the writer's JudyHS alias search hits, and an identifier re-declared under a second scope every 16 scopes), in the default mode and in `-c` mode, each followed by `fst2vcd`: eight outputs.
- the viewer, `gtkwave -S` under Xvfb, loading `des.fst`, `transaction.fst` and the large synthetic FST, listing every facility through the Tcl API and looking up a present and an absent signal: three listings of 1,436, 5 and 32,803 lines, every run exiting 0.

14 of 14 converted outputs and 3 of 3 listings identical (`corr/compare.txt`, `gui/compare.txt`).

**Instructions and memory** (workloads: `gtkwave_vcd2fst_syn_small`, `gtkwave_vcd2fst_syn_large`, `gtkwave_fst2vcd_syn_small`, `gtkwave_fst2vcd_syn_large`). Callgrind counts each process's instructions exactly; the per-object columns are self counts, which add up to the process total (`summarize.py` checks that). Ratios are libexpanse ÷ stock; below 1 is fewer.

| Workload | Arm | Process Ir | In the Judy library | In libc | Peak RSS |
|---|---|---:|---:|---:|---:|
| `gtkwave_vcd2fst_syn_small` | B | 0.9906 | 0.4378 | 1.0111 | 1.0658 |
| | B1 | 0.9906 | 0.4377 | 1.0111 | 1.0825 |
| `gtkwave_vcd2fst_syn_large` | B | 1.0002 | 0.6239 | 1.0337 | 1.1298 |
| | B1 | 1.0002 | 0.6238 | 1.0337 | 1.1300 |
| `gtkwave_fst2vcd_syn_small` | B | 1.0002 | — | 1.0000 | 1.0780 |
| | B1 | 1.0002 | — | 1.0000 | 1.0758 |
| `gtkwave_fst2vcd_syn_large` | B | 1.0001 | — | 1.0000 | 1.0371 |
| | B1 | 1.0001 | — | 1.0000 | 1.0515 |

In absolute terms (workload: `gtkwave_vcd2fst_syn_large`, arm B), the instructions executed inside the Judy library fell by 19,091,665, from 50,764,430 to 31,672,765, and those executed in libc rose by 19,396,987, from 575,783,126 to 595,180,113, so the process executes 344,595 more (1,488,197,660 to 1,488,542,255). On `gtkwave_vcd2fst_syn_small` the library's count fell by 7,667,635 and libc's rose by 2,434,208, a net 5,194,154 fewer. The libc increase is in `malloc`, `free` and their internals (`_int_malloc`, `_int_free`, `malloc_consolidate`) in the per-function profiles; gtkwave's own code executes the same count in every arm (689,368,582 and 313,140,403). `fst2vcd` executes almost no Judy code (57 instructions in libJudy, 291 in libexpanse); its process difference, 38,153 more instructions on each input, is almost all outside gtkwave's code, the Judy library and libc, in the other objects the profile lists (the loader among them).

**Where libexpanse is worse.**

- **Peak RSS is higher in every cell**: +1,008 and +1,264 kB on `vcd2fst` small, +3,656 and +3,660 kB on `vcd2fst` large (28,156 kB to 31,812 and 31,816 kB), and +256 to +356 kB on `fst2vcd`, which executes almost no Judy code. The `fst2vcd` cells set a floor for the part that is not data-structure footprint: `libexpanse.so.1` is 2,694,888 B against `libJudy.so.1.0.3`'s 329,528 B. How the rest of the `vcd2fst` difference splits between that and the arrays is unmeasured (no allocator census was taken).
- **On the large conversion libexpanse's allocator calls cost more than its engine saves**, by 344,595 instructions in 1.49 billion (1.0002×). The engine executes fewer instructions than stock's on both conversions; the allocator work it adds in libc is larger than that saving on the large one and smaller on the small one. Which of the three Judy uses the added allocations come from is not attributed here. The large profile's second-largest libexpanse function by self count is `DefaultHasher::write`, 7,514,993 instructions; the JudyHS short-key path's per-entry allocation and SipHash cost are tracked in [#1681](https://github.com/orieg/expanse/issues/1681).

**Where libexpanse is better.** Inside the library, 0.44× and 0.62× the instructions of stock on the two conversions, and 0.9906× for the whole small conversion. A whole-process instruction ratio is not a time ratio: no wall clock was measured.

**Disclosure.** Local wall clock is not a publishable claim (AGENTS.md §8.4), and none is reported. The host was heavily shared during the run: the container's load average was 16.1–20.5 at the start, 18.3–19.7 before the Callgrind phase and 10.4–14.2 at the end on 4 vCPUs (`load.txt`). Callgrind's counts do not depend on load; peak RSS is one process per cell. These are aarch64 counts; an x86-64 run may differ.

## Reproducing

In an Ubuntu 24.04 container with deb-src enabled, a work directory `/w`, this repository at `/w/expanse` and the scripts at `/w/scripts`: install `build-essential libjudy-dev valgrind time python3 dpkg-dev xvfb xauth` and gtkwave's `Build-Depends` (`libgtk-3-dev zlib1g-dev libbz2-dev flex gperf tcl-dev tk-dev liblzma-dev`), and a Rust toolchain at or above the MSRV. Then:

```bash
bash /w/scripts/build.sh      # libexpanse, arms A and B, and build_evidence.txt
bash /w/scripts/run_eval.sh   # tests, correctness, viewer, Callgrind and RSS into /w/out
python3 /w/scripts/summarize.py /w/out
```

`scripts/gen_vcd.py` is deterministic: the committed `inputs.sha256` values were reproduced outside the container.
