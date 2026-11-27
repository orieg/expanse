//! Every pattern of `hot_layout.ld` names a function the source defines.
//!
//! The linker script selects functions by the input section the compiler
//! gives them, `.text.<symbol>`. A pattern that matches nothing places
//! nothing and the link still succeeds, so a renamed or moved function would
//! silently leave the layout. This test decodes each pattern into a module
//! and a function name and requires the definition to exist, generic exactly
//! when the pattern says so.

use std::fs;
use std::path::Path;

const CRATE: &str = "12expanse_trie";

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

/// The input-section patterns the script lists, without the `.text.` prefix.
fn patterns(code: &str) -> Vec<String> {
    let needle = "*(.text.";
    code.match_indices(needle)
        .map(|(at, _)| {
            let rest = &code[at + needle.len()..];
            rest.split(')')
                .next()
                .expect("closing parenthesis")
                .to_owned()
        })
        .collect()
}

/// Splits one length-prefixed identifier off the front of a mangled path.
fn ident(path: &str) -> (&str, &str) {
    let digits = path.chars().take_while(char::is_ascii_digit).count();
    assert!(digits > 0, "no length prefix in {path:?}");
    let len: usize = path[..digits].parse().expect("length prefix");
    let rest = &path[digits..];
    assert!(rest.len() >= len, "length prefix overruns {path:?}");
    rest.split_at(len)
}

/// Whether `source` defines `fn name` outside comments, and whether that
/// definition takes generic parameters.
fn definition(source: &str, name: &str) -> Option<bool> {
    let plain = format!("fn {name}(");
    let generic = format!("fn {name}<");
    let mut found = None;
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or("");
        if code.contains(&generic) {
            found = Some(true);
        } else if code.contains(&plain) && found.is_none() {
            found = Some(false);
        }
    }
    found
}

#[test]
fn every_pattern_names_a_defined_function() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let listed = patterns(&script_code(&root));
    assert_eq!(listed.len(), 15, "hot_layout.ld lists {listed:?}");
    for (i, pattern) in listed.iter().enumerate() {
        assert!(
            !listed[..i].contains(pattern),
            "hot_layout.ld lists {pattern} more than once"
        );
        if let Some(mangled) = pattern.strip_prefix("_R*") {
            let path = mangled
                .strip_prefix(CRATE)
                .unwrap_or_else(|| panic!("{pattern} does not name the {CRATE} crate"));
            let (module, rest) = ident(path);
            let (name, tail) = ident(rest);
            let wants_generic = match tail {
                "" => false,
                "K*" => true,
                other => panic!("{pattern} ends in {other:?}, neither nothing nor K*"),
            };
            let file = root.join(format!("crates/expanse/src/{module}.rs"));
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|_| panic!("{pattern} names module {module}, which has no file"));
            let is_generic = definition(&source, name)
                .unwrap_or_else(|| panic!("{pattern}: {module}.rs defines no fn {name}"));
            assert_eq!(
                is_generic, wants_generic,
                "{pattern}: fn {name} is generic = {is_generic}, the pattern says {wants_generic}"
            );
        } else {
            let source = fs::read_to_string(root.join("crates/expanse-capi/src/compat.rs"))
                .expect("compat.rs is readable");
            let exported = format!("pub unsafe extern \"C\" fn {pattern}(");
            assert!(
                source
                    .lines()
                    .any(|l| l.split("//").next().unwrap_or("").contains(&exported)),
                "{pattern}: compat.rs exports no such function"
            );
        }
    }
}
