# Security Policy

## Reporting a vulnerability

Report privately through GitHub's **Security → Report a vulnerability** form on
this repository. That opens a private advisory visible only to the maintainers,
which is the right channel for anything you would not want in a public issue.

Please do not open a public issue for a suspected vulnerability first. Public
issues are the right place for a crash you can reproduce from safe input, a
build failure, or a question about the threat model below.

A useful report names the entry point (a Rust API, a `expanse_*` or `Judy*` C
symbol, or a language binding), the input that triggers the behaviour, and what
you observed. A reproducing test case or a fuzz artifact is worth more than a
description. If you have a proposed fix, say so and it will be reviewed with the
report rather than after it.

Reports are acknowledged, triaged, and — where a fix is warranted — released
with a credit line unless you ask otherwise. Progress is reported on the private
advisory as it happens rather than against a published schedule.

## Supported versions

Fixes land on `main` and ship in the next release. Only the latest released
version is supported; there are no maintained release branches.

## Threat model

Expanse is an in-process data-structure library with no network surface, no
process isolation, and no privilege boundary of its own. It inherits the trust
domain of the process that links it. What follows is where that trust boundary
actually sits, so a reporter can tell a bug from an expected contract.

**Treated as untrusted input — a memory-safety failure here is a vulnerability:**

- **Binary images.** `ExpanseBlobMap::load_from_file` / `from_bytes_slice`
  (`crates/expanse/src/blobmap.rs`) parse a serialized image whose every header
  field is attacker-controlled. Out-of-bounds access, an unchecked allocation
  size, an integer overflow reaching an offset, or a panic on a malformed image
  is in scope. This path is fuzzed (`fuzz/fuzz_targets/blobmap_image_corrupt.rs`).
- **Keys and values through any public API.** Arbitrary byte strings, arbitrary
  key distributions, and arbitrary insert/remove/scan orderings are all expected
  input. A crash, a use-after-free, or a wrong answer from any sequence of safe
  Rust API calls is in scope.
- **Concurrent access through the `Sync*` wrappers.** A data race, a torn read,
  or a use-after-free reachable from the documented reader/writer protocol is in
  scope.

**Caller contracts — a failure here is the caller's, not a vulnerability:**

- **C ABI pointer and length arguments.** `libexpanse` is a C library. A
  `(ptr, len)` argument that does not describe `len` readable bytes, a
  non-NUL-terminated string passed to a `JudySL*` or `expanse_strmap_*` entry
  point, or a handle used after it was freed, is undefined behaviour by the same
  contract stock `libjudy` carries. `include/expanse.h` and `docs/COMPAT.md`
  state each function's contract.
- **Pointers documented as valid only until the next structural mutation.**
  `expanse_map_slot`, `expanse_map_ins_slot` and their `strmap`/`bytesmap` twins
  return a live pointer into the trie's own storage. Retaining one across an
  insert or remove is a use-after-free the library cannot prevent. Where a
  payload must be decoded, use the caller-owned destination form
  (`expanse_blob_map_get_into`) instead.
- **`unsafe` Rust.** The `unsafe` entry points document their preconditions in a
  `# Safety` section. Violating one is out of scope.

**Known and documented, not a vulnerability report:**

- **`no_std` hash-flooding.** `ExpanseBytesMap` defaults to a fixed-basis FNV-1a
  hasher in `no_std` builds, because `std::hash::RandomState`'s per-process seed
  is unavailable there. With attacker-controlled keys in a `no_std` deployment,
  supply a seeded or cryptographically secure `BuildHasher` via `with_hasher`.
  This is a documented property of the build configuration, not a defect.
- **Resource exhaustion proportional to input.** Inserting many keys uses memory
  proportional to the populated key expanse. That is the data structure working.

## Dependencies

`expanse-trie` has no runtime dependencies. Everything in the dependency graph
arrives through dev-dependencies or a binding crate's own toolchain, and the
graph is gated on every pull request by `cargo deny` (advisories, licences,
sources, and wildcard version requirements) against the policy in `deny.toml`.
