//! Structural check that every optimistic-reader answer is validated
//! (#1189, `docs/ARCHITECTURE.md` §4.1).
//!
//! The rule: every answer a validated read returns, an absence included,
//! depends only on loads that a successful validation covers. The
//! deterministic tests in `sync::validated_answer_tests` inject a writer at
//! four sites and assert a retry. This file checks every other return site by
//! reading the source:
//!
//! - in `walk_validated_body!` (`src/sync.rs`), each `return Ok(` has a
//!   validation (`chk!();` or `node_validate(`) before it on the same path,
//!   with no shared load between the two, and without crossing into another
//!   match arm;
//! - in `src/sync_nav.rs`, the two entry points answer only after validating
//!   the retained read set and the tree version, and every branch version the
//!   search samples goes through `ReadSet::sample`, which retains it.
//!
//! The scanners strip `//` comments before matching, and each clause was
//! shown to fail on its own when the code it guards was broken (PR body of
//! the change that added this file).
#![cfg(not(miri))]

const SYNC: &str = include_str!("../src/sync.rs");
const SYNC_NAV: &str = include_str!("../src/sync_nav.rs");

/// `return Ok(` sites in `walk_validated_body!`. A change that adds or
/// removes one updates this count, and so is reviewed against the rule.
const WALK_RETURN_SITES: usize = 23;

/// Calls that load shared memory on a validated walk.
const SHARED_LOADS: &[&str] = &[
    "shared_word::load",
    "shared_bitmap::",
    "shared_keys::find",
    "Edge::load_at",
    "load_ptr",
    "BranchHeader::load_at",
    "node_sample(",
];

/// Calls that validate the walk's current cover.
const VALIDATIONS: &[&str] = &["chk!();", "node_validate("];

/// The code of a line, without its `//` comment.
fn code(line: &str) -> &str {
    line.split("//").next().unwrap_or("")
}

/// The body of `walk_validated_body!`: from its `macro_rules!` line to the
/// first line that is a bare `}` at column 0.
fn walk_body(src: &str) -> Vec<&str> {
    let lines: Vec<&str> = src.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == "macro_rules! walk_validated_body {")
        .expect("walk_validated_body! definition");
    let end = (start + 1..lines.len())
        .find(|&i| lines[i] == "}")
        .expect("end of walk_validated_body!");
    lines[start..end].to_vec()
}

/// Every `return Ok(` in `body` that is not validated on its path. Walking
/// back from the return, the first validation must come before any shared
/// load and before any match arm (`=>`) other than the return's own line.
fn unvalidated_returns(body: &[&str]) -> Vec<String> {
    let mut bad = Vec::new();
    for (i, line) in body.iter().enumerate() {
        if !code(line).contains("return Ok(") {
            continue;
        }
        let mut verdict = Err("no validation before it in the macro");
        for j in (0..i).rev() {
            let c = code(body[j]);
            if VALIDATIONS.iter().any(|v| c.contains(v)) {
                verdict = Ok(());
                break;
            }
            if SHARED_LOADS.iter().any(|l| c.contains(l)) {
                verdict = Err("a shared load comes after its last validation");
                break;
            }
            if c.contains("=>") {
                verdict = Err("its match arm has no validation before it");
                break;
            }
        }
        if let Err(why) = verdict {
            bad.push(format!(
                "walk_validated_body! line {}: {} -- {why}",
                i + 1,
                line.trim()
            ));
        }
    }
    bad
}

fn walk_return_sites(body: &[&str]) -> usize {
    body.iter()
        .filter(|l| code(l).contains("return Ok("))
        .count()
}

/// The lines of `fn <name>` in `src`, from its signature to the first line
/// that is a bare `}` at column 0.
fn fn_body<'a>(src: &'a str, name: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = src.lines().collect();
    let sig = format!("fn {name}<");
    let start = lines
        .iter()
        .position(|l| code(l).contains(&sig))
        .unwrap_or_else(|| panic!("fn {name}"));
    let end = (start + 1..lines.len())
        .find(|&i| lines[i] == "}")
        .unwrap_or_else(|| panic!("end of fn {name}"));
    lines[start..=end].to_vec()
}

/// What is wrong with an ordered-read entry point's answer: it must return
/// `Ok(found)` once, as its last expression, and the last check before it
/// must validate both the retained read set and the tree version.
fn ordered_entry_faults(body: &[&str], name: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let answers: Vec<usize> = body
        .iter()
        .enumerate()
        .filter(|(_, l)| code(l).contains("Ok("))
        .map(|(i, _)| i)
        .collect();
    if answers.len() != 1 || code(body[answers[0]]).trim() != "Ok(found)" {
        bad.push(format!(
            "{name}: expected exactly one `Ok(found)`, found {answers:?}"
        ));
        return bad;
    }
    let check = (0..answers[0])
        .rev()
        .find(|&j| code(body[j]).contains("validate"))
        .map(|j| code(body[j]));
    match check {
        Some(c) => {
            if !c.contains("rs.validate_all()") {
                bad.push(format!(
                    "{name}: the final check does not validate the read set"
                ));
            }
            if !c.contains("ver.validate(snap)") {
                bad.push(format!(
                    "{name}: the final check does not validate the tree version"
                ));
            }
        }
        None => bad.push(format!("{name}: no validation before its answer")),
    }
    bad
}

/// `node_sample(` call sites in `src` outside `use` lines. Every branch
/// version an ordered search samples must be retained, so the one call is the
/// one inside `ReadSet::sample`.
fn node_sample_calls(src: &str) -> usize {
    src.lines()
        .map(code)
        .filter(|c| c.contains("node_sample(") && !c.trim_start().starts_with("use "))
        .count()
}

#[test]
fn walk_validated_answers_are_validated() {
    let body = walk_body(SYNC);
    assert_eq!(
        walk_return_sites(&body),
        WALK_RETURN_SITES,
        "the number of answer sites in walk_validated_body! changed: check each new one against \
         the rule in docs/ARCHITECTURE.md §4.1, then update WALK_RETURN_SITES"
    );
    let bad = unvalidated_returns(&body);
    assert!(bad.is_empty(), "unvalidated answers:\n{}", bad.join("\n"));
}

#[test]
fn ordered_read_answers_are_validated() {
    let mut bad = Vec::new();
    for name in ["next_validated", "prev_validated"] {
        bad.extend(ordered_entry_faults(&fn_body(SYNC_NAV, name), name));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert_eq!(
        node_sample_calls(SYNC_NAV),
        1,
        "sync_nav.rs samples a branch version outside ReadSet::sample, so the final validation \
         would not see it"
    );
}

/// The scanners on synthetic input: each clause turns red on its own.
#[test]
fn scanners_catch_each_fault() {
    let good = [
        "    chk!();",
        "    let k = unsafe { shared_word::load::<true>(p) };",
        "    chk!();",
        "    return Ok(Some(k));",
    ];
    assert!(unvalidated_returns(&good).is_empty());
    // A load between the last validation and the answer.
    let late_load = [good[0], good[1], good[3]];
    assert_eq!(unvalidated_returns(&late_load).len(), 1);
    // The validation is in another arm.
    let other_arm = [
        "    A => { chk!(); }",
        "    B => {",
        "        return Ok(None);",
    ];
    assert_eq!(unvalidated_returns(&other_arm).len(), 1);
    // No validation at all; a validation in a comment does not count.
    let none = ["    // chk!();", "    return Ok(None);"];
    assert_eq!(unvalidated_returns(&none).len(), 1);

    let entry = |check: &str| {
        vec![
            "fn next_validated<".to_string(),
            format!("    if {check} {{"),
            "        return Err(Retry);".to_string(),
            "    }".to_string(),
            "    Ok(found)".to_string(),
            "}".to_string(),
        ]
    };
    let full = entry("!unsafe { rs.validate_all() } || !ver.validate(snap)");
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    assert!(ordered_entry_faults(&full, "x").is_empty());
    for partial in ["!unsafe { rs.validate_all() }", "!ver.validate(snap)"] {
        let e = entry(partial);
        let e: Vec<&str> = e.iter().map(String::as_str).collect();
        assert_eq!(ordered_entry_faults(&e, "x").len(), 1, "{partial}");
    }
    assert_eq!(
        node_sample_calls(
            "use crate::occ::node_sample;\nlet s = node_sample(v);\n// node_sample(x)"
        ),
        1
    );
}
