//! `hot_layout.ld` and the `link_section` attributes it places name the same
//! set of functions.
//!
//! The linker script places a function only if an attribute puts it in a
//! `.text.expanse_hot.<group>.<name>` input section, and an attribute whose
//! name the script does not list falls into its group's trailing wildcard at
//! an address nothing chose. Either drift is silent at link time, so this
//! test compares the two lists.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const PREFIX: &str = ".text.expanse_hot.";

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("source directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Section names carried by `link_section` attributes, with their site count.
/// A line whose code part (before any `//`) holds no attribute is not counted,
/// so a commented-out attribute is seen as absent.
fn attribute_names(root: &Path) -> BTreeMap<String, usize> {
    let mut files = Vec::new();
    rust_sources(&root.join("crates/expanse/src"), &mut files);
    rust_sources(&root.join("crates/expanse-capi/src"), &mut files);
    let needle = format!("link_section = \"{PREFIX}");
    let mut names = BTreeMap::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("source file is readable");
        for line in text.lines() {
            let code = line.split("//").next().unwrap_or("");
            if let Some(at) = code.find(&needle) {
                let rest = &code[at + needle.len()..];
                let name = rest.split('"').next().expect("closing quote");
                *names.entry(name.to_owned()).or_insert(0) += 1;
            }
        }
    }
    names
}

/// The script with its `/* ... */` comments removed.
fn script_code(root: &Path) -> String {
    let text = fs::read_to_string(root.join("crates/expanse-capi/hot_layout.ld"))
        .expect("hot_layout.ld is readable");
    let mut code = String::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find("/*") {
        code.push_str(&rest[..open]);
        let close = rest[open..].find("*/").expect("comment is closed");
        rest = &rest[open + close + 2..];
    }
    code.push_str(rest);
    code
}

/// Names the script lists, in order, wildcards included.
fn script_names(code: &str) -> Vec<String> {
    let needle = format!("*({PREFIX}");
    code.match_indices(&needle)
        .map(|(at, _)| {
            let rest = &code[at + needle.len()..];
            rest.split(')')
                .next()
                .expect("closing parenthesis")
                .to_owned()
        })
        .collect()
}

#[test]
fn script_and_attributes_name_the_same_functions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let attrs = attribute_names(&root);
    assert!(!attrs.is_empty(), "no link_section attribute was found");
    for (name, sites) in &attrs {
        assert_eq!(*sites, 1, "{name} is carried by {sites} attributes");
    }

    let listed = script_names(&script_code(&root));
    let explicit: Vec<&String> = listed.iter().filter(|n| !n.ends_with(".*")).collect();
    for name in &explicit {
        assert!(
            attrs.contains_key(*name),
            "hot_layout.ld places {name}, which no attribute carries"
        );
        assert_eq!(
            explicit.iter().filter(|n| n == &name).count(),
            1,
            "hot_layout.ld lists {name} more than once"
        );
    }
    for name in attrs.keys() {
        assert!(
            explicit.iter().any(|n| *n == name),
            "{name} carries an attribute and hot_layout.ld does not place it"
        );
        let group = name.split('.').next().expect("group");
        assert!(
            listed.iter().any(|n| n == &format!("{group}.*")),
            "hot_layout.ld has no trailing wildcard for group {group}"
        );
    }
}
