# Contributing to Expanse

Expanse has one maintainer and does not currently expect a stream of outside
contributions. Issues, bug reports, and pull requests are still welcome — this
page says what a useful one looks like so a drive-by contribution does not
bounce on a rule it had no way to know about.

## Before you write code

- **Open an issue first** for anything larger than a typo or an obvious fix.
  The engine's node ladder, encodings, and C ABI surface are tightly coupled;
  a patch that is correct in isolation can still be unmergeable. Use the
  [issue forms](.github/ISSUE_TEMPLATE) — they ask for exactly the evidence a
  triage needs.
- **Read [`AGENTS.md`](AGENTS.md).** It is the canonical engineering guide for
  humans and coding agents alike, and it is not optional decoration: §6.5 maps
  each rule to the CI job that enforces it. The parts that most often surprise
  a first-time contributor are §2 (architectural invariants), §3 (clean room),
  and §8 (benchmark and claim integrity).

## The clean-room rule (non-negotiable)

The original `libjudy` is LGPL. **Do not read, decompile, quote, or port its
source** — not to settle an edge case, not for inspiration. Compatibility
questions are answered from published documentation or by black-box differential
testing against a compiled stock binary. A pull request that shows evidence of
exposure to the original source cannot be merged, and neither can anything
derived from it. See `AGENTS.md` §3.

## Local gates

One command runs what CI's `lint` and `test` jobs run:

```bash
scripts/gate.sh
```

Add `--miri` when the change touches `unsafe` or node layout; add
`--with-bindings` if your toolchain has the PHP and Python prerequisites. A
green local gate is necessary, not sufficient — CI is the authority.

## Pull requests

- Branch from `main` (`feat/…`, `fix/…`, `perf/…`, `refactor/…`, `docs/…`).
  `main` is protected; everything lands through a pull request whose single
  required check is `CI Gate / All Checks Passed`.
- [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): description`,
  atomic, subject factual. No agent or session trailers.
- Fill in [the pull request template](.github/PULL_REQUEST_TEMPLATE.md) and tick
  only the boxes your diff actually supports. It is a checklist of evidence, not
  a formality — an unticked box with a sentence explaining why beats a ticked
  one that the diff contradicts.
- Deleting tracked files requires a `removes:` or `deletes:` line in the body;
  a CI script enforces it.

## Performance and benchmark claims

Expanse is an empirical project, and the bar for a number is high:

- The regression instrument is Callgrind instruction counts, gated by
  `scripts/perf_report.py` on the `instruction-counts` job. Anything the report
  flags needs a justification in the body.
- Every published number carries a `(measured: host, commit)` tag that resolves
  to a committed artifact, and wall-clock claims carry a BCa 95 % bootstrap
  confidence interval. Losing cells get published alongside winning ones.
- Never hand-write, estimate, or infer a benchmark result. A missing measurement
  is stated as missing.
- No time estimates anywhere — in issues, pull requests, code comments, or
  documentation. State ordering, dependencies, and gate criteria instead. A CI
  script enforces this one too.

## Privacy in the repository

No personal hostnames, LAN addresses, internal domains, home-directory paths, or
OS usernames in commits, issues, pull requests, comments, or docs. Benchmark
hosts are described by hardware (CPU model, cores, cache, OS), never by name.

## Security

Do not report a suspected vulnerability in a public issue. See
[`SECURITY.md`](SECURITY.md) for the private channel and the threat model that
says what counts as one.

## Licence

Contributions are licensed under MIT OR Apache-2.0, matching the project.
