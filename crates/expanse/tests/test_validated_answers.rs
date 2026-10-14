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
//! - in `Shared::optimistic_read` (`src/sync.rs`), the tree version is sampled
//!   once, before the root is loaded (#1194, G36.4);
//! - in the two `BranchB` arms of `src/sync_nav.rs`, the branch's version is
//!   sampled once, before any load from the branch (#1194, G36.4: the row
//!   whose registered mutation cannot return a wrong key on a shared set,
//!   `sync::ordered_read_tests::set_ordered_read_past_a_removed_bitmap_branch_digit_restarts`).
//!
//! - outside test modules, the set form of the ordered walk (`MAP = false`)
//!   is instantiated in `set_ordered_walk!` (`src/sync.rs`) and nowhere else,
//!   and the map form (`MAP = true`) at the map's two wrappers and in
//!   `sync_nav::seek_submap` and nowhere else (#1194, G36.7).
//!
//! - in the two `FullExpanse` arms of `src/sync_nav.rs`, the set form
//!   answers from the suffix only after checking that the edge fills the
//!   level it is read at, and answers `Err(Retry)` otherwise (#1194, §36.3
//!   C6). Debug builds panic at that check, so a debug-build test cannot
//!   observe the retry; the order of the statements is pinned here.
//!
//! shown to fail on its own when the code it guards was broken (PR body of
//! the change that added this file).
#![cfg(not(miri))]

use std::sync::LazyLock;

/// The sources as checked out. Read only through [`SYNC`] and [`SYNC_NAV`].
const SYNC_RAW: &str = include_str!("../src/sync.rs");
const SYNC_NAV_RAW: &str = include_str!("../src/sync_nav.rs");

/// `src` with CRLF line endings turned into LF. A Windows checkout gives the
/// included sources CRLF endings, and every scanner and edit in this file
/// matches multi-line text and bare `}` lines written with `\n` alone, so
/// each source is normalized once, where it is loaded.
fn normalized(src: &str) -> String {
    src.replace("\r\n", "\n")
}

/// `src/sync.rs`, normalized.
static SYNC: LazyLock<String> = LazyLock::new(|| normalized(SYNC_RAW));
/// `src/sync_nav.rs`, normalized.
static SYNC_NAV: LazyLock<String> = LazyLock::new(|| normalized(SYNC_NAV_RAW));

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
    let body = walk_body(&SYNC);
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
        bad.extend(ordered_entry_faults(&fn_body(&SYNC_NAV, name), name));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert_eq!(
        node_sample_calls(&SYNC_NAV),
        1,
        "sync_nav.rs samples a branch version outside ReadSet::sample, so the final validation \
         would not see it"
    );
}

/// `src` with its comments and the contents of its string and character
/// literals removed, and its line structure kept. The sample-order scanners
/// read this, so a pattern inside a `//` comment, a `/* */` comment or a
/// literal is not counted as code.
fn bare(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let at = |i: usize| b.get(i).copied();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == '/' && at(i + 1) == Some('/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && at(i + 1) == Some('*') {
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '/' && at(i + 1) == Some('*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && at(i + 1) == Some('/') {
                    depth -= 1;
                    i += 2;
                } else {
                    if b[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
        } else if c == '"' {
            // A string literal, or the quoted part of a raw one whose `r`
            // and hashes were copied as code: close on the same hashes.
            let mut hashes = 0;
            while hashes < i && b[i - 1 - hashes] == '#' {
                hashes += 1;
            }
            let raw = hashes < i && b[i - 1 - hashes] == 'r' || hashes > 0;
            out.push('"');
            i += 1;
            while i < b.len() {
                if !raw && b[i] == '\\' {
                    i += 1;
                } else if b[i] == '"' && (0..hashes).all(|h| at(i + 1 + h) == Some('#')) {
                    break;
                }
                if at(i) == Some('\n') {
                    out.push('\n');
                }
                i += 1;
            }
            out.push('"');
            i += 1;
        } else if c == '\'' && at(i + 1) == Some('\\') {
            // An escaped character literal.
            i += 2;
            while i < b.len() && b[i] != '\'' {
                i += 1;
            }
            out.push_str("' '");
            i += 1;
        } else if c == '\'' && at(i + 2) == Some('\'') {
            // A one-character literal; a lifetime has no closing quote.
            out.push_str("' '");
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// The body of `Shared::optimistic_read` in bare source: from its signature
/// to the first line that closes a method (`    }`).
fn optimistic_read_body(src: &str) -> Vec<&str> {
    let lines: Vec<&str> = src.lines().collect();
    let start = lines
        .iter()
        .position(|l| code(l).trim() == "fn optimistic_read<R>(")
        .expect("Shared::optimistic_read");
    let end = (start + 1..lines.len())
        .find(|&i| lines[i] == "    }")
        .expect("end of Shared::optimistic_read");
    lines[start..=end].to_vec()
}

/// `lines` without the statements a `#[cfg(test)]` attribute gates: the
/// attribute's line and what follows it, up to the first line whose code
/// ends a statement. The negative controls in the walk are such statements,
/// and they load before the sample on purpose.
fn without_test_statements<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut gated = false;
    for &l in lines {
        let c = code(l).trim();
        if gated {
            gated = !c.ends_with(';');
            continue;
        }
        if c == "#[cfg(test)]" {
            gated = true;
            continue;
        }
        out.push(l);
    }
    out
}

/// What is wrong with the order of a version sample and the loads it
/// covers, in `body`: there must be exactly one line matching `sample`, at
/// least one matching a pattern of `loads`, and no such load above the
/// sample.
fn sample_order_faults(body: &[&str], sample: &str, loads: &[&str], name: &str) -> Vec<String> {
    let at = |pats: &[&str]| -> Vec<usize> {
        body.iter()
            .enumerate()
            .filter(|(_, l)| pats.iter().any(|p| code(l).contains(p)))
            .map(|(i, _)| i)
            .collect()
    };
    let (samples, loads) = (at(&[sample]), at(loads));
    let mut bad = Vec::new();
    if samples.len() != 1 {
        bad.push(format!(
            "{name}: expected exactly one `{sample}`, found {}",
            samples.len()
        ));
    }
    if loads.is_empty() {
        bad.push(format!(
            "{name}: no load found; the scanner's patterns no longer match"
        ));
    }
    if let (Some(&s), Some(&l)) = (samples.first(), loads.first())
        && l < s
    {
        bad.push(format!(
            "{name}: `{}` comes before the version sample",
            code(body[l]).trim()
        ));
    }
    bad
}

/// How `Shared::optimistic_read` samples the tree version and loads the root.
const TREE_SAMPLE: &str = "self.version().sample()";
const ROOT_LOADS: &[&str] = &["self.published().load()", "root_of("];

/// How a `BranchB` arm of `sync_nav` samples the branch and loads from it.
///
/// The scanner recognises these spellings and no others: the sample as
/// `rs.sample(vp)`, and a load from the branch as a read of
/// `(*node).level`, a call into `shared_bitmap::`, a call of
/// `branch_b_child(` or an `Edge::load_at`. It reads source order, not
/// behaviour: a load written another way above the sample (through a new
/// helper, a different projection of `node`, a macro) passes, and
/// `sample_order_scanners_turn_red_on_the_real_source` pins one such case
/// as a stated limit (`docs/benchmarks/concurrency/METHODOLOGY.md` §36.13,
/// item 5).
const BRANCH_SAMPLE: &str = "rs.sample(vp)";
const BRANCH_B_LOADS: &[&str] = &[
    "(*node).level",
    "shared_bitmap::",
    "branch_b_child(",
    "Edge::load_at",
];

/// The `BranchB` match arms of `src`: from each arm's first line to the line
/// before the next arm at the same depth.
fn branch_b_arms(src: &str) -> Vec<Vec<&str>> {
    let lines: Vec<&str> = src.lines().collect();
    let is_arm = |l: &str| l.starts_with("        EdgeTag::") && code(l).contains("=>");
    (0..lines.len())
        .filter(|&i| is_arm(lines[i]) && code(lines[i]).contains("EdgeType::BranchB)"))
        .map(|i| {
            let end = (i + 1..lines.len())
                .find(|&j| is_arm(lines[j]) || lines[j] == "    }")
                .expect("end of a BranchB arm");
            lines[i..end].to_vec()
        })
        .collect()
}

/// The faults of `Shared::optimistic_read` in `sync_src`.
fn optimistic_read_faults(sync_src: &str) -> Vec<String> {
    let src = bare(sync_src);
    let body = optimistic_read_body(&src);
    sample_order_faults(&body, TREE_SAMPLE, ROOT_LOADS, "optimistic_read")
}

/// The faults of the `BranchB` arms in `nav_src`: there must be two arms,
/// and each must sample once, above every load it makes from the branch.
fn branch_b_faults(nav_src: &str) -> Vec<String> {
    let src = bare(nav_src);
    let arms = branch_b_arms(&src);
    let mut bad = Vec::new();
    if arms.len() != 2 {
        bad.push(format!(
            "expected one BranchB arm in next_in and one in prev_in, found {}",
            arms.len()
        ));
    }
    for (i, arm) in arms.iter().enumerate() {
        let arm = without_test_statements(arm);
        bad.extend(sample_order_faults(
            &arm,
            BRANCH_SAMPLE,
            BRANCH_B_LOADS,
            &format!("BranchB arm {i}"),
        ));
    }
    bad
}

/// #1194, G36.4, the root row: an optimistic read samples the tree version
/// before it loads the root it then validates against that sample.
#[test]
fn optimistic_read_samples_the_tree_version_before_the_root() {
    let bad = optimistic_read_faults(&SYNC);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// #1194, G36.4, the `BranchB` row: both arms sample the branch's version
/// before any load from the branch.
#[test]
fn bitmap_branch_arms_sample_before_they_load() {
    let bad = branch_b_faults(&SYNC_NAV);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// `src` with the first `old` after the first `anchor` replaced by `new`.
fn edited_after(src: &str, anchor: &str, old: &str, new: &str) -> String {
    let a = src
        .find(anchor)
        .unwrap_or_else(|| panic!("anchor {anchor:?}"));
    let at = a + src[a..]
        .find(old)
        .unwrap_or_else(|| panic!("{old:?} after {anchor:?}"));
    format!("{}{new}{}", &src[..at], &src[at + old.len()..])
}

/// The two scanners against edited copies of the real source, held in
/// memory: the sample commented out both ways, a load moved above it, and a
/// second sample. Each edit yields the one fault of its clause. The edits
/// are made here and not on disk because a tree with the sample commented
/// out does not compile, so no test of it could run.
#[test]
fn sample_order_scanners_turn_red_on_the_real_source() {
    const ARM: &str = "EdgeTag::Structural(EdgeType::BranchB) => {";
    const SAMPLE: &str = "let nsnap = unsafe { rs.sample(vp)? };";
    let one = |faults: Vec<String>, clause: &str| {
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(faults[0].contains(clause), "{faults:?}");
    };
    assert!(branch_b_faults(&SYNC_NAV).is_empty());

    // The sample commented out: with `//`, and with `/* */`.
    for commented in [format!("// {SAMPLE}"), format!("/* {SAMPLE} */")] {
        let src = edited_after(&SYNC_NAV, ARM, SAMPLE, &commented);
        one(branch_b_faults(&src), "BranchB arm 0: expected exactly one");
    }
    // Sampled twice.
    let twice = format!("{SAMPLE}\n            let _again = unsafe {{ rs.sample(vp)? }};");
    let src = edited_after(&SYNC_NAV, ARM, SAMPLE, &twice);
    one(branch_b_faults(&src), "BranchB arm 0: expected exactly one");
    // A load above the sample, in each recognised spelling.
    for load in [
        "let _early = unsafe { (*node).level };",
        "let _early = unsafe { crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, 0) };",
        "let _early = unsafe { branch_b_child(node, 0, holder) };",
        "let _early = unsafe { Edge::load_at::<true>(core::ptr::null()) };",
    ] {
        let src = edited_after(
            &SYNC_NAV,
            ARM,
            SAMPLE,
            &format!("{load}\n            {SAMPLE}"),
        );
        one(branch_b_faults(&src), "comes before the version sample");
    }
    // The same text inside a comment or a string literal is not a load.
    for inert in [
        "/* let _early = unsafe { (*node).level }; */",
        "let _text = \"(*node).level shared_bitmap:: branch_b_child( Edge::load_at\";",
    ] {
        let src = edited_after(
            &SYNC_NAV,
            ARM,
            SAMPLE,
            &format!("{inert}\n            {SAMPLE}"),
        );
        assert!(branch_b_faults(&src).is_empty(), "{inert}");
    }
    // The stated limit: an early load in a spelling the scanner does not
    // know passes. This assertion records the limit; it is not a property
    // to keep.
    let unseen = "let _early = unsafe { node.cast::<u8>().add(4).read() };";
    let src = edited_after(
        &SYNC_NAV,
        ARM,
        SAMPLE,
        &format!("{unseen}\n            {SAMPLE}"),
    );
    assert!(branch_b_faults(&src).is_empty());
    // An arm removed: the count of arms is its own clause.
    let src = edited_after(&SYNC_NAV, ARM, "EdgeType::BranchB)", "EdgeType::BranchC)");
    one(branch_b_faults(&src), "expected one BranchB arm in next_in");

    // `optimistic_read`: the sample commented out, and the root loaded first.
    const READ: &str = "fn optimistic_read<R>(";
    const TREE: &str = "let snap = self.version().sample();";
    assert!(optimistic_read_faults(&SYNC).is_empty());
    for commented in [format!("// {TREE}"), format!("/* {TREE} */")] {
        let src = edited_after(&SYNC, READ, TREE, &commented);
        one(optimistic_read_faults(&src), "expected exactly one");
    }
    let early = format!("let _early = self.published().load();\n            {TREE}");
    let src = edited_after(&SYNC, READ, TREE, &early);
    one(
        optimistic_read_faults(&src),
        "comes before the version sample",
    );
}

/// The sample-order scanners on synthetic input: each clause turns red on
/// its own, a comment does not count as code, and a `#[cfg(test)]`
/// statement is not read as the walk's own load.
#[test]
fn sample_order_scanners_catch_each_fault() {
    let good = [
        "    let snap = self.version().sample();",
        "    let root = self.published().load();",
    ];
    assert!(sample_order_faults(&good, TREE_SAMPLE, ROOT_LOADS, "x").is_empty());
    // The root loaded first.
    let swapped = [good[1], good[0]];
    let bad = sample_order_faults(&swapped, TREE_SAMPLE, ROOT_LOADS, "x");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].contains("comes before the version sample"));
    // The sample commented out, or made twice.
    let commented = ["    // let snap = self.version().sample();", good[1]];
    let bad = sample_order_faults(&commented, TREE_SAMPLE, ROOT_LOADS, "x");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].contains("expected exactly one"));
    let twice = [good[0], good[1], good[0]];
    let bad = sample_order_faults(&twice, TREE_SAMPLE, ROOT_LOADS, "x");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].contains("expected exactly one"));
    // No load the patterns recognise.
    let no_load = [good[0], "    let root = self.fetch_root();"];
    let bad = sample_order_faults(&no_load, TREE_SAMPLE, ROOT_LOADS, "x");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].contains("no load found"));

    // A `#[cfg(test)]` statement is dropped whole; the code after it stays.
    let arm = [
        "            #[cfg(test)]",
        "            let early = switch()",
        "                .then(|| unsafe { (*node).level });",
        "            let nsnap = unsafe { rs.sample(vp)? };",
        "            let bl = unsafe { (*node).level };",
    ];
    let kept = without_test_statements(&arm);
    assert_eq!(kept, &arm[3..]);
    assert!(sample_order_faults(&kept, BRANCH_SAMPLE, BRANCH_B_LOADS, "x").is_empty());
    // The same early load without the attribute is a fault.
    let ungated = without_test_statements(&arm[1..]);
    assert_eq!(
        sample_order_faults(&ungated, BRANCH_SAMPLE, BRANCH_B_LOADS, "x").len(),
        1
    );

    // Comments of both kinds and literal contents are removed, lines kept.
    let text =
        "a(); // b()\n/* c()\n d() */ e(\"f() \\\" g()\", '\"', r#\"h()\"#);\nlet x: &'a str;\n";
    assert_eq!(
        bare(text),
        "a(); \n\n e(\"\", ' ', r#\"\"#);\nlet x: &'a str;\n"
    );

    // An arm is cut at the next arm, and only `BranchB` arms are taken.
    let src = "        EdgeTag::Structural(EdgeType::BranchB) => {\n            a();\n        }\n\n        EdgeTag::Structural(EdgeType::BranchU) => {\n            b();\n        }\n    }\n";
    let arms = branch_b_arms(src);
    assert_eq!(arms.len(), 1);
    assert_eq!(arms[0].len(), 4);
}

// ---- Where each flavour of the ordered walk is instantiated (#1194, G36.7) ----

/// `src` in bare form (see [`bare`]) without the bodies of its test modules:
/// every `mod` item under a `#[cfg(...)]` that names `test` or `loom`, from
/// its opening line to its closing brace. Line structure is kept.
fn without_test_modules(src: &str) -> String {
    let bare_src = bare(src);
    let lines: Vec<&str> = bare_src.lines().collect();
    let is_mod = |l: &str| {
        let t = l.trim();
        t.ends_with('{')
            && ["mod ", "pub mod ", "pub(crate) mod ", "pub(super) mod "]
                .iter()
                .any(|p| t.starts_with(p))
    };
    let gated = |i: usize| {
        lines[..i]
            .iter()
            .rev()
            .take_while(|l| l.trim().starts_with("#["))
            .any(|l| l.contains("cfg(") && (l.contains("test") || l.contains("loom")))
    };
    let mut out = String::with_capacity(bare_src.len());
    let mut i = 0;
    while i < lines.len() {
        if is_mod(lines[i]) && gated(i) {
            let mut depth = 0i64;
            loop {
                depth += lines[i].matches('{').count() as i64;
                depth -= lines[i].matches('}').count() as i64;
                out.push('\n');
                i += 1;
                if depth <= 0 || i >= lines.len() {
                    break;
                }
            }
        } else {
            out.push_str(lines[i]);
            out.push('\n');
            i += 1;
        }
    }
    out
}

/// The line range of an item of `lines`: from the first line containing
/// `opening` to the first later line that is a bare `}` at column 0.
fn item_range(lines: &[&str], opening: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|l| l.contains(opening))?;
    let end = (start + 1..lines.len()).find(|&i| lines[i] == "}")?;
    Some((start, end))
}

/// The lines of `lines` on which `call`, written with the const argument
/// `flavour` (`true` or `false`, bare or in braces), occurs. Whitespace is
/// ignored, so a spelling rustfmt would rewrite is still seen.
fn flavour_sites(lines: &[&str], call: &str, flavour: &str) -> Vec<usize> {
    let pats = [
        format!("{call}::<{flavour}>"),
        format!("{call}::<{flavour},"),
        format!("{call}::<{{{flavour}}}>"),
        format!("{call}::<{{{flavour}}},"),
    ];
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let tight: String = l.chars().filter(|c| !c.is_whitespace()).collect();
            pats.iter().any(|p| tight.contains(p.as_str()))
        })
        .map(|(i, _)| i)
        .collect()
}

/// The two entry points of the ordered walk, and the functions under them
/// that take the flavour.
const WALK_ENTRIES: [&str; 2] = ["next_validated", "prev_validated"];
const WALK_INNER: [&str; 4] = ["next_in", "prev_in", "root_leaf", "bitmap_leaf"];

/// G36.7, clause 1: outside test modules, `next_validated::<false>` and
/// `prev_validated::<false>` each occur once, inside `set_ordered_walk!`,
/// and no function under them is named with a literal `false`. `others` are
/// the remaining source files, by name.
fn set_form_faults(sync_src: &str, nav_src: &str, others: &[(String, String)]) -> Vec<String> {
    let sync = without_test_modules(sync_src);
    let sync: Vec<&str> = sync.lines().collect();
    let mut bad = Vec::new();
    let Some((lo, hi)) = item_range(&sync, "macro_rules! set_ordered_walk {") else {
        return vec!["set form: `set_ordered_walk!` not found in sync.rs".to_string()];
    };
    for call in WALK_ENTRIES {
        let sites = flavour_sites(&sync, call, "false");
        if sites.len() != 1 {
            bad.push(format!(
                "set form: expected `{call}::<false>` once in sync.rs outside tests, found {}",
                sites.len()
            ));
        }
        for at in sites {
            if !(lo..=hi).contains(&at) {
                bad.push(format!(
                    "set form: `{call}::<false>` outside `set_ordered_walk!` (sync.rs line {})",
                    at + 1
                ));
            }
        }
    }
    let nav = without_test_modules(nav_src);
    let nav: Vec<&str> = nav.lines().collect();
    let mut elsewhere: Vec<(&str, Vec<&str>)> = vec![("sync_nav.rs", nav)];
    let stripped: Vec<(String, String)> = others
        .iter()
        .map(|(n, src)| (n.clone(), without_test_modules(src)))
        .collect();
    for (n, src) in &stripped {
        elsewhere.push((n.as_str(), src.lines().collect()));
    }
    for (name, lines) in &elsewhere {
        for call in WALK_ENTRIES.iter().chain(&WALK_INNER) {
            for at in flavour_sites(lines, call, "false") {
                bad.push(format!(
                    "set form: `{call}::<false>` in {name} (line {})",
                    at + 1
                ));
            }
        }
    }
    for call in WALK_INNER {
        for at in flavour_sites(&sync, call, "false") {
            bad.push(format!(
                "set form: `{call}::<false>` in sync.rs (line {})",
                at + 1
            ));
        }
    }
    bad
}

/// G36.7, clause 2: outside test modules, `next_validated::<true>` occurs
/// once, in `map_next_with`, and `prev_validated::<true>` once, in
/// `map_prev_with`; a function under them is named with a literal `true` in
/// `sync_nav::seek_submap` only.
fn map_form_faults(sync_src: &str, nav_src: &str, others: &[(String, String)]) -> Vec<String> {
    let sync = without_test_modules(sync_src);
    let sync: Vec<&str> = sync.lines().collect();
    let mut bad = Vec::new();
    for (call, home) in WALK_ENTRIES
        .iter()
        .zip(["fn map_next_with(", "fn map_prev_with("])
    {
        let sites = flavour_sites(&sync, call, "true");
        if sites.len() != 1 {
            bad.push(format!(
                "map form: expected `{call}::<true>` once in sync.rs outside tests, found {}",
                sites.len()
            ));
        }
        let range = item_range(&sync, home);
        for at in sites {
            if !range.is_some_and(|(lo, hi)| (lo..=hi).contains(&at)) {
                bad.push(format!(
                    "map form: `{call}::<true>` outside `{home}..)` (sync.rs line {})",
                    at + 1
                ));
            }
        }
    }
    for call in WALK_INNER {
        for at in flavour_sites(&sync, call, "true") {
            bad.push(format!(
                "map form: `{call}::<true>` in sync.rs (line {})",
                at + 1
            ));
        }
    }
    let nav = without_test_modules(nav_src);
    let nav: Vec<&str> = nav.lines().collect();
    let seek = item_range(&nav, "unsafe fn seek_submap<");
    if seek.is_none() {
        bad.push("map form: `seek_submap` not found in sync_nav.rs".to_string());
    }
    for call in WALK_ENTRIES.iter().chain(&WALK_INNER) {
        for at in flavour_sites(&nav, call, "true") {
            if !seek.is_some_and(|(lo, hi)| (lo..=hi).contains(&at)) {
                bad.push(format!(
                    "map form: `{call}::<true>` outside `seek_submap` (sync_nav.rs line {})",
                    at + 1
                ));
            }
        }
    }
    for (name, src) in others {
        let src = without_test_modules(src);
        let lines: Vec<&str> = src.lines().collect();
        for call in WALK_ENTRIES.iter().chain(&WALK_INNER) {
            for at in flavour_sites(&lines, call, "true") {
                bad.push(format!(
                    "map form: `{call}::<true>` in {name} (line {})",
                    at + 1
                ));
            }
        }
    }
    bad
}

/// Every source file of the crate but `sync.rs` and `sync_nav.rs`, read
/// from disk so a file added later is scanned without being listed here.
fn other_sources() -> Vec<(String, String)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("the crate's src directory") {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a UTF-8 file name")
            .to_string();
        if name.ends_with(".rs") && name != "sync.rs" && name != "sync_nav.rs" {
            let src = normalized(&std::fs::read_to_string(&path).expect("a readable source file"));
            out.push((name, src));
        }
    }
    assert!(
        out.len() >= 30,
        "expected the crate's other source files, found {}",
        out.len()
    );
    out
}

/// #1194, G36.7: the set form of the ordered walk is instantiated in one
/// place, and the map form where it was.
#[test]
fn ordered_walk_flavours_are_instantiated_where_registered() {
    let others = other_sources();
    let bad = set_form_faults(&SYNC, &SYNC_NAV, &others);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    let bad = map_form_faults(&SYNC, &SYNC_NAV, &others);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The two clauses of G36.7 against edited copies of the real source, held
/// in memory: each clause turns red under its own mutation, and under the
/// two mutations the gate names (the set macro's constant flipped; a
/// `::<false>` on a map path), which break both clauses at once.
#[test]
fn flavour_scanner_turns_red_on_the_real_source() {
    const SET_MACRO: &str = "macro_rules! set_ordered_walk {";
    const SET_NEXT: &str = "crate::sync_nav::next_validated::<false>(root, key, ver, snap)";
    const MAP_NEXT: &str = "crate::sync_nav::next_validated::<true>(root, key, ver, snap)";
    const MAP_FN: &str = "fn map_next_with(";
    let none: Vec<(String, String)> = Vec::new();
    let set = |sync: &str, nav: &str| set_form_faults(sync, nav, &none);
    let map = |sync: &str, nav: &str| map_form_faults(sync, nav, &none);
    assert!(set(&SYNC, &SYNC_NAV).is_empty());
    assert!(map(&SYNC, &SYNC_NAV).is_empty());

    // Clause 1 alone: a second set-form call, outside the macro, in a
    // non-test function. The map form is untouched.
    let extra = format!("fn stray() {{ let _ = {SET_NEXT}; }}\n\n{MAP_FN}");
    let src = edited_after(
        &SYNC,
        "/// The smallest entry with key `>= key`",
        MAP_FN,
        &extra,
    );
    let faults = set(&src, &SYNC_NAV);
    assert_eq!(faults.len(), 2, "{faults:?}");
    assert!(
        faults[0].contains("expected `next_validated::<false>` once"),
        "{faults:?}"
    );
    assert!(
        faults[1].contains("outside `set_ordered_walk!`"),
        "{faults:?}"
    );
    assert!(map(&src, &SYNC_NAV).is_empty());
    // The same call in a test module, in a comment or in a string is not a site.
    let in_tests =
        format!("#[cfg(test)]\nmod extra_tests {{\n    fn f() {{ let _ = {SET_NEXT}; }}\n}}\n");
    for inert in [
        in_tests,
        format!("// {SET_NEXT}\n"),
        format!("const _TEXT: &str = \"{SET_NEXT}\";\n"),
    ] {
        let src = format!("{}\n{inert}", *SYNC);
        assert!(set(&src, &SYNC_NAV).is_empty(), "{inert}");
        assert!(map(&src, &SYNC_NAV).is_empty(), "{inert}");
    }
    // Clause 1 alone, through an inner function of the walk.
    let src = edited_after(
        &SYNC_NAV,
        "pub(crate) unsafe fn next_validated<",
        "root_leaf::<MAP>",
        "root_leaf::<false>",
    );
    let faults = set(&SYNC, &src);
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("`root_leaf::<false>` in sync_nav.rs"),
        "{faults:?}"
    );
    assert!(map(&SYNC, &src).is_empty());

    // Clause 2 alone: a second map-form call outside the map's wrappers.
    let extra = format!("fn stray() {{ let _ = {MAP_NEXT}; }}\n\n{MAP_FN}");
    let src = edited_after(
        &SYNC,
        "/// The smallest entry with key `>= key`",
        MAP_FN,
        &extra,
    );
    let faults = map(&src, &SYNC_NAV);
    assert_eq!(faults.len(), 2, "{faults:?}");
    assert!(
        faults[0].contains("expected `next_validated::<true>` once"),
        "{faults:?}"
    );
    assert!(
        faults[1].contains("outside `fn map_next_with(..)`"),
        "{faults:?}"
    );
    assert!(set(&src, &SYNC_NAV).is_empty());
    // Clause 2 alone, in `sync_nav`: a literal `true` outside `seek_submap`.
    let src = edited_after(
        &SYNC_NAV,
        "pub(crate) unsafe fn prev_validated<",
        "root_leaf::<MAP>",
        "root_leaf::<true>",
    );
    let faults = map(&SYNC, &src);
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("`root_leaf::<true>` outside `seek_submap`"),
        "{faults:?}"
    );
    assert!(set(&SYNC, &src).is_empty());
    // Clause 2 alone, in a third file.
    let third = vec![(
        "strmap.rs".to_string(),
        format!("fn f() {{ let _ = {MAP_NEXT}; }}\n"),
    )];
    let faults = map_form_faults(&SYNC, &SYNC_NAV, &third);
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(faults[0].contains("in strmap.rs"), "{faults:?}");
    assert!(set_form_faults(&SYNC, &SYNC_NAV, &third).is_empty());

    // The gate's first mutation: the set macro's constant flipped. The set
    // form is gone from its macro, and a map-form call sits outside the
    // map's wrappers.
    let src = edited_after(
        &SYNC,
        SET_MACRO,
        "next_validated::<false>",
        "next_validated::<true>",
    );
    let faults = set(&src, &SYNC_NAV);
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("expected `next_validated::<false>` once"),
        "{faults:?}"
    );
    let faults = map(&src, &SYNC_NAV);
    assert_eq!(faults.len(), 2, "{faults:?}");
    assert!(
        faults[1].contains("outside `fn map_next_with(..)`"),
        "{faults:?}"
    );
    // The gate's second mutation: a `::<false>` on a map path.
    let src = edited_after(
        &SYNC,
        MAP_FN,
        "next_validated::<true>",
        "next_validated::<false>",
    );
    let faults = set(&src, &SYNC_NAV);
    assert_eq!(faults.len(), 2, "{faults:?}");
    assert!(
        faults[1].contains("outside `set_ordered_walk!`"),
        "{faults:?}"
    );
    let faults = map(&src, &SYNC_NAV);
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("expected `next_validated::<true>` once"),
        "{faults:?}"
    );
}

// ---- The full-expanse arm fails safe (#1194, §36.3 C6) ----

/// The `FullExpanse` match arms of `src` in bare form: from each arm's first
/// line to the line before the next arm at the same depth.
fn full_expanse_arms(src: &str) -> Vec<Vec<String>> {
    let bare_src = bare(src);
    let lines: Vec<&str> = bare_src.lines().collect();
    let mut arms = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.trim() == "EdgeTag::Structural(EdgeType::FullExpanse) => {" {
            let indent = l.len() - l.trim_start().len();
            let end = (i + 1..lines.len())
                .find(|&j| {
                    let t = lines[j];
                    !t.trim().is_empty()
                        && t.len() - t.trim_start().len() == indent
                        && t.trim() == "}"
                })
                .unwrap_or(lines.len() - 1);
            arms.push(lines[i..=end].iter().map(|s| s.to_string()).collect());
        }
    }
    arms
}

/// What is wrong with the full-expanse arms of `nav_src`: there must be two;
/// each must test `!full_expanse_fills(edge, level)` exactly once, outside
/// any `#[cfg]`; a `return Err(Retry);` must follow that test within four
/// lines; and the arm's answer (`Ok(Some((suffix`) must come after both.
fn full_expanse_faults(nav_src: &str) -> Vec<String> {
    const GUARD: &str = "if !full_expanse_fills(edge, level) {";
    const RETRY: &str = "return Err(Retry);";
    const ANSWER: &str = "Ok(Some((suffix";
    let arms = full_expanse_arms(nav_src);
    let mut bad = Vec::new();
    if arms.len() != 2 {
        bad.push(format!(
            "expected two FullExpanse arms (next_in, prev_in), found {}",
            arms.len()
        ));
    }
    for (n, arm) in arms.iter().enumerate() {
        let at = |pat: &str| -> Vec<usize> {
            arm.iter()
                .enumerate()
                .filter(|(_, l)| l.contains(pat))
                .map(|(i, _)| i)
                .collect()
        };
        let (guards, answers) = (at(GUARD), at(ANSWER));
        if guards.len() != 1 {
            bad.push(format!(
                "FullExpanse arm {n}: expected the level check once, found {}",
                guards.len()
            ));
            continue;
        }
        let g = guards[0];
        if g > 0 && arm[g - 1].trim().starts_with("#[cfg") {
            bad.push(format!(
                "FullExpanse arm {n}: the level check is behind a cfg attribute"
            ));
        }
        // The retry that belongs to the check: within four lines of it.
        if !at(RETRY).iter().any(|&r| r > g && r <= g + 4) {
            bad.push(format!(
                "FullExpanse arm {n}: no `return Err(Retry);` within four lines of the level check"
            ));
        }
        match answers.as_slice() {
            [a] if *a > g => {}
            [_] => bad.push(format!(
                "FullExpanse arm {n}: the arm answers before the level check"
            )),
            other => bad.push(format!(
                "FullExpanse arm {n}: expected one answer from the suffix, found {}",
                other.len()
            )),
        }
    }
    bad
}

/// #1194, §36.3 C6: the set form's full-expanse arms check the level before
/// they answer, and retry when the check fails.
#[test]
fn full_expanse_arms_fail_safe() {
    let bad = full_expanse_faults(&SYNC_NAV);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The scanner against edited copies of the real source, one clause per
/// edit: the check removed, the check behind `cfg(debug_assertions)` (the
/// form it had before it was a release guard), the retry removed, the
/// answer moved above the check, and an arm removed.
#[test]
fn full_expanse_scanner_turns_red_on_the_real_source() {
    const ARM: &str = "EdgeTag::Structural(EdgeType::FullExpanse) => {";
    const GUARD: &str = "if !full_expanse_fills(edge, level) {";
    const RETRY: &str =
        "                return Err(Retry);\n            }\n            Ok(Some((suffix";
    let one = |faults: Vec<String>, clause: &str| {
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(faults[0].contains(clause), "{faults:?}");
    };
    assert!(full_expanse_faults(&SYNC_NAV).is_empty());
    // The check removed (commented out), and tested on something else.
    let src = edited_after(
        &SYNC_NAV,
        ARM,
        GUARD,
        "// if !full_expanse_fills(edge, level) {\n            if false {",
    );
    one(
        full_expanse_faults(&src),
        "arm 0: expected the level check once, found 0",
    );
    // The check compiled out of release builds.
    let src = edited_after(
        &SYNC_NAV,
        ARM,
        GUARD,
        "#[cfg(debug_assertions)]\n            if !full_expanse_fills(edge, level) {",
    );
    one(
        full_expanse_faults(&src),
        "arm 0: the level check is behind a cfg attribute",
    );
    // The retry removed: the arm falls through to its answer.
    let src = edited_after(
        &SYNC_NAV,
        ARM,
        RETRY,
        "            }\n            Ok(Some((suffix",
    );
    one(
        full_expanse_faults(&src),
        "arm 0: no `return Err(Retry);` within four lines",
    );
    // The answer above the check.
    let src = edited_after(
        &SYNC_NAV,
        ARM,
        GUARD,
        "if suffix == u64::MAX {\n                return Ok(Some((suffix, 0)));\n            }\n            if !full_expanse_fills(edge, level) {",
    );
    one(
        full_expanse_faults(&src),
        "arm 0: expected one answer from the suffix, found 2",
    );
    // The second arm is read too: its retry removed.
    let second = SYNC_NAV.find(ARM).expect("the first arm") + ARM.len();
    let tail = edited_after(
        &SYNC_NAV[second..],
        ARM,
        RETRY,
        "            }\n            Ok(Some((suffix",
    );
    let src = format!("{}{tail}", &SYNC_NAV[..second]);
    one(
        full_expanse_faults(&src),
        "arm 1: no `return Err(Retry);` within four lines",
    );
    // An arm removed.
    let src = edited_after(
        &SYNC_NAV,
        ARM,
        "EdgeType::FullExpanse)",
        "EdgeType::FullExpansf)",
    );
    one(full_expanse_faults(&src), "expected two FullExpanse arms");
    // The same text in a comment is not a check.
    let src = edited_after(
        &SYNC_NAV,
        ARM,
        GUARD,
        "// if !full_expanse_fills(edge, level) { return Err(Retry); }\n            if !full_expanse_fills(edge, level) {",
    );
    assert!(full_expanse_faults(&src).is_empty());
}

/// A Windows checkout: the same sources with CRLF line endings, as
/// `include_str!` reads them there. Normalized, they scan as an LF checkout
/// does and accept the multi-line edits the negative controls make; raw,
/// an edit whose needle spans a line end finds nothing, and a scanner that
/// looks for a bare `}` line finds no end to an arm.
#[test]
fn crlf_checkouts_scan_as_lf_checkouts() {
    const ARM: &str = "EdgeTag::Structural(EdgeType::FullExpanse) => {";
    const RETRY: &str =
        "                return Err(Retry);\n            }\n            Ok(Some((suffix";
    let crlf_nav = SYNC_NAV.replace('\n', "\r\n");
    let crlf_sync = SYNC.replace('\n', "\r\n");
    assert!(crlf_nav.contains("\r\n") && crlf_sync.contains("\r\n"));
    let nav = normalized(&crlf_nav);
    let sync = normalized(&crlf_sync);
    assert_eq!(
        nav, *SYNC_NAV,
        "normalizing a CRLF copy gives the LF text back"
    );
    assert!(full_expanse_faults(&nav).is_empty());
    assert!(branch_b_faults(&nav).is_empty());
    assert!(optimistic_read_faults(&sync).is_empty());
    let none: Vec<(String, String)> = Vec::new();
    assert!(set_form_faults(&sync, &nav, &none).is_empty());
    assert!(map_form_faults(&sync, &nav, &none).is_empty());
    let edited = edited_after(
        &nav,
        ARM,
        RETRY,
        "            }\n            Ok(Some((suffix",
    );
    let faults = full_expanse_faults(&edited);
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("arm 0: no `return Err(Retry);`"),
        "{faults:?}"
    );
    // The raw CRLF text is what the normalization is for: a needle with a
    // line end in it is not found there.
    let raw = std::panic::catch_unwind(|| {
        edited_after(
            &crlf_nav,
            ARM,
            RETRY,
            "            }\n            Ok(Some((suffix",
        )
    });
    assert!(
        raw.is_err(),
        "a multi-line needle must not match CRLF text unnormalized"
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
