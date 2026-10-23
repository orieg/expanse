//! Every mutation of the shared string map in `sync.rs` calls its shared
//! twin (Refs #1586).
//!
//! The plain `ExpanseStrMap::insert` and `remove` carry no shared path: on a
//! map switched to deferred reclamation they would free what a reader may
//! still hold. Their debug guard catches a call that a test reaches; a call
//! on a path no default test reaches (the `ablation-str-serial-writers`
//! build, a rare fallback) is caught here, by a scan of the production code
//! of `SyncExpanseStrMap` and `StrExclusive` (AGENTS.md §5, structural
//! attribution).

use std::string::String;
use std::vec::Vec;

/// The shared-twin call sites in the scanned region: `from_map`'s rebuild;
/// `insert`'s and `remove`'s serial-ablation and fallback paths (four);
/// `compare_exchange`'s exclusive closure (two); and `StrExclusive::insert`
/// and `remove`.
const TWIN_CALLS: usize = 9;

/// The real source, scanned and mutated below.
const SRC: &str = include_str!("../sync.rs");

/// One source line with its string literals emptied and everything from
/// `//` on dropped, so neither a comment nor a literal can count.
fn code_of(line: &str) -> String {
    let mut out = String::new();
    let mut in_str = false;
    let mut escaped = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
                out.push('"');
            }
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push('"');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            break;
        }
        out.push(c);
    }
    String::from(out.trim())
}

/// The code lines from `impl SyncExpanseStrMap {` to the string reader walk
/// that follows the wrapper, `StrExclusive` and `From` included. Line endings
/// are normalised first: a CRLF checkout (Windows) would otherwise match
/// neither marker.
fn region_code(src: &str) -> Vec<String> {
    let src = src.replace("\r\n", "\n");
    let start = src
        .find("\nimpl SyncExpanseStrMap {\n")
        .expect("the wrapper is in sync.rs");
    let end = start
        + src[start..]
            .find("\nmacro_rules! str_get_walk")
            .expect("the reader walk after the wrapper");
    src[start..end]
        .lines()
        .map(code_of)
        .filter(|l| !l.is_empty())
        .collect()
}

/// Whether `line` contains `needle` with nothing identifier-like before it,
/// so `m.insert(` matches `|m| m.insert(` and not `tm.insert(`.
fn calls(line: &str, needle: &str) -> bool {
    line.match_indices(needle).any(|(i, _)| {
        line[..i]
            .chars()
            .next_back()
            .is_none_or(|p| !(p.is_alphanumeric() || p == '_'))
    })
}

/// Whether `line` names `ExpanseStrMap::insert` or `::remove` as a path
/// (`write(ExpanseStrMap::insert)`), not as a prefix of a longer name.
fn path_form(line: &str) -> bool {
    ["ExpanseStrMap::insert", "ExpanseStrMap::remove"]
        .iter()
        .any(|p| {
            line.match_indices(p).any(|(i, _)| {
                !line[i + p.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
            })
        })
}

/// The scan: no plain `insert`/`remove` on the map (`m.`, `map.`,
/// `self.map.`, or the `ExpanseStrMap::` path form), and exactly
/// `TWIN_CALLS` shared-twin calls. Returns the clause that failed.
fn scan(src: &str) -> Result<(), String> {
    const PLAIN: [&str; 4] = ["m.insert(", "m.remove(", "map.insert(", "map.remove("];
    let code = region_code(src);
    if let Some(l) = code
        .iter()
        .find(|l| PLAIN.iter().any(|p| calls(l, p)) || path_form(l))
    {
        return Err(std::format!(
            "plain: `{l}` mutates the shared map through the plain entry"
        ));
    }
    let twins: usize = code
        .iter()
        .map(|l| l.matches(".insert_deferred(").count() + l.matches(".remove_deferred(").count())
        .sum();
    if twins != TWIN_CALLS {
        return Err(std::format!(
            "count: {twins} shared-twin calls, want {TWIN_CALLS}"
        ));
    }
    Ok(())
}

#[test]
fn deferred_str_mutations_call_the_shared_twin() {
    assert_eq!(scan(SRC), Ok(()));
}

/// The scan's negative controls, one per clause, on mutated copies of the
/// real source: a site reverted to the plain entry, in method or path
/// form, fails the plain clause; a twin call rewritten out of the counted
/// form, or commented out, fails the count with the plain clause clean; a
/// plain call in a comment or a string literal is not seen.
#[test]
fn deferred_dispatch_scan_fails_on_each_clause() {
    let site = ".write_root_covered_exact(|m| m.insert_deferred(key, val)),";
    assert!(SRC.contains(site));
    let reverted = SRC.replacen(
        site,
        ".write_root_covered_exact(|m| m.insert(key, val)),",
        1,
    );
    assert!(scan(&reverted).unwrap_err().starts_with("plain:"));
    let path = SRC.replacen(site, ".write_root_covered_exact(ExpanseStrMap::insert),", 1);
    assert!(scan(&path).unwrap_err().starts_with("plain:"));

    let exclusive = "self.map.remove_deferred(key)";
    assert!(SRC.contains(exclusive));
    let rewritten = SRC.replacen(
        exclusive,
        "ExpanseStrMap::remove_deferred(self.map, key)",
        1,
    );
    assert!(scan(&rewritten).unwrap_err().starts_with("count:"));
    let commented = SRC.replacen(exclusive, &std::format!("0 // {exclusive}"), 1);
    assert!(scan(&commented).unwrap_err().starts_with("count:"));

    let in_comment = SRC.replacen(site, &std::format!("{site} // m.insert(key, val)"), 1);
    assert_eq!(scan(&in_comment), Ok(()));
    let in_literal = SRC.replacen(site, &std::format!("{site} let _ = \"m.remove(key)\";"), 1);
    assert_eq!(scan(&in_literal), Ok(()));
}

/// The scan on a CRLF copy of the source, as a Windows checkout gives it:
/// the same region and lines as the LF copy, a clean scan, and each clause
/// still red on a mutated CRLF copy, so the normalisation cannot make the
/// controls above pass vacuously.
#[test]
fn deferred_dispatch_scan_reads_a_crlf_checkout() {
    let lf = SRC.replace("\r\n", "\n");
    let crlf = lf.replace('\n', "\r\n");
    assert!(crlf.contains("\r\nimpl SyncExpanseStrMap {\r\n"));
    let region = region_code(&crlf);
    assert!(!region.is_empty());
    assert_eq!(region, region_code(&lf));
    assert_eq!(scan(&crlf), Ok(()));

    let site = ".write_root_covered_exact(|m| m.insert_deferred(key, val)),";
    let reverted = crlf.replacen(
        site,
        ".write_root_covered_exact(|m| m.insert(key, val)),",
        1,
    );
    assert!(scan(&reverted).unwrap_err().starts_with("plain:"));
    let exclusive = "self.map.remove_deferred(key)";
    let commented = crlf.replacen(exclusive, &std::format!("0 // {exclusive}"), 1);
    assert!(scan(&commented).unwrap_err().starts_with("count:"));
}
