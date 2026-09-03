#!/usr/bin/env python3
"""scripts/set_domain_bounds.py — Mathematical bounds and memory derivations
for the interned set domain (Issue #611).

Enforces Rule 12 / GEMINI.md §1.3 (Math-first validation in committed Python
with reference-pinned unit tests) and AGENTS.md §8.8 (Commit 1 Step 0).
"""

from __future__ import annotations

import unittest


def escape_encode_bytes(data: bytes) -> bytes:
    """Order-preserving escape encoding for arbitrary byte slices into ExpanseStrMap.

    In ExpanseStrMap (strmap.rs:218, 233), 8-byte chunks terminate when a 0x00
    byte is encountered. In release builds, strmap.rs:673 debug_assert! is
    compiled out, so embedded 0x00 bytes silently truncate the key.

    To support arbitrary byte slices (including 16-byte binary UUIDs, where
    P(>=1 NUL) ≈ 1.000 across real corpora), this encoding transforms:
        0x00 -> 0x01 0x01
        0x01 -> 0x01 0x02
        b    -> b (for b in 0x02..=0xFF)

    Properties:
        1. Encoded output contains strictly ZERO 0x00 bytes.
        2. Strictly preserves byte-lexicographical sort order:
           a < b <=> escape_encode_bytes(a) < escape_encode_bytes(b).
        3. Inverse is unique and deterministic.
    """
    out = bytearray()
    for b in data:
        if b == 0:
            out.append(1)
            out.append(1)
        elif b == 1:
            out.append(1)
            out.append(2)
        else:
            out.append(b)
    return bytes(out)


def escape_decode_bytes(encoded: bytes) -> bytes:
    """Inverse of escape_encode_bytes."""
    out = bytearray()
    i = 0
    n = len(encoded)
    while i < n:
        b = encoded[i]
        if b == 1:
            i += 1
            if i >= n:
                raise ValueError("truncated escape sequence")
            esc = encoded[i]
            if esc == 1:
                out.append(0)
            elif esc == 2:
                out.append(1)
            else:
                raise ValueError(f"invalid escape sequence: 0x01 0x{esc:02x}")
        elif b == 0:
            raise ValueError("encoded stream must not contain 0x00")
        else:
            out.append(b)
        i += 1
    return bytes(out)


def escape_encoded_length_bounds(raw_len: int, nul_count: int, one_count: int) -> int:
    """Computes exact encoded byte length under order-preserving escape encoding.

    Bounds:
        L <= L_enc <= 2 * L
    """
    if raw_len < 0 or nul_count < 0 or one_count < 0:
        raise ValueError("lengths and counts must be non-negative")
    if nul_count + one_count > raw_len:
        raise ValueError("escaped byte counts cannot exceed raw length")
    return raw_len + nul_count + one_count


def uniform_random_expected_inflation(raw_len: int) -> float:
    """Expected byte inflation for uniform random byte strings of length raw_len.

    Each byte has probability 2/256 of being 0x00 or 0x01:
        E[L_enc] = L * (1 + 2/256) = L * 1.0078125 (< 0.8% overhead).
    """
    if raw_len < 0:
        raise ValueError("raw_len must be non-negative")
    return raw_len * (1.0 + 2.0 / 256.0)


def uncompressed_hash_dict_bytes(key_count: int, avg_key_len: int) -> int:
    """Memory consumption of uncompressed hash-dictionary storage (e.g. ExpanseBytesMap).

    In ExpanseBytesMap (bytesmap.rs:76, 443), each entry allocates an
    uncompressed Box<[u8]> (16 bytes fat-pointer overhead + L bytes heap)
    plus 8 bytes u64 value, plus ExpanseMap word-trie index overhead (~16-24 B/entry):
        Mem_uncompressed(N, L) = N * (24 + L + IndexOverhead)
    """
    if key_count < 0 or avg_key_len < 0:
        raise ValueError("counts and lengths must be non-negative")
    entry_cost = 24 + avg_key_len + 16
    return key_count * entry_cost


def prefix_compressed_trie_bytes(
    key_count: int,
    avg_key_len: int,
    shared_prefix_len: int,
) -> int:
    """Memory consumption of prefix-compressed digital trie (ExpanseStrMap).

    In ExpanseStrMap (strmap.rs §4):
    Keys sharing an 8-byte prefix chunk share StrNode trie branches.
    Only the diverging suffix is allocated in StrSuffix.

    For N keys with a shared prefix of P bytes:
        Shared chunks = floor(P / 8)
        Remaining suffix = avg_key_len - P
    """
    if key_count < 0 or avg_key_len < 0 or shared_prefix_len < 0:
        raise ValueError("parameters must be non-negative")
    if shared_prefix_len > avg_key_len:
        raise ValueError("shared prefix cannot exceed total key length")

    shared_chunks = shared_prefix_len // 8
    shared_node_cost = shared_chunks * 88

    unshared_len = avg_key_len - shared_prefix_len
    per_key_suffix_cost = 16 + unshared_len + 16
    return shared_node_cost + (key_count * per_key_suffix_cost)


def offset_32bit_capacity_limit() -> int:
    """Maximum addressable bytes in a 32-bit flat offset table (Vec<u32>).

    Limit: 2^32 - 1 bytes = 4,294,967,295 B (~4 GiB).
    Any payload beyond this truncates silently in release mode if cast to u32.
    """
    return (1 << 32) - 1


def max_keys_before_32bit_overflow(avg_key_len: int) -> int:
    """Number of keys of average length avg_key_len that exhaust 32-bit offset capacity."""
    if avg_key_len <= 0:
        raise ValueError("avg_key_len must be positive")
    return offset_32bit_capacity_limit() // avg_key_len


def blob_arena_64bit_capacity_limit() -> int:
    """Maximum addressable bytes in BlobArena using 64-bit global offsets.

    Uses GlobalOffset (chunk_idx: u32, chunk_offset: u32):
        2^32 chunks * 1 GiB max chunk = 4 EiB.
    """
    return (1 << 32) * (1 << 30)


class TestSetDomainBounds(unittest.TestCase):
    def test_escape_encoding_properties(self):
        cases = [
            b"",
            b"hello world",
            b"\x00",
            b"\x01",
            b"\x00\x01\x02",
            b"prefix\x00suffix",
            bytes(range(256)),
        ]
        for raw in cases:
            enc = escape_encode_bytes(raw)
            self.assertNotIn(0, enc, f"Encoded {raw!r} contained 0x00 byte!")
            dec = escape_decode_bytes(enc)
            self.assertEqual(raw, dec, f"Round-trip failed for {raw!r}")

        for raw in cases:
            enc = escape_encode_bytes(raw)
            self.assertTrue(len(raw) <= len(enc) <= 2 * len(raw))

    def test_escape_encoding_lexicographical_order_preservation(self):
        keys = [
            b"apple",
            b"apple\x00",
            b"apple\x00\x00",
            b"apple\x01",
            b"apple\x02",
            b"banana",
            b"uuid:\x00\x01\x02",
            b"uuid:\x00\x01\x03",
            b"uuid:\x01\x00\x00",
        ]
        sorted_raw = sorted(keys)
        sorted_enc = sorted(keys, key=escape_encode_bytes)
        self.assertEqual(sorted_raw, sorted_enc, "Lexicographical order was not preserved!")

    def test_uniform_random_inflation_rate(self):
        exp_16 = uniform_random_expected_inflation(16)
        self.assertAlmostEqual(exp_16, 16.125, places=3)
        self.assertTrue((exp_16 - 16) / 16 < 0.01)

    def test_prefix_compression_memory_advantage(self):
        n = 100_000
        avg_len = 64
        shared_prefix = 32

        uncomp = uncompressed_hash_dict_bytes(n, avg_len)
        trie = prefix_compressed_trie_bytes(n, avg_len, shared_prefix)

        self.assertLess(trie, uncomp)
        savings = (uncomp - trie) / uncomp
        self.assertGreater(savings, 0.30, f"Expected >30% savings, got {savings:.1%}")

    def test_32bit_offset_truncation_limits(self):
        limit_32 = offset_32bit_capacity_limit()
        self.assertEqual(limit_32, 4294967295)

        max_k_64 = max_keys_before_32bit_overflow(64)
        self.assertEqual(max_k_64, 67_108_863)

        limit_64 = blob_arena_64bit_capacity_limit()
        self.assertGreater(limit_64, limit_32 * 1_000_000_000)


if __name__ == "__main__":
    unittest.main()
