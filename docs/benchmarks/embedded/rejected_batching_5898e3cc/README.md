# The rejected BLE eviction batching: the paired harvests behind §8.1.6

`docs/design/32-bit-embedded.md` §8.1.6 rejects the TTL-eviction slab batching
that #628 added and #641 reverted. These are the runs that decided it, kept so
every number in that section resolves to an artifact (AGENTS.md §8.7).

| file | engine | `expanse_ble_tracker.c` | on-device version line |
|---|---|---|---|
| `ctl_no_batching.json` | current | pre-#628, no batching | `v0.5.0-100-g4733cbe2-dirty` |
| `trt_batching.json` | current, byte-identical to control | #628's 64-entry stack batch | `v0.5.0-100-g4733cbe2-dirty` |
| `pub_reverted_published.json` | current | reverted (what `main` ships) | `v0.5.0-104-g41080e96` |

The control and treatment differ in that one file and nothing else: the engine
is byte-identical between them and to the published artifact, so the pairing
has a single variable. `pair.py` produced `pairing.txt` from the first two;
`pub_reverted_published.json` is the same file as the suite's committed
`results/esp32.json` and is duplicated here only so the three links can be
compared without leaving this directory.

**Read the two pop=500 eviction arms as layout, not as a result.** They land
within 1% of the two discrete values §8.1.3 records for byte-identical engine
source, control on the upper mode and treatment on the lower. The verdict rests
on the pop=2000 arms, where layout does not dominate.

**These three links are also the evidence for §8.1.3's determinism claim.**
`pub_reverted_published.json` and the previously published artifact were built
from identical engine and component source with equal-length version strings
(20 characters each) and agree bit-identically on all 17 arms;
`ctl_no_batching.json` is that same source with a six-character-longer string
and moves arms by up to 0.9%.
