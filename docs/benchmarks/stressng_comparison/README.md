# stress-ng × Judy: Expanse as a runtime libJudy replacement

stress-ng's `judy` and `sparsematrix` stressors run one binary against stock libJudy and against `libexpanse`. The library is swapped through `libJudy.so.1` and nothing else changes. This suite is item 4 of [#1666](https://github.com/orieg/expanse/issues/1666): the re-measure after the packaging fix (#1667), the Callgrind arms (#1668) and the documented small-array floor (#1669).

- **Pre-registration:** [`METHODOLOGY.md`](METHODOLOGY.md). It was committed before the first timed process of this measurement and is never edited to match outcomes.
- **Status:** pre-registered; the timed runs have not been taken. This file gains the results, their verdicts and the provenance of each artifact once they exist.
- **Review:** no independent, non-LLM review has been done. #1666 item 4 requires one before any statement to stress-ng or to distribution packagers.

## Reproduce

On a host with Docker and the workspace toolchain:

```bash
docs/benchmarks/stressng_comparison/scripts/build.sh
```

```bash
docs/benchmarks/stressng_comparison/run.sh 1 ref
```

```bash
docs/benchmarks/stressng_comparison/run.sh 2 ref
```

```bash
python3 docs/benchmarks/stressng_comparison/scripts/analyze.py ref /var/tmp/stressng-eval/results/ref_run1 /var/tmp/stressng-eval/results/ref_run2
```

`build.sh` is untimed. Each `run.sh` holds the host bench lock for its whole run and refuses to start on the conditions in METHODOLOGY.md §7. `analyze.py` writes `results/baseline_<host>_run{1,2}.json` and prints the verdict table.
