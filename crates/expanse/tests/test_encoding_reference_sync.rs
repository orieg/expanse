//! Drift gate for `docs/ARCHITECTURE.md` §10, the bit-level encoding
//! reference.
//!
//! §10 publishes struct sizes, field offsets, tag discriminants and
//! immediate capacity tables. Every one of those is derived here from the
//! compiled crate and compared against the markdown, so a layout change
//! fails CI instead of silently invalidating the prose. That is the whole
//! point of the section: a document of hand-copied constants would
//! recreate the drift it was written to stop (issue #391).
//!
//! Sibling of `test_visualizer_sync.rs`, which does the same job for
//! `docs/visualizer_data.json`; the helpers deliberately mirror it.
//!
//! What is gated, and how:
//!
//! - `<!-- ENCODING-CONSTANTS -->`: every row's value must equal the value
//!   computed from the public API, and its `path:line` citation must
//!   resolve to a line that still mentions the symbol.
//! - `<!-- ENCODING-TABLE edge_type|tag32|slot_tag|slot_tag32 -->`: every
//!   listed byte must decode to the named variant (compared against the
//!   compiled `Debug` name, so a rename fails), and the listed byte set
//!   must be complete.
//! - `<!-- ENCODING-TABLE trie32_tags -->`: the shipped 32-bit engine's tag
//!   constants are module-private, so they are gated by scanning
//!   `trie32.rs` for their declarations.
//! - `<!-- ENCODING-TABLE immediate_capacity -->`: recomputed from the
//!   compiled payload budgets, not from the byte counts in the prose.
//! - The load-bearing behavioural claims (unmasked word-0 pointer, inline
//!   slot bit positions, `ArenaMeta` field positions, locator arithmetic)
//!   are asserted directly against the engine.

#![cfg(not(miri))]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use expanse_trie::bits::Bitmap256;
use expanse_trie::blobmap::{
    ARENA_ALIGN, ARENA_META_CEILING, DEFAULT_CHUNK_SIZE, MAX_ARENA_CAPACITY, MAX_ARENA_CHUNKS,
};
use expanse_trie::node::{
    BranchB, BranchHeader, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL,
};
use expanse_trie::node32::{BranchHeader32, BranchL2_32, BranchL6_32, BranchU32, LeafBitmap1_32};
use expanse_trie::slot::{SlotTag, ValueSlot};
use expanse_trie::slot32::{SlotTag32, ValueSlot32};
use expanse_trie::types::{
    BITMAP_TO_UNCOMPRESSED_THRESHOLD, BRANCH_FANOUT, BRANCH_L3_CAP, BRANCH_L7_CAP, CACHE_LINE,
    EdgeTag, EdgeType, IMMED_PAYLOAD_BYTES, ImmedType, MAX_LEVEL, RAW_ALIGN,
};
use expanse_trie::types32::{CACHE_LINE_32, Edge32, MAX_LEVEL_32, Tag32};

// ---------------------------------------------------------------------------
// Shared helpers (mirroring `test_visualizer_sync.rs`)
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("failed to locate workspace root")
        .to_path_buf()
}

fn architecture_md() -> String {
    let path = repo_root().join("docs").join("ARCHITECTURE.md");
    fs::read_to_string(&path).expect("failed to read docs/ARCHITECTURE.md")
}

/// Rows of the markdown table between `<!-- BEGIN -->` / `<!-- END -->`,
/// with the header and separator rows dropped and every cell trimmed of
/// whitespace and backticks.
fn table_rows(md: &str, begin: &str, end: &str) -> Vec<Vec<String>> {
    let start = md
        .find(begin)
        .unwrap_or_else(|| panic!("docs/ARCHITECTURE.md is missing the marker {begin:?}"));
    let rest = &md[start + begin.len()..];
    let stop = rest
        .find(end)
        .unwrap_or_else(|| panic!("docs/ARCHITECTURE.md is missing the closing marker {end:?}"));
    let block = &rest[..stop];

    let mut rows = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        // `\|` inside a cell is an escaped pipe, not a column separator.
        const ESCAPED_PIPE: char = '\u{0}';
        let cells: Vec<String> = line
            .replace("\\|", &ESCAPED_PIPE.to_string())
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().trim_matches('`').trim().replace(ESCAPED_PIPE, "|"))
            .collect();
        // Drop the separator row (`|---|---|`).
        if cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        rows.push(cells);
    }
    assert!(
        rows.len() >= 2,
        "table {begin:?} must have a header row and at least one data row"
    );
    // Drop the header row.
    rows.remove(0);
    rows
}

/// Parses `12`, `0xFF`, or `0x00FF_FFFF`.
fn parse_num(text: &str, ctx: &str) -> u64 {
    let cleaned = text.replace('_', "");
    let parsed = if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        cleaned.parse::<u64>()
    };
    parsed.unwrap_or_else(|_| panic!("{ctx}: {text:?} is not a decimal or 0x-prefixed integer"))
}

/// Reads the line a `path:line` citation names, or explains why it cannot.
fn cited_line(citation: &str) -> String {
    let (rel, lineno) = citation
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("citation {citation:?} is not in `path:line` form"));
    let lineno: usize = lineno
        .parse()
        .unwrap_or_else(|_| panic!("citation {citation:?} has a non-numeric line number"));
    let path = repo_root().join(rel);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("citation {citation:?} names a file that does not exist"));
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lineno >= 1 && lineno <= lines.len(),
        "citation {citation:?} points past the end of the file ({} lines)",
        lines.len()
    );
    lines[lineno - 1].to_string()
}

/// Identifiers in a symbol expression, minus the layout wrappers, longest
/// first. `size_of::<Edge>()` yields `["ValueSlot"-style names]`.
fn symbol_identifiers(symbol: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in symbol.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.retain(|id| id.len() >= 3 && !matches!(id.as_str(), "size_of" | "align_of" | "offset_of"));
    out.sort_by_key(|id| std::cmp::Reverse(id.len()));
    out
}

// ---------------------------------------------------------------------------
// Derived values
// ---------------------------------------------------------------------------

/// Byte offset of `Edge`'s aux field, derived from the pointer the public
/// accessor hands out (the field itself is private, so `offset_of!` is not
/// available to an integration test).
fn edge_aux_offset() -> usize {
    let edge = Edge::NULL;
    let base = (&raw const edge).cast::<u8>() as usize;
    let aux = edge.aux_bytes().as_ptr() as usize;
    aux - base
}

fn edge_aux_len() -> usize {
    Edge::NULL.aux_bytes().len()
}

/// Set-flavor immediate capacity: the 15-byte payload budget divided by the
/// remainder width (`ImmedType::max_count`).
fn immed_set_max(kb: u8) -> usize {
    ImmedType::max_count(kb) as usize
}

/// Map-flavor immediate capacity: word 0 is spent on the value or the
/// value-array pointer, so only the aux bytes hold keys. Derived from the
/// compiled aux length rather than from the literal 7.
fn immed_map_max(kb: u8) -> usize {
    edge_aux_len() / kb as usize
}

/// 32-bit set-flavor immediate capacity: keys pack across word 0 and the
/// aux bytes of an 8-byte edge, i.e. every byte but the tag.
fn immed_set32_max(kb: u8) -> usize {
    (core::mem::size_of::<Edge32>() - 1) / kb as usize
}

/// Every constant §10.8 is allowed to publish, computed from the crate.
fn pinned_constants() -> BTreeMap<&'static str, u64> {
    use core::mem::{align_of, offset_of, size_of};

    let mut m: BTreeMap<&'static str, u64> = BTreeMap::new();

    // -- Edge (64-bit) -----------------------------------------------------
    m.insert("size_of::<Edge>()", size_of::<Edge>() as u64);
    m.insert("align_of::<Edge>()", align_of::<Edge>() as u64);
    m.insert("offset_of!(Edge, aux)", edge_aux_offset() as u64);
    m.insert(
        "offset_of!(Edge, tag)",
        (edge_aux_offset() + edge_aux_len()) as u64,
    );

    // -- types.rs constants ------------------------------------------------
    m.insert("IMMED_PAYLOAD_BYTES", IMMED_PAYLOAD_BYTES as u64);
    m.insert("MAX_LEVEL", u64::from(MAX_LEVEL));
    m.insert("BRANCH_FANOUT", BRANCH_FANOUT as u64);
    m.insert("BRANCH_L3_CAP", BRANCH_L3_CAP as u64);
    m.insert("BRANCH_L7_CAP", BRANCH_L7_CAP as u64);
    m.insert(
        "BITMAP_TO_UNCOMPRESSED_THRESHOLD",
        BITMAP_TO_UNCOMPRESSED_THRESHOLD as u64,
    );
    m.insert("CACHE_LINE", CACHE_LINE as u64);
    m.insert("RAW_ALIGN", RAW_ALIGN as u64);

    // -- Branch / leaf nodes (64-bit) --------------------------------------
    m.insert(
        "size_of::<BranchHeader>()",
        size_of::<BranchHeader>() as u64,
    );
    m.insert(
        "offset_of!(BranchHeader, version)",
        offset_of!(BranchHeader, version) as u64,
    );
    m.insert(
        "offset_of!(BranchHeader, digits)",
        offset_of!(BranchHeader, digits) as u64,
    );
    m.insert("size_of::<BranchL3>()", size_of::<BranchL3>() as u64);
    m.insert(
        "offset_of!(BranchL3, edges)",
        offset_of!(BranchL3, edges) as u64,
    );
    m.insert("size_of::<BranchL7>()", size_of::<BranchL7>() as u64);
    m.insert(
        "offset_of!(BranchL7, edges)",
        offset_of!(BranchL7, edges) as u64,
    );
    m.insert("size_of::<BranchB>()", size_of::<BranchB>() as u64);
    m.insert(
        "offset_of!(BranchB, subarrays)",
        offset_of!(BranchB, subarrays) as u64,
    );
    m.insert(
        "offset_of!(BranchB, pop_counts)",
        offset_of!(BranchB, pop_counts) as u64,
    );
    m.insert(
        "offset_of!(BranchB, version)",
        offset_of!(BranchB, version) as u64,
    );
    m.insert("size_of::<BranchU>()", size_of::<BranchU>() as u64);
    m.insert(
        "offset_of!(BranchU, version)",
        offset_of!(BranchU, version) as u64,
    );
    m.insert("size_of::<LeafBitmap1>()", size_of::<LeafBitmap1>() as u64);
    m.insert(
        "offset_of!(LeafBitmap1, version)",
        offset_of!(LeafBitmap1, version) as u64,
    );
    m.insert("size_of::<LeafBitmapL>()", size_of::<LeafBitmapL>() as u64);
    m.insert(
        "offset_of!(LeafBitmapL, values)",
        offset_of!(LeafBitmapL, values) as u64,
    );
    m.insert(
        "offset_of!(LeafBitmapL, version)",
        offset_of!(LeafBitmapL, version) as u64,
    );
    m.insert("size_of::<Bitmap256>()", size_of::<Bitmap256>() as u64);

    // -- ValueSlot (64-bit) ------------------------------------------------
    m.insert("size_of::<ValueSlot>()", size_of::<ValueSlot>() as u64);
    m.insert("ValueSlot::TAG_MASK", ValueSlot::TAG_MASK);
    m.insert("ValueSlot::ARENA_META_MASK", ValueSlot::ARENA_META_MASK);
    m.insert(
        "ValueSlot::ARENA_META_MAX",
        u64::from(ValueSlot::ARENA_META_MAX),
    );

    // -- Blob arena geometry -----------------------------------------------
    m.insert("ARENA_ALIGN", ARENA_ALIGN as u64);
    m.insert("ARENA_META_CEILING", ARENA_META_CEILING);
    m.insert("MAX_ARENA_CHUNKS", MAX_ARENA_CHUNKS as u64);
    m.insert("MAX_ARENA_CAPACITY", MAX_ARENA_CAPACITY as u64);
    m.insert("DEFAULT_CHUNK_SIZE", DEFAULT_CHUNK_SIZE as u64);

    // -- 32-bit ------------------------------------------------------------
    m.insert("size_of::<Edge32>()", size_of::<Edge32>() as u64);
    m.insert("align_of::<Edge32>()", align_of::<Edge32>() as u64);
    m.insert("MAX_LEVEL_32", u64::from(MAX_LEVEL_32));
    m.insert("CACHE_LINE_32", CACHE_LINE_32 as u64);
    m.insert(
        "size_of::<BranchHeader32>()",
        size_of::<BranchHeader32>() as u64,
    );
    m.insert("size_of::<BranchL2_32>()", size_of::<BranchL2_32>() as u64);
    m.insert("size_of::<BranchL6_32>()", size_of::<BranchL6_32>() as u64);
    m.insert("size_of::<BranchU32>()", size_of::<BranchU32>() as u64);
    m.insert(
        "size_of::<LeafBitmap1_32>()",
        size_of::<LeafBitmap1_32>() as u64,
    );
    m.insert("size_of::<ValueSlot32>()", size_of::<ValueSlot32>() as u64);
    m.insert("ValueSlot32::TAG_MASK", u64::from(ValueSlot32::TAG_MASK));
    m.insert(
        "ValueSlot32::ARENA_OFFSET_MASK",
        u64::from(ValueSlot32::ARENA_OFFSET_MASK),
    );
    m.insert(
        "ValueSlot32::ARENA_OFFSET_SHIFT",
        u64::from(ValueSlot32::ARENA_OFFSET_SHIFT),
    );
    m.insert(
        "ValueSlot32::ARENA_META_MASK",
        u64::from(ValueSlot32::ARENA_META_MASK),
    );
    m.insert(
        "ValueSlot32::ARENA_META_SHIFT",
        u64::from(ValueSlot32::ARENA_META_SHIFT),
    );

    m
}

// ---------------------------------------------------------------------------
// §10.8 pinned constants
// ---------------------------------------------------------------------------

#[test]
fn test_encoding_constants_match_source() {
    let md = architecture_md();
    let rows = table_rows(
        &md,
        "<!-- ENCODING-CONSTANTS -->",
        "<!-- /ENCODING-CONSTANTS -->",
    );
    let expected = pinned_constants();

    let mut seen: Vec<String> = Vec::new();
    for row in &rows {
        assert_eq!(
            row.len(),
            3,
            "each ENCODING-CONSTANTS row is `| Symbol | Value | Source |`; got {row:?}"
        );
        let (symbol, value) = (&row[0], &row[1]);
        let want = expected.get(symbol.as_str()).unwrap_or_else(|| {
            panic!(
                "docs/ARCHITECTURE.md section 10.8 publishes {symbol:?}, which this gate cannot \
                 compute. Every published constant must be derivable from the crate -- add it to \
                 `pinned_constants()` in crates/expanse/tests/test_encoding_reference_sync.rs, or \
                 drop the row."
            )
        });
        let got = parse_num(value, &format!("ENCODING-CONSTANTS row {symbol:?}"));
        assert_eq!(
            got, *want,
            "docs/ARCHITECTURE.md section 10.8 publishes {symbol} = {value}, but the compiled \
             crate says {want}. The code wins: update the document."
        );
        seen.push(symbol.clone());
    }

    let mut missing: Vec<&str> = expected
        .keys()
        .filter(|k| !seen.iter().any(|s| s == *k))
        .copied()
        .collect();
    missing.sort_unstable();
    assert!(
        missing.is_empty(),
        "these constants are gated but no longer published in docs/ARCHITECTURE.md section 10.8: \
         {missing:?}. Removing a row silently un-gates it, so re-add the row or remove it from \
         `pinned_constants()`."
    );
}

#[test]
fn test_encoding_citations_resolve() {
    let md = architecture_md();
    let rows = table_rows(
        &md,
        "<!-- ENCODING-CONSTANTS -->",
        "<!-- /ENCODING-CONSTANTS -->",
    );

    for row in &rows {
        let (symbol, citation) = (&row[0], &row[2]);
        let line = cited_line(citation);
        let idents = symbol_identifiers(symbol);
        assert!(
            !idents.is_empty(),
            "symbol {symbol:?} has no identifier a citation could be checked against"
        );
        assert!(
            idents.iter().any(|id| line.contains(id.as_str())),
            "docs/ARCHITECTURE.md section 10.8 cites {citation} for {symbol}, but that line \
             mentions none of {idents:?}:\n    {}\nThe citation has drifted; re-point it at the \
             declaration.",
            line.trim()
        );
    }
}

/// Every `path:line` in §10's prose, not just the ones in §10.8, must name
/// a file and line that still exist. A citation that has scrolled off the
/// end of its file is exactly the kind of quiet decay this section exists
/// to prevent.
#[test]
fn test_encoding_prose_citations_resolve() {
    let md = architecture_md();
    let section = md
        .split_once("## 10. Bit-level encoding reference")
        .expect("docs/ARCHITECTURE.md must contain section 10")
        .1;

    let mut checked = 0usize;
    for chunk in section.split('`') {
        let Some((rel, lineno)) = chunk.rsplit_once(':') else {
            continue;
        };
        if !rel.starts_with("crates/") || !rel.ends_with(".rs") {
            continue;
        }
        let Ok(lineno) = lineno.parse::<usize>() else {
            continue;
        };
        let path = repo_root().join(rel);
        let text = fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("docs/ARCHITECTURE.md section 10 cites {rel}:{lineno}, which does not exist")
        });
        let count = text.lines().count();
        assert!(
            lineno >= 1 && lineno <= count,
            "docs/ARCHITECTURE.md section 10 cites {rel}:{lineno}, but that file has {count} lines"
        );
        checked += 1;
    }
    assert!(
        checked >= 50,
        "expected section 10 to carry at least 50 source citations, found {checked} -- the \
         scanner or the section has changed shape"
    );
}

// ---------------------------------------------------------------------------
// §10.3 tag discriminants
// ---------------------------------------------------------------------------

#[test]
fn test_edge_type_table_matches_source() {
    let md = architecture_md();
    let rows = table_rows(
        &md,
        "<!-- ENCODING-TABLE edge_type -->",
        "<!-- /ENCODING-TABLE -->",
    );

    let mut listed: Vec<u8> = Vec::new();
    for row in &rows {
        assert_eq!(
            row.len(),
            3,
            "edge_type row is `| Variant | Tag byte | Refers to |`"
        );
        let byte =
            u8::try_from(parse_num(&row[1], "edge_type tag byte")).expect("tag byte fits u8");
        let decoded = EdgeType::from_u8(byte).unwrap_or_else(|| {
            panic!(
                "docs/ARCHITECTURE.md section 10.3.1 lists tag {byte:#04x} as {:?}, but \
                 EdgeType::from_u8 rejects it",
                row[0]
            )
        });
        assert_eq!(
            format!("{decoded:?}"),
            row[0],
            "tag {byte:#04x} decodes to {decoded:?}, not {}",
            row[0]
        );
        listed.push(byte);
    }
    listed.sort_unstable();

    let mut actual: Vec<u8> = (0..=u8::MAX)
        .filter(|b| EdgeType::from_u8(*b).is_some())
        .collect();
    actual.sort_unstable();
    assert_eq!(
        listed, actual,
        "docs/ARCHITECTURE.md section 10.3.1 must list every structural tag byte exactly once"
    );
}

#[test]
fn test_tag32_table_matches_source() {
    let md = architecture_md();
    let rows = table_rows(
        &md,
        "<!-- ENCODING-TABLE tag32 -->",
        "<!-- /ENCODING-TABLE -->",
    );

    let mut listed: Vec<u8> = Vec::new();
    for row in &rows {
        assert_eq!(row.len(), 2, "tag32 row is `| Variant | Tag byte |`");
        let byte = u8::try_from(parse_num(&row[1], "tag32 tag byte")).expect("tag byte fits u8");
        let decoded = Tag32::from_u8(byte);
        assert_eq!(
            format!("{decoded:?}"),
            row[0],
            "tag {byte:#04x} decodes to {decoded:?}, not {}",
            row[0]
        );
        listed.push(byte);
    }
    listed.sort_unstable();

    // Every byte with a dedicated variant, plus the one byte the document
    // uses to illustrate the `Custom` catch-all.
    let mut actual: Vec<u8> = (0..=u8::MAX)
        .filter(|b| Tag32::from_u8(*b) != Tag32::Custom)
        .collect();
    actual.push(0xFF);
    actual.sort_unstable();
    assert_eq!(
        listed, actual,
        "docs/ARCHITECTURE.md section 10.3.3 must list every dedicated Tag32 byte, plus 0xFF for \
         the Custom catch-all"
    );
}

#[test]
fn test_trie32_engine_tags_match_source() {
    let md = architecture_md();
    let rows = table_rows(
        &md,
        "<!-- ENCODING-TABLE trie32_tags -->",
        "<!-- /ENCODING-TABLE -->",
    );

    // The engine's tag constants are module-private, so the gate reads the
    // declarations out of the source instead of referencing the symbols.
    let src = fs::read_to_string(repo_root().join("crates/expanse/src/trie32.rs"))
        .expect("failed to read crates/expanse/src/trie32.rs");
    let mut declared: BTreeMap<String, u64> = BTreeMap::new();
    for line in src.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("const ") else {
            continue;
        };
        let Some((name, tail)) = rest.split_once(':') else {
            continue;
        };
        let Some((ty, value)) = tail.split_once('=') else {
            continue;
        };
        if ty.trim() != "u8" {
            continue;
        }
        let value = value.trim().trim_end_matches(';').trim();
        declared.insert(
            name.trim().to_string(),
            parse_num(value, &format!("trie32.rs const {name}")),
        );
    }
    assert!(
        !declared.is_empty(),
        "no `const NAME: u8 = ...;` declarations found in crates/expanse/src/trie32.rs -- the \
         scan this gate depends on has broken"
    );

    for row in &rows {
        assert_eq!(
            row.len(),
            3,
            "trie32_tags row is `| Constant | Value | Meaning |`"
        );
        let name = &row[0];
        let want = declared.get(name.as_str()).unwrap_or_else(|| {
            panic!(
                "docs/ARCHITECTURE.md section 10.3.3 publishes {name}, which no longer exists in \
                 crates/expanse/src/trie32.rs"
            )
        });
        let got = parse_num(&row[1], &format!("trie32_tags row {name}"));
        assert_eq!(
            got, *want,
            "section 10.3.3 publishes {name} = {}, but trie32.rs declares {want} ({want:#04x})",
            row[1]
        );
    }

    let published: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
    let missing: Vec<&String> = declared
        .keys()
        .filter(|k| k.starts_with("T_") && !published.contains(&k.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "crates/expanse/src/trie32.rs declares tag constants section 10.3.3 does not publish: \
         {missing:?}"
    );
}

#[test]
fn test_slot_tag_table_matches_source() {
    let md = architecture_md();
    let rows = table_rows(
        &md,
        "<!-- ENCODING-TABLE slot_tag -->",
        "<!-- /ENCODING-TABLE -->",
    );

    let mut listed: Vec<u8> = Vec::new();
    for row in &rows {
        assert_eq!(
            row.len(),
            4,
            "slot_tag row is `| Variant | Tag byte | Inline length | Rest of the word |`"
        );
        let byte = u8::try_from(parse_num(&row[1], "slot_tag tag byte")).expect("tag byte fits u8");
        let decoded = SlotTag::from_u8(byte);
        assert_eq!(
            format!("{decoded:?}"),
            row[0],
            "slot tag {byte:#04x} decodes to {decoded:?}, not {}",
            row[0]
        );
        let want_len = match decoded.inline_len() {
            Some(n) => n.to_string(),
            None => "\u{2014}".to_string(),
        };
        assert_eq!(
            row[2], want_len,
            "slot tag {byte:#04x} has inline length {want_len}, not {}",
            row[2]
        );
        listed.push(byte);
    }
    listed.sort_unstable();

    let mut actual: Vec<u8> = (0..=u8::MAX)
        .filter(|b| SlotTag::from_u8(*b) != SlotTag::RawWord)
        .collect();
    actual.push(0xFF);
    actual.sort_unstable();
    assert_eq!(
        listed, actual,
        "docs/ARCHITECTURE.md section 10.5 must list every dedicated SlotTag byte, plus 0xFF for \
         the RawWord catch-all"
    );
}

#[test]
fn test_slot_tag32_table_matches_source() {
    let md = architecture_md();
    let rows = table_rows(
        &md,
        "<!-- ENCODING-TABLE slot_tag32 -->",
        "<!-- /ENCODING-TABLE -->",
    );

    let mut listed: Vec<u8> = Vec::new();
    for row in &rows {
        assert_eq!(
            row.len(),
            3,
            "slot_tag32 row is `| Variant | Tag byte | Rest of the word |`"
        );
        let byte = u8::try_from(parse_num(&row[1], "slot_tag32 tag byte")).expect("byte fits u8");
        let decoded = SlotTag32::from(byte);
        assert_eq!(
            format!("{decoded:?}"),
            row[0],
            "32-bit slot tag {byte:#04x} decodes to {decoded:?}, not {}",
            row[0]
        );
        listed.push(byte);
    }
    listed.sort_unstable();

    let mut actual: Vec<u8> = (0..=u8::MAX)
        .filter(|b| SlotTag32::from(*b) != SlotTag32::RawWord)
        .collect();
    actual.push(0xFF);
    actual.sort_unstable();
    assert_eq!(
        listed, actual,
        "docs/ARCHITECTURE.md section 10.5 must list every dedicated SlotTag32 byte, plus 0xFF for \
         the RawWord catch-all"
    );
}

// ---------------------------------------------------------------------------
// §10.4 immediate capacity
// ---------------------------------------------------------------------------

#[test]
fn test_immediate_capacity_table_matches_source() {
    let md = architecture_md();
    let rows = table_rows(
        &md,
        "<!-- ENCODING-TABLE immediate_capacity -->",
        "<!-- /ENCODING-TABLE -->",
    );

    let max_kb = u8::try_from(IMMED_PAYLOAD_BYTES / 2)
        .expect("fits u8")
        .min(7);
    assert_eq!(
        rows.len(),
        max_kb as usize,
        "section 10.4 must have one row per immediate key width 1..={max_kb}"
    );

    for (i, row) in rows.iter().enumerate() {
        assert_eq!(
            row.len(),
            4,
            "immediate_capacity row is `| Key bytes | 64-bit set | 64-bit map | 32-bit set |`"
        );
        let kb = u8::try_from(parse_num(&row[0], "immediate_capacity key width")).expect("fits u8");
        assert_eq!(
            kb as usize,
            i + 1,
            "section 10.4 rows must run over key widths 1..={max_kb} in order"
        );

        let want_set = immed_set_max(kb);
        let got_set = parse_num(&row[1], "immediate_capacity 64-bit set") as usize;
        assert_eq!(
            got_set, want_set,
            "section 10.4: {kb}-byte remainders fit {want_set} set-flavor keys in an immediate \
             ({IMMED_PAYLOAD_BYTES}-byte payload budget), not {got_set}"
        );

        let want_map = immed_map_max(kb);
        let got_map = parse_num(&row[2], "immediate_capacity 64-bit map") as usize;
        assert_eq!(
            got_map,
            want_map,
            "section 10.4: {kb}-byte remainders fit {want_map} map-flavor keys in an immediate \
             ({} aux bytes; word 0 carries the value or the value-array pointer), not {got_map}",
            edge_aux_len()
        );

        // A 32-bit key has at most MAX_LEVEL_32 - ... undecoded bytes, so
        // wider remainders cannot occur there at all.
        let max_kb_32 = MAX_LEVEL_32;
        if kb <= max_kb_32 {
            let want32 = immed_set32_max(kb);
            let got32 = parse_num(&row[3], "immediate_capacity 32-bit set") as usize;
            assert_eq!(
                got32, want32,
                "section 10.4: {kb}-byte remainders fit {want32} set-flavor keys in an 8-byte \
                 Edge32, not {got32}"
            );
        } else {
            assert_eq!(
                row[3], "n/a",
                "section 10.4: a {kb}-byte remainder cannot occur on a {max_kb_32}-level 32-bit \
                 trie, so the cell must read `n/a`"
            );
        }
    }
}

#[test]
fn test_immediate_tag_space_totals() {
    // The document states 14 structural + 37 immediate = 51 valid tag bytes.
    let mut structural = 0usize;
    let mut immediate = 0usize;
    for raw in 0..=u8::MAX {
        let s = EdgeType::from_u8(raw);
        let i = ImmedType::from_u8(raw);
        assert!(
            s.is_none() || i.is_none(),
            "tag {raw:#04x} decodes as both structural and immediate"
        );
        if s.is_some() {
            structural += 1;
        }
        if i.is_some() {
            immediate += 1;
        }
        if s.is_some() || i.is_some() {
            assert_eq!(
                EdgeTag::from_u8(raw).map(EdgeTag::as_u8),
                Some(raw),
                "EdgeTag must round-trip {raw:#04x}"
            );
        } else {
            assert!(EdgeTag::from_u8(raw).is_none());
        }
    }
    assert_eq!(
        structural, 14,
        "structural tag count published in section 10.3.2"
    );
    assert_eq!(
        immediate, 37,
        "immediate tag count published in section 10.3.2"
    );

    let md = architecture_md();
    assert!(
        md.contains(&format!(
            "{structural} structural + {immediate} immediate = {} of 256",
            structural + immediate
        )),
        "section 10.3.2 must publish the tag-space totals as \
         `{structural} structural + {immediate} immediate = {} of 256`",
        structural + immediate
    );
}

// ---------------------------------------------------------------------------
// Behavioural gates for the load-bearing prose claims
// ---------------------------------------------------------------------------

/// §10.1: word 0 holds the raw untruncated 64-bit pointer. No bit above 47
/// is stolen, which is what keeps the representation correct under 52-bit
/// ARM64 LVA and 57-bit x86-64 LA57.
#[test]
fn test_edge_word0_is_stored_unmasked() {
    // Values with every bit above 47 set, plus a canonical-looking LA57
    // address and an ARM64 LVA-range address. These are never dereferenced.
    let probes: [usize; 4] = [
        usize::MAX & !0xF,
        0xFFFF_FFFF_FFFF_FFF0,
        0x00FF_1234_5678_9AB0, // above bit 47, within 57-bit LA57
        0x000F_1234_5678_9AB0, // above bit 47, within 52-bit ARM64 LVA
    ];
    for probe in probes {
        let ptr = probe as *mut u8;
        let edge = Edge::new_node(ptr, EdgeType::BranchL3.as_u8());
        assert_eq!(
            edge.node_ptr() as usize,
            probe,
            "Edge::node_ptr must return {probe:#018x} bit-for-bit; any masking here would \
             corrupt pointers on LA57 / ARM64-LVA hardware (docs/ARCHITECTURE.md section 10.1)"
        );
        assert_eq!(
            edge.tag_byte(),
            EdgeType::BranchL3.as_u8(),
            "the tag lives in its own byte and must be unaffected by the pointer value"
        );
    }
}

/// §10.1: aux carries `pop0` in its low `L` bytes and the narrow-pointer
/// decode digits above them, and the two never overlap.
#[test]
fn test_edge_aux_is_level_split() {
    let aux_len = edge_aux_len();
    assert_eq!(
        edge_aux_offset(),
        core::mem::size_of::<usize>(),
        "aux starts at word 1"
    );
    assert_eq!(
        edge_aux_offset() + aux_len + 1,
        core::mem::size_of::<Edge>(),
        "aux plus the one tag byte must exactly fill the second word"
    );

    for level in 1..=6u8 {
        let mut edge = Edge::NULL;
        let max_pop0 = (1u64 << (u32::from(level) * 8)) - 1;
        edge.set_pop0(level, max_pop0);
        let decode: Vec<u8> = (0..(7 - level)).map(|i| 0xA0 | i).collect();
        edge.set_decode_bytes(level, &decode);
        assert_eq!(
            edge.pop0(level),
            max_pop0,
            "decode bytes must not clobber pop0 at level {level}"
        );
        assert_eq!(
            edge.decode_bytes(level),
            &decode[..],
            "pop0 must not clobber the decode bytes at level {level}"
        );
    }
}

/// §10.5: an inline payload occupies bits 63:8, and the low byte is its
/// length. This is where `ExpanseBlobMap` puts payloads of <= 7 bytes --
/// in the leaf's value slot, not inside an edge.
#[test]
fn test_inline_value_slot_bit_positions() {
    let max = ValueSlot::TAG_MASK as usize; // the tag doubles as the length
    for len in 0..=7usize {
        let bytes: Vec<u8> = (0..len).map(|i| 0xA1u8.wrapping_add(i as u8)).collect();
        let slot = ValueSlot::new_inline(&bytes).expect("<= 7 bytes is inline");
        let raw = slot.to_raw();

        assert_eq!(
            raw & ValueSlot::TAG_MASK,
            len as u64,
            "the low byte of an inline slot is the payload length"
        );
        assert_eq!(
            slot.tag().inline_len(),
            Some(len),
            "the tag decodes back to the payload length"
        );

        let mut want_payload = 0u64;
        for (i, &b) in bytes.iter().enumerate() {
            want_payload |= u64::from(b) << (8 * i);
        }
        assert_eq!(
            raw >> 8,
            want_payload,
            "inline payload bytes occupy bits 63:8 little-endian"
        );

        let (buf, got_len) = slot.inline_payload();
        assert_eq!(got_len, len);
        assert_eq!(&buf[..got_len], &bytes[..]);
    }
    assert!(
        ValueSlot::new_inline(&[0u8; 8]).is_none(),
        "8 bytes does not fit alongside the tag byte"
    );
    assert!(max >= 7, "the tag field must be able to encode length 7");
}

/// §10.5: `[hot_meta 24 | locator 32 | tag 8]`, with metadata rejected
/// rather than truncated above the 24-bit field.
#[test]
fn test_arena_meta_field_positions() {
    let meta = 0x00AB_CDEFu32 & ValueSlot::ARENA_META_MAX;
    let locator = 0xDEAD_BEEFu32;
    let slot = ValueSlot::new_arena_meta(meta, locator).expect("24-bit meta");

    assert_eq!(
        slot.to_raw(),
        (u64::from(meta) << 40) | (u64::from(locator) << 8) | 0x10,
        "the ArenaMeta word is `(hot_meta << 40) | (locator << 8) | 0x10`"
    );
    assert_eq!(slot.tag(), SlotTag::ArenaMeta);
    assert_eq!(
        slot.arena_meta_meta(),
        meta,
        "hot metadata reads from bits 63:40"
    );
    assert_eq!(
        slot.arena_meta_locator(),
        locator,
        "the locator reads from bits 39:8"
    );

    // Fields are disjoint: rewriting one leaves the other alone.
    let updated = slot.with_arena_meta_meta(0x0012_3456).expect("24-bit meta");
    assert_eq!(updated.arena_meta_meta(), 0x0012_3456);
    assert_eq!(updated.arena_meta_locator(), locator);

    // 24 bits exactly, and overflow is rejected rather than truncated.
    assert_eq!(
        ValueSlot::ARENA_META_MAX,
        (1u32 << 24) - 1,
        "hot metadata is a 24-bit field"
    );
    assert!(ValueSlot::new_arena_meta(ValueSlot::ARENA_META_MAX + 1, locator).is_none());
    assert!(slot.with_arena_meta_meta(1 << 24).is_none());
}

/// §10.5: the locator is a flat global address in `ARENA_ALIGN`-byte units,
/// not a chunk/offset pair.
#[test]
fn test_arena_locator_arithmetic() {
    assert_eq!(
        ARENA_META_CEILING,
        (1u64 << 32) * ARENA_ALIGN as u64,
        "the 32-bit locator addresses 2^32 units of ARENA_ALIGN bytes"
    );
    assert!(
        (MAX_ARENA_CAPACITY as u64) < ARENA_META_CEILING,
        "the shipped growth cap must stay inside the locator envelope, so a locator overflow \
         cannot occur under it"
    );
    for global in [0u64, 16, 4096, 1 << 20, (MAX_ARENA_CAPACITY as u64) - 16] {
        let locator = u32::try_from(global / ARENA_ALIGN as u64).expect("within envelope");
        assert_eq!(
            u64::from(locator) * ARENA_ALIGN as u64,
            global,
            "locator <-> global-offset conversion must be exact for 16-byte-aligned records"
        );
    }
}

/// §10.5: both `ValueSlot32` arena fields are 12 bits wide -- not the 16
/// bits older prose claimed.
#[test]
fn test_value_slot32_arena_fields_are_12_bit() {
    assert_eq!(
        ValueSlot32::ARENA_OFFSET_MASK >> ValueSlot32::ARENA_OFFSET_SHIFT,
        0x0FFF,
        "the slab offset is a 12-bit field"
    );
    assert_eq!(
        ValueSlot32::ARENA_META_MASK >> ValueSlot32::ARENA_META_SHIFT,
        0x0FFF,
        "the hot-metadata field is 12 bits, so 4096 states"
    );
    assert!(
        ValueSlot32::new_arena(0x1000, 0).is_none(),
        "metadata above 12 bits must be rejected, not truncated"
    );
    assert!(
        ValueSlot32::new_arena(0, 0x1000).is_none(),
        "a slab offset above 12 bits must be rejected, not truncated"
    );
    let slot = ValueSlot32::new_arena(0x0FFF, 0x0FFF).expect("both fields at their maximum");
    assert_eq!(slot.hot_meta(), 0x0FFF);
    assert_eq!(slot.slab_offset(), 0x0FFF);
    assert_eq!(slot.tag(), SlotTag32::Arena);
}

/// §10.6: reaching a value in a bitmap map-leaf is `bitmap test` ->
/// `subexpanse rank` -> index into the subexpanse's packed array, with the
/// 256 digits split into eight 32-digit subexpanses.
#[test]
fn test_bitmap_subexpanse_rank_addressing() {
    let subexpanses = core::mem::size_of::<Bitmap256>() * 8 / 32;
    assert_eq!(
        subexpanses, 8,
        "256 digits split into eight 32-digit subexpanses"
    );

    let mut bm = Bitmap256::new();
    let members: [u8; 6] = [0x03, 0x11, 0x20, 0x2F, 0x80, 0xFF];
    for m in members {
        assert!(bm.set(m));
    }

    for m in members {
        let sub = (m >> 5) as usize;
        let expected_rank = members
            .iter()
            .filter(|&&other| (other >> 5) as usize == sub && other < m)
            .count();
        assert_eq!(
            bm.test_and_subexpanse_rank(m),
            Some(expected_rank),
            "digit {m:#04x} sits at rank {expected_rank} inside subexpanse {sub}"
        );
        assert_eq!(
            bm.test_and_subexpanse_rank_with_sub(m),
            Some((sub, expected_rank))
        );
        assert_eq!(bm.subexpanse_rank(m) as usize, expected_rank);
    }

    for sub in 0..subexpanses {
        let expected = members
            .iter()
            .filter(|&&m| (m >> 5) as usize == sub)
            .count();
        assert_eq!(
            bm.subexpanse_count(sub) as usize,
            expected,
            "subexpanse {sub} holds {expected} members, which is its packed array's length"
        );
    }

    // `rank`/`select` are the *global* pair used for ordered navigation.
    let mut sorted = members;
    sorted.sort_unstable();
    for (n, &m) in sorted.iter().enumerate() {
        assert_eq!(bm.rank(m) as usize, n);
        assert_eq!(bm.select(n as u32), Some(m));
    }
    assert_eq!(bm.select(members.len() as u32), None);
}
