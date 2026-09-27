//! Structural checks on writer stores to published memory that the Miri
//! census cannot catch on a schedule (#1190, #1086).
//!
//! The census catches a plain store to published memory only when a reader's
//! load overlaps it with no happens-before edge between them, and the version
//! words order most loads. Two fixes have no failing census record for that
//! reason, so this file checks them by reading the source, with `//` comments
//! stripped:
//!
//! - the covered walks (`insert_with_path_occ`, `remove_occ`,
//!   `map_insert_with_path_occ`, `map_remove_occ`) reach the edge they own
//!   only through `Edge::load_at` / `Edge::store_at` on the raw `edge_ptr`,
//!   never by dereferencing it (no `&mut Edge` over a published slot);
//! - the 32-bit bitmap branch's subarray helpers (`sub_edges_insert`,
//!   `sub_edges_remove`) write a published subarray only through `word::`
//!   stores: a plain write appears only in the `else` arm of `if SHARED`.
//!
//! Each clause was shown to fail on its own when the code it guards was
//! broken (PR body of the change that added this file).
#![cfg(not(miri))]

const MUTATE: &str = include_str!("../src/mutate.rs");
const MUTATE_MAP: &str = include_str!("../src/mutate_map.rs");
const TRIE32: &str = include_str!("../src/trie32.rs");

/// The code of a line, without its `//` comment.
fn code(line: &str) -> &str {
    line.split("//").next().unwrap_or("")
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

/// Lines of a covered walk that dereference `edge_ptr`.
fn edge_ptr_derefs(body: &[&str]) -> Vec<String> {
    body.iter()
        .map(|l| code(l))
        .filter(|c| {
            c.match_indices("edge_ptr").any(|(i, _)| {
                let before = c[..i].trim_end();
                let after = &c[i + "edge_ptr".len()..];
                let word_end = !after.starts_with(|ch: char| ch.is_alphanumeric() || ch == '_');
                word_end && before.ends_with('*')
            })
        })
        .map(|c| c.trim().to_string())
        .collect()
}

fn edge_ptr_stores(body: &[&str]) -> usize {
    body.iter()
        .filter(|l| code(l).contains("Edge::store_at::<OCC>(edge_ptr"))
        .count()
}

/// Plain writes that could reach a subarray.
const PLAIN_WRITES: &[&str] = &[
    "core::ptr::copy(",
    ".write(",
    "*p.add(",
    "put_edge::<false>",
];

/// Lines of `body` holding a plain write outside the `else` arm of an
/// `if SHARED {` block. The arms are found by indentation: an `if SHARED {`
/// line and the `} else {` and `}` lines at the same indent.
fn plain_writes_outside_else(body: &[&str]) -> Vec<String> {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut in_else = vec![false; body.len()];
    for (i, l) in body.iter().enumerate() {
        if code(l).trim() != "if SHARED {" {
            continue;
        }
        let ind = indent(l);
        let Some(e) = (i + 1..body.len())
            .find(|&j| indent(body[j]) == ind && body[j].trim().starts_with('}'))
        else {
            continue;
        };
        if body[e].trim() != "} else {" {
            continue;
        }
        if let Some(end) =
            (e + 1..body.len()).find(|&j| indent(body[j]) == ind && body[j].trim() == "}")
        {
            in_else[e + 1..end].iter_mut().for_each(|x| *x = true);
        }
    }
    body.iter()
        .enumerate()
        .filter(|(i, l)| !in_else[*i] && PLAIN_WRITES.iter().any(|w| code(l).contains(w)))
        .map(|(_, l)| l.trim().to_string())
        .collect()
}

#[test]
fn covered_walks_store_edges_through_the_raw_pointer() {
    let mut bad = Vec::new();
    for (src, name) in [
        (MUTATE, "insert_with_path_occ"),
        (MUTATE, "remove_occ"),
        (MUTATE_MAP, "map_insert_with_path_occ"),
        (MUTATE_MAP, "map_remove_occ"),
    ] {
        let body = fn_body(src, name);
        assert!(
            edge_ptr_stores(&body) > 0,
            "{name}: no `Edge::store_at::<OCC>(edge_ptr, ..)` found -- the scanner is not reading \
             the walk it guards"
        );
        bad.extend(
            edge_ptr_derefs(&body)
                .into_iter()
                .map(|l| format!("{name}: {l}")),
        );
    }
    assert!(
        bad.is_empty(),
        "a covered walk dereferences the published edge it owns (use Edge::load_at / \
         Edge::store_at on edge_ptr):\n{}",
        bad.join("\n")
    );
}

#[test]
fn sync32_subarray_helpers_store_shared_edges_as_words() {
    let mut bad = Vec::new();
    for name in ["sub_edges_insert", "sub_edges_remove"] {
        let body = fn_body(TRIE32, name);
        bad.extend(
            plain_writes_outside_else(&body)
                .into_iter()
                .map(|l| format!("{name}: {l}")),
        );
        if !body.iter().any(|l| code(l).contains("word::store_edge(")) {
            bad.push(format!(
                "{name}: no `word::store_edge` found -- the shared path shifts no edge as a word, or the scanner is not reading the helper it guards"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "a shared subarray is written without word stores (a plain write belongs only in the \
         `else` arm of `if SHARED`):\n{}",
        bad.join("\n")
    );
}

/// The scanners on synthetic input: each clause turns red on its own.
#[test]
fn scanners_catch_each_fault() {
    let good = [
        "    let mut cur = unsafe { Edge::load_at::<OCC>(edge_ptr) };",
        "    unsafe { Edge::store_at::<OCC>(edge_ptr, *edge) };",
        "    path.record_ancestor(edge_ptr, level);",
        "    // let edge = &mut *edge_ptr;",
    ];
    assert!(edge_ptr_derefs(&good).is_empty());
    assert_eq!(edge_ptr_stores(&good), 1);
    for bad in [
        "    let edge = &mut *edge_ptr;",
        "    *edge_ptr = x;",
        "    let e = * edge_ptr;",
    ] {
        assert_eq!(edge_ptr_derefs(&[bad]).len(), 1, "{bad}");
    }
    // A different pointer whose name ends in `edge_ptr` is not this one.
    assert!(edge_ptr_derefs(&["    *child_edge_ptr_x = y;"]).is_empty());

    let helper = [
        "fn h<const SHARED: bool>() {",
        "    if SHARED {",
        "        word::store_edge(p.add(1), e);",
        "    } else {",
        "        core::ptr::copy(p, p.add(1), n);",
        "    }",
        "    word::put_edge::<SHARED>(p.add(0), e);",
        "}",
    ];
    assert!(plain_writes_outside_else(&helper).is_empty());
    let mut in_shared_arm = helper;
    in_shared_arm[2] = "        *p.add(1) = e;";
    assert_eq!(plain_writes_outside_else(&in_shared_arm).len(), 1);
    let mut after_if = helper;
    after_if[6] = "    p.add(0).write(e);";
    assert_eq!(plain_writes_outside_else(&after_if).len(), 1);
}
