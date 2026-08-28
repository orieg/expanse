//! Automated sync test: validates that `docs/architecture_visualizer.html`
//! and `docs/visualizer_data.json` are bit-exact synchronized with the
//! compiled Rust codebase (`types.rs`, `set.rs`, `leaf.rs`, `instructions.rs`).
//!
//! If any geometry constant, promotion ceiling, or benchmark definition changes
//! in the Rust source code, this test will fail until the visualizer is updated.

#![cfg(not(miri))]

use expanse_trie::set::ROOT_LEAF_CAP;
use expanse_trie::types::{
    BITMAP_TO_UNCOMPRESSED_THRESHOLD, BRANCH_FANOUT, BRANCH_L3_CAP, BRANCH_L7_CAP, CACHE_LINE,
    MAX_LEVEL, RAW_ALIGN,
};
use std::fs;
use std::path::Path;

#[test]
fn test_visualizer_constants_sync() {
    // Locate the visualizer HTML and JSON files relative to CARGO_MANIFEST_DIR
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let repo_root = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("failed to locate workspace root");

    let html_path = repo_root.join("docs").join("architecture_visualizer.html");
    let json_path = repo_root.join("docs").join("visualizer_data.json");

    assert!(
        html_path.exists(),
        "docs/architecture_visualizer.html must exist at {:?}",
        html_path
    );
    assert!(
        json_path.exists(),
        "docs/visualizer_data.json must exist at {:?}",
        json_path
    );

    let html_content =
        fs::read_to_string(&html_path).expect("failed to read docs/architecture_visualizer.html");
    let json_content =
        fs::read_to_string(&json_path).expect("failed to read docs/visualizer_data.json");

    // 1. Verify Rust constants match expected values
    assert_eq!(ROOT_LEAF_CAP, 31, "ROOT_LEAF_CAP should be 31");
    assert_eq!(BRANCH_L3_CAP, 3, "BRANCH_L3_CAP should be 3");
    assert_eq!(BRANCH_L7_CAP, 7, "BRANCH_L7_CAP should be 7");
    assert_eq!(
        BITMAP_TO_UNCOMPRESSED_THRESHOLD, 192,
        "BITMAP_TO_UNCOMPRESSED_THRESHOLD should be 192"
    );
    assert_eq!(MAX_LEVEL, 8, "MAX_LEVEL should be 8");
    assert_eq!(BRANCH_FANOUT, 256, "BRANCH_FANOUT should be 256");
    assert_eq!(CACHE_LINE, 64, "CACHE_LINE should be 64");
    assert_eq!(RAW_ALIGN, 16, "RAW_ALIGN should be 16");

    // 2. Verify HTML embeds all critical constants
    assert!(
        html_content.contains("ROOT_LEAF_CAP: 31")
            || html_content.contains("ROOT_LEAF_CAP = 31")
            || html_content.contains("pop <= 31")
            || html_content.contains("Pop ≤ 31"),
        "HTML visualizer must reference ROOT_LEAF_CAP (31)"
    );
    assert!(
        html_content.contains("BRANCH_L3_CAP")
            || html_content.contains("Branch L3/L7")
            || html_content.contains("Fanout 1..7"),
        "HTML visualizer must reference BRANCH_L3_CAP / BRANCH_L7_CAP"
    );
    assert!(
        html_content.contains("192") || html_content.contains("Branch U"),
        "HTML visualizer must reference uncompressed threshold (192)"
    );
    assert!(
        html_content.contains("BENCHMARK_DATA"),
        "HTML visualizer must contain BENCHMARK_DATA array"
    );
    assert!(
        html_content.contains("MEMORY_DATA"),
        "HTML visualizer must contain MEMORY_DATA array"
    );

    // 3. Verify JSON data file contains matching ladder definitions
    assert!(
        json_content.contains(r#""ROOT_LEAF_CAP": 31"#),
        "JSON must contain ROOT_LEAF_CAP: 31"
    );
    assert!(
        json_content.contains(r#""BRANCH_L3_CAP": 3"#),
        "JSON must contain BRANCH_L3_CAP: 3"
    );
    assert!(
        json_content.contains(r#""BRANCH_L7_CAP": 7"#),
        "JSON must contain BRANCH_L7_CAP: 7"
    );
    assert!(
        json_content.contains(r#""BRANCHB_UP": 192"#),
        "JSON must contain BRANCHB_UP: 192"
    );
    assert!(
        json_content.contains(r#""MAX_LEVEL": 8"#),
        "JSON must contain MAX_LEVEL: 8"
    );

    // 4. Verify 32-bit embedded constants
    assert_eq!(expanse_trie::types32::MAX_LEVEL_32, 4);
    assert_eq!(expanse_trie::types32::CACHE_LINE_32, 32);
    assert_eq!(core::mem::size_of::<expanse_trie::types32::Edge32>(), 8);
    assert!(
        json_content.contains("embedded_32bit_benchmarks"),
        "JSON must contain embedded_32bit_benchmarks section"
    );
}

#[test]
fn test_benchmark_coverage_sync() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let repo_root = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("failed to locate workspace root");

    let benches_path = repo_root
        .join("crates")
        .join("expanse")
        .join("benches")
        .join("instructions.rs");
    let benches_content =
        fs::read_to_string(&benches_path).expect("failed to read benches/instructions.rs");

    let json_path = repo_root.join("docs").join("visualizer_data.json");
    let json_content =
        fs::read_to_string(&json_path).expect("failed to read docs/visualizer_data.json");

    let html_path = repo_root.join("docs").join("architecture_visualizer.html");
    let html_content =
        fs::read_to_string(&html_path).expect("failed to read docs/architecture_visualizer.html");

    // All library benchmark functions in instructions.rs must be accounted for in visualizer data
    let expected_benches = [
        "map_insert",
        "set_insert",
        "map_ins_slot",
        "map_get",
        "set_contains",
        "map_churn",
        "map_remove",
        "map_iterate",
        "map_nav",
    ];

    for bench in expected_benches {
        assert!(
            benches_content.contains(&format!("fn {bench}")),
            "benches/instructions.rs must contain function {bench}"
        );
        assert!(
            json_content.contains(bench),
            "docs/visualizer_data.json must include benchmark entries for {bench}"
        );
        assert!(
            html_content.contains(bench),
            "docs/architecture_visualizer.html must include benchmark entries for {bench}"
        );
    }

    // Verify Drop-In C-Compat ABI Benchmark coverage
    let expected_c_benches = [
        "judyl_get",
        "judy1_set",
        "judyl_insert",
        "judy1_test",
        "judyl_churn",
    ];
    for c_bench in expected_c_benches {
        assert!(
            json_content.contains(c_bench),
            "docs/visualizer_data.json must include C ABI benchmark entries for {c_bench}"
        );
        assert!(
            html_content.contains(c_bench),
            "docs/architecture_visualizer.html must include C ABI benchmark entries for {c_bench}"
        );
    }
    assert!(
        html_content.contains("STOCK_VS_EXPANSE_DATA"),
        "docs/architecture_visualizer.html must define STOCK_VS_EXPANSE_DATA"
    );
    assert!(
        html_content.contains("YCSB_BENCHMARKS_DATA"),
        "docs/architecture_visualizer.html must define YCSB_BENCHMARKS_DATA"
    );
    assert!(
        html_content.contains("LARGE_VALUE_BENCHMARKS_DATA"),
        "docs/architecture_visualizer.html must define LARGE_VALUE_BENCHMARKS_DATA"
    );
    assert!(
        json_content.contains("ycsb_benchmarks"),
        "docs/visualizer_data.json must contain ycsb_benchmarks"
    );
    assert!(
        json_content.contains("large_value_benchmarks"),
        "docs/visualizer_data.json must contain large_value_benchmarks"
    );
    assert!(
        json_content.contains("modern_architecture"),
        "docs/visualizer_data.json must contain modern_architecture"
    );
    assert!(
        html_content.contains("ValueSlot")
            && html_content.contains("BlobArena")
            && html_content.contains("Edge32")
            && html_content.contains("SyncExpanseMap"),
        "docs/architecture_visualizer.html must contain modern architecture sections"
    );
}

#[test]
fn test_visualizer_javascript_syntax() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let repo_root = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("failed to locate workspace root");

    let html_path = repo_root.join("docs").join("architecture_visualizer.html");
    let html_content =
        fs::read_to_string(&html_path).expect("failed to read docs/architecture_visualizer.html");

    // Extract all <script>...</script> blocks
    let mut scripts = Vec::new();
    let mut cursor = 0;
    while let Some(start_tag) = html_content[cursor..].find("<script") {
        let script_start = cursor + start_tag;
        let tag_end = match html_content[script_start..].find('>') {
            Some(pos) => script_start + pos + 1,
            None => break,
        };
        let script_end = match html_content[tag_end..].find("</script>") {
            Some(pos) => tag_end + pos,
            None => break,
        };
        let script_body = &html_content[tag_end..script_end];
        scripts.push(script_body);
        cursor = script_end + 9;
    }

    assert!(
        !scripts.is_empty(),
        "docs/architecture_visualizer.html must contain at least one <script> tag"
    );

    // 1. Rust-level parser checking comment and brace balances
    for (i, script) in scripts.iter().enumerate() {
        validate_js_structure(script, i);
    }

    // 2. If `node` is installed on PATH, execute `node -c` to verify with V8
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        use std::io::Write;
        for (i, script) in scripts.iter().enumerate() {
            let mut child = std::process::Command::new("node")
                .arg("-c")
                .arg("-")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("spawn node");
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(script.as_bytes()).expect("write script");
            }
            let output = child.wait_with_output().expect("node exit");
            assert!(
                output.status.success(),
                "node -c failed on <script> #{i}:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

/// Structural validator for JavaScript comments and delimiters
fn validate_js_structure(js: &str, script_idx: usize) {
    let chars: Vec<char> = js.chars().collect();
    let mut i = 0;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_template_string = false;
    let mut brace_stack = Vec::new();

    while i < chars.len() {
        let ch = chars[i];
        let next_ch = if i + 1 < chars.len() {
            Some(chars[i + 1])
        } else {
            None
        };

        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            if ch == '*' && next_ch == Some('/') {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if in_single_quote {
            if ch == '\\' {
                i += 2;
                continue;
            }
            if ch == '\'' {
                in_single_quote = false;
            }
            i += 1;
            continue;
        }

        if in_double_quote {
            if ch == '\\' {
                i += 2;
                continue;
            }
            if ch == '"' {
                in_double_quote = false;
            }
            i += 1;
            continue;
        }

        if in_template_string {
            if ch == '\\' {
                i += 2;
                continue;
            }
            if ch == '`' {
                in_template_string = false;
            }
            i += 1;
            continue;
        }

        // Check comment starts
        if ch == '/' && next_ch == Some('/') {
            in_line_comment = true;
            i += 2;
            continue;
        }

        if ch == '/' && next_ch == Some('*') {
            in_block_comment = true;
            i += 2;
            continue;
        }

        // Check string starts
        if ch == '\'' {
            in_single_quote = true;
            i += 1;
            continue;
        }
        if ch == '"' {
            in_double_quote = true;
            i += 1;
            continue;
        }
        if ch == '`' {
            in_template_string = true;
            i += 1;
            continue;
        }

        // Check delimiters
        match ch {
            '{' | '(' | '[' => brace_stack.push(ch),
            '}' => {
                let last = brace_stack.pop();
                assert_eq!(
                    last,
                    Some('{'),
                    "Mismatched closing '}}' in script #{script_idx}"
                );
            }
            ')' => {
                let last = brace_stack.pop();
                assert_eq!(
                    last,
                    Some('('),
                    "Mismatched closing ')' in script #{script_idx}"
                );
            }
            ']' => {
                let last = brace_stack.pop();
                assert_eq!(
                    last,
                    Some('['),
                    "Mismatched closing ']' in script #{script_idx}"
                );
            }
            _ => {}
        }

        i += 1;
    }

    assert!(
        !in_block_comment,
        "Unclosed multi-line comment '/*' in <script> #{script_idx}"
    );
    assert!(
        !in_single_quote,
        "Unclosed single-quote string in <script> #{script_idx}"
    );
    assert!(
        !in_double_quote,
        "Unclosed double-quote string in <script> #{script_idx}"
    );
    assert!(
        !in_template_string,
        "Unclosed template literal '`' in <script> #{script_idx}"
    );
    assert!(
        brace_stack.is_empty(),
        "Unclosed delimiters in <script> #{script_idx}: {:?}",
        brace_stack
    );
}

// =====================================================================
// Issue #384: provenance & derivation gates for the published artifact.
//
// The original sync test covered only the Callgrind benchmark routines,
// which left `memory_budget`, `stock_vs_expanse`, `concurrency_scaling`,
// `embedded_32bit_benchmarks` and every hand-written `notes` string
// ungated -- that is why retracted figures survived the #371-#375
// remediation inside a committed, published artifact. These tests close
// the gap: every derived column is recomputed, every measured memory cell
// is recomputed from the engine itself, benchmark rows are checked against
// the real harness arms, the HTML's embedded fallback copy must parse
// equal to the JSON, and stamped narrative claims are rejected outright
// (AGENTS.md section 8.2).
// =====================================================================

use expanse_trie::map::ExpanseMap;
use expanse_trie::node::Edge;
use expanse_trie::set::ExpanseSet;
use expanse_trie::types32::Edge32;
use expanse_trie::{ExpanseMap32, ExpanseSet32, Key32};
use serde_json::Value;

/// `crates/expanse/benches/instructions.rs::POP`.
const BENCH_POP: f64 = 50_000.0;

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("failed to locate workspace root")
        .to_path_buf()
}

fn load_json() -> Value {
    let path = repo_root().join("docs").join("visualizer_data.json");
    let text = fs::read_to_string(&path).expect("failed to read docs/visualizer_data.json");
    serde_json::from_str(&text).expect("docs/visualizer_data.json must be valid JSON")
}

fn load_html() -> String {
    fs::read_to_string(
        repo_root()
            .join("docs")
            .join("architecture_visualizer.html"),
    )
    .expect("failed to read docs/architecture_visualizer.html")
}

fn f(v: &Value, key: &str) -> f64 {
    v.get(key)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("missing or non-numeric field {key:?} in {v}"))
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing or non-string field {key:?} in {v}"))
}

fn round_to(x: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    (x * scale).round() / scale
}

/// The XorShift64 PRNG used by `crates/expanse/examples/bytes_per_key.rs`.
struct BudgetRng(u64);
impl BudgetRng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Mirrors the key generators in `crates/expanse/examples/bytes_per_key.rs`.
/// If that example's distributions change, this diverges and the published
/// memory rows fail -- which is the point.
fn budget_keys(dist: &str, n: usize) -> Vec<u64> {
    let mut rng = BudgetRng(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(n);
    match dist {
        "sequential" => out.extend(0..n as u64),
        "random" => out.extend((0..n).map(|_| rng.next())),
        "clustered" => {
            let mut base = 0;
            for i in 0..n as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        "clustered-wide" => {
            let mut base = 0;
            for i in 0..n as u64 {
                if i % 4096 == 0 {
                    base = rng.next() & !0xFFF;
                }
                out.push(base + (i % 4096));
            }
        }
        "sparse" => out.extend((0..n as u64).map(|i| i << 40)),
        other => panic!("unknown distribution {other}"),
    }
    out
}

fn bytes_per_key(dist: &str, pop: usize) -> (f64, f64) {
    let ks = budget_keys(dist, pop);
    let mut set = ExpanseSet::new();
    let mut map = ExpanseMap::new();
    for &k in &ks {
        set.insert(k);
        map.insert(k, !k);
    }
    (
        set.mem_used() as f64 / set.len().max(1) as f64,
        map.mem_used() as f64 / map.len().max(1) as f64,
    )
}

/// Extracts the balanced `[...]` / `{...}` literal that follows
/// `let <var> = ` in the visualizer HTML. The embedded fallback datasets are
/// emitted as verbatim JSON precisely so this test can parse and compare them.
fn extract_embedded_literal(html: &str, var: &str) -> Value {
    let needle = format!("\n  let {var} = ");
    let start = html
        .find(&needle)
        .unwrap_or_else(|| panic!("docs/architecture_visualizer.html must declare `let {var} =`"))
        + needle.len();
    let opener = html[start..].chars().next().expect("literal body");
    assert!(
        opener == '[' || opener == '{',
        "{var} literal must start with '[' or '{{', got {opener:?} -- the embedded \
         fallback datasets are generated from docs/visualizer_data.json and must \
         stay verbatim JSON"
    );
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in html[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    let text = &html[start..start + offset + ch.len_utf8()];
                    return serde_json::from_str(text).unwrap_or_else(|e| {
                        panic!("embedded {var} literal is not valid JSON: {e}")
                    });
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced {var} literal in docs/architecture_visualizer.html");
}

/// Walks every string leaf in a JSON value, yielding `(path, text)`.
fn string_leaves(v: &Value, path: &str, out: &mut Vec<(String, String)>) {
    match v {
        Value::String(t) => out.push((path.to_string(), t.clone())),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                string_leaves(item, &format!("{path}[{i}]"), out);
            }
        }
        Value::Object(map) => {
            for (k, item) in map {
                string_leaves(item, &format!("{path}.{k}"), out);
            }
        }
        _ => {}
    }
}

/// Every published section carries a provenance entry (AGENTS.md section 8.7).
#[test]
fn test_every_data_section_has_provenance() {
    let data = load_json();
    let prov = data
        .get("provenance")
        .and_then(Value::as_object)
        .expect("docs/visualizer_data.json must carry a top-level `provenance` map");

    for section in [
        "node_ladder",
        "benchmarks",
        "memory_budget",
        "stock_vs_expanse",
        "modern_architecture",
        "embedded_32bit_benchmarks",
    ] {
        assert!(
            data.get(section).is_some(),
            "docs/visualizer_data.json must contain section {section}"
        );
        assert!(
            prov.contains_key(section),
            "section {section} has no `provenance` entry -- every published number \
             needs a source (AGENTS.md section 8.7)"
        );
    }
    // Sub-sections whose numbers come from different runs must say so separately.
    for key in [
        "ycsb_benchmarks.workloads.throughput_mops",
        "ycsb_benchmarks.workloads.latency_and_memory",
        "ycsb_benchmarks.concurrency_scaling",
        "large_value_benchmarks.inlining_speedups",
        "large_value_benchmarks.predicate_scan_selectivity",
    ] {
        assert!(prov.contains_key(key), "missing provenance entry for {key}");
    }
}

/// No stamped narrative performance claims outside `provenance`
/// (AGENTS.md section 8.2). This is what let "45% faster than Stock Judy"
/// and "SIMD demotion saves -77% instrs" sit in a published artifact.
#[test]
fn test_no_stamped_narrative_claims() {
    let mut data = load_json();
    // `provenance` is the one place an honest retraction narrative belongs.
    data.as_object_mut()
        .expect("top level object")
        .remove("provenance");

    let banned = [
        "faster",
        "slower",
        "speedup",
        "outperform",
        "beats",
        "saves",
        "saving",
        "% reduction",
        "reduces",
        "advantage",
        "win rationale",
        "better than",
        "than stock",
        "vs stock",
    ];
    let mut leaves = Vec::new();
    string_leaves(&data, "", &mut leaves);
    for (path, text) in leaves {
        let lower = text.to_lowercase();
        for word in banned {
            assert!(
                !lower.contains(word),
                "narrative performance claim {word:?} in {path}: {text:?}\n\
                 AGENTS.md section 8.2 forbids stamped narrative constants in \
                 published artifacts. Put the derivation in `provenance`, or drop \
                 the string."
            );
        }
    }
}

/// Retracted figures must never reappear in the published artifact.
#[test]
fn test_retracted_figures_absent() {
    let mut data = load_json();
    data.as_object_mut()
        .expect("top level object")
        .remove("provenance");
    let text = serde_json::to_string(&data).expect("serialize");

    // The retracted 95/5 write-mixed `SyncExpanseMap` curve
    // (docs/BENCHMARKING.md section 3): it came from the ~100%-miss unbounded
    // `u64` word-key arms.
    for figure in ["19.63", "27.94", "35.62", "33.97", "28.84"] {
        assert!(
            !text.contains(figure),
            "retracted concurrency figure {figure} is published again -- the honest \
             measurement is negative scaling (0.12x-0.55x), docs/BENCHMARKING.md \
             section 3"
        );
    }
    // The retracted vs-libjudy wall-clock random-lookup reading (README.md):
    // measured under load; the quiet host measures ~11% SLOWER.
    for figure in ["26.8", "48.6"] {
        assert!(
            !text.contains(figure),
            "retracted vs-libjudy wall-clock figure {figure} ns is published again -- \
             see README.md and docs/BENCHMARKING.md"
        );
    }
    // The retracted YCSB Workload E `ExpanseMap` throughput (#375: an
    // asymmetric scan predicate passing ~100% vs ~50% for the baselines).
    assert!(
        !text.contains("15.26"),
        "retracted YCSB Workload E figure 15.26 Mops/s is published again (#375)"
    );
}

/// Every benchmark row must name an arm that exists in the harness.
/// `set_insert / small` was published although `set_insert` carries no
/// `#[bench::small]` arm.
#[test]
fn test_benchmark_rows_match_harness_arms() {
    let benches = fs::read_to_string(
        repo_root()
            .join("crates")
            .join("expanse")
            .join("benches")
            .join("instructions.rs"),
    )
    .expect("failed to read benches/instructions.rs");

    // Collect `#[bench::<dist>]` attributes and attach them to the `fn` they
    // decorate.
    let mut harness: std::collections::BTreeSet<String> = Default::default();
    let mut pending: Vec<String> = Vec::new();
    for line in benches.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("#[bench::") {
            pending.push(
                rest.chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect(),
            );
        } else if let Some(rest) = line.strip_prefix("fn ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            for dist in pending.drain(..) {
                harness.insert(format!("{name}/{dist}"));
            }
        }
    }
    assert!(
        harness.contains("map_insert/random"),
        "harness arm parse failed -- expected map_insert/random in {harness:?}"
    );

    let data = load_json();
    for row in data["benchmarks"].as_array().expect("benchmarks array") {
        let id = s(row, "id");
        assert!(
            harness.contains(id),
            "docs/visualizer_data.json publishes benchmark arm {id:?}, which does not \
             exist in crates/expanse/benches/instructions.rs. Known arms: {harness:?}"
        );
        assert_eq!(
            s(row, "name"),
            id.replace('/', " / "),
            "benchmark `name` must be the `id` with spaces around the slash"
        );
    }
}

/// The benchmark table's derived columns must actually be derived.
#[test]
fn test_benchmark_derived_columns() {
    let data = load_json();
    for row in data["benchmarks"].as_array().expect("benchmarks array") {
        let id = s(row, "id");
        let v1 = f(row, "instructions_v1");
        let v3 = f(row, "instructions_v3");

        let expected_per_op = round_to(v1 / BENCH_POP, 2);
        assert!(
            (f(row, "instr_per_op_v1") - expected_per_op).abs() < 1e-9,
            "{id}: instr_per_op_v1 must be instructions_v1 / {BENCH_POP} = \
             {expected_per_op}, got {}",
            f(row, "instr_per_op_v1")
        );

        assert_eq!(
            s(row, "v3_delta"),
            format!("{:.2}%", (v3 - v1) / v1 * 100.0),
            "{id}: v3_delta must be derived from the two instruction counts"
        );
    }
}

/// The vs-libjudy table's ratio columns must follow from the raw counts, and
/// the columns withdrawn for lack of provenance must stay withdrawn.
#[test]
fn test_stock_vs_expanse_derived_columns() {
    let data = load_json();
    let rows = data["stock_vs_expanse"].as_array().expect("stock array");
    assert!(!rows.is_empty());
    for row in rows {
        let op = s(row, "op");
        let v1 = f(row, "expanse_so_v1");
        let v3 = f(row, "expanse_so_v3");
        let stock = f(row, "stock_libjudy");
        let ratio = v3 / stock;

        assert_eq!(
            s(row, "ratio_so"),
            format!("{ratio:.2}x"),
            "{op}: ratio_so must be expanse_so_v3 / stock_libjudy"
        );
        assert_eq!(
            s(row, "hw_delta"),
            format!("{:.2}%", (v3 - v1) / v1 * 100.0),
            "{op}: hw_delta must be the v1 -> v3 instruction delta"
        );
        assert_eq!(
            row["is_win"].as_bool().expect("is_win"),
            ratio < 1.0,
            "{op}: is_win must follow from ratio_so, not be asserted by hand"
        );
        // Columns with no committed count and no stated model stay removed.
        for gone in ["ratio_rlib", "est_cycles_so", "notes"] {
            assert!(
                row.get(gone).is_none(),
                "{op}: `{gone}` was removed for lack of provenance (#384); re-add it \
                 only together with the artifact it derives from"
            );
        }
    }
}

/// Every memory cell is recomputed from the engine. This is the gate that
/// would have caught the "Random Uniform 1M = 16.31 B/key" row -- 16.31 is
/// the *sparse* `i << 40` figure, not the random-uniform one.
#[test]
fn test_memory_budget_matches_engine() {
    let data = load_json();
    let rows = data["memory_budget"]
        .as_array()
        .expect("memory_budget array");

    // The label a reader sees must map to the generator that produced the row.
    let dists = [
        ("Sequential (0..N)", "sequential"),
        ("Random Uniform (64-bit)", "random"),
        ("Clustered (256-key runs)", "clustered"),
        ("Clustered-wide (4096-key runs)", "clustered-wide"),
        ("Sparse (i << 40)", "sparse"),
    ];
    assert_eq!(
        rows.len(),
        dists.len(),
        "memory_budget must publish exactly the distributions \
         crates/expanse/examples/bytes_per_key.rs measures"
    );

    for (row, (label, dist)) in rows.iter().zip(dists) {
        assert_eq!(s(row, "dist"), label, "unexpected memory_budget row label");
        for (pop, set_key, map_key) in [
            (1_000usize, "set_1k", "map_1k"),
            (100_000, "set_100k", "map_100k"),
            (1_000_000, "set_1m", "map_1m"),
        ] {
            let (set_bpk, map_bpk) = bytes_per_key(dist, pop);
            assert!(
                (f(row, set_key) - round_to(set_bpk, 2)).abs() < 1e-9,
                "{label} {set_key}: published {} B/key but the engine measures \
                 {set_bpk:.2} B/key (mem_used()/len, deterministic). Re-run \
                 `cargo run --release -p expanse-trie --example bytes_per_key`.",
                f(row, set_key)
            );
            assert!(
                (f(row, map_key) - round_to(map_bpk, 2)).abs() < 1e-9,
                "{label} {map_key}: published {} B/key but the engine measures \
                 {map_bpk:.2} B/key",
                f(row, map_key)
            );
        }
        // The published ceilings are the committed regression budget.
        assert!(
            f(row, "set_1m") <= f(row, "set_ceiling_1m"),
            "{label}: set_1m exceeds its published ceiling"
        );
        assert!(
            f(row, "map_1m") <= f(row, "map_ceiling_1m"),
            "{label}: map_1m exceeds its published ceiling"
        );
    }
}

/// The 32-bit density rows are recomputed too. The published figures
/// (0.51 / 0.63 / 8.03 / 8.00 B/key) matched none of the real ones -- the
/// CAN row was off by roughly 20x.
#[test]
fn test_embedded_32bit_matches_engine() {
    let data = load_json();
    let rows = data["embedded_32bit_benchmarks"]["workloads"]
        .as_array()
        .expect("embedded_32bit_benchmarks.workloads array");

    let mut measured: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();

    let n_sensor = 10_000usize;
    let mut sensor = ExpanseSet32::new();
    for i in 0..n_sensor {
        sensor.insert(1_700_000_000 + i as Key32);
    }
    measured.insert("Clustered Sensor Timestamps", (n_sensor, sensor.mem_used()));

    let n_can = 500usize;
    let mut can = ExpanseSet32::new();
    for i in 0..n_can as Key32 {
        can.insert((i * 100_007) & 0x1FFF_FFFF);
    }
    measured.insert("Sparse CAN-Bus 29-bit Identifiers", (n_can, can.mem_used()));

    let n_routes = 2_000usize;
    let mut routes = ExpanseMap32::new();
    for i in 0..n_routes as Key32 {
        let ip = (10 << 24) | ((i / 256) << 16) | ((i % 256) << 8);
        routes.insert(ip, i % 16);
    }
    measured.insert(
        "IPv4 Subnet /24 Routing Table",
        (n_routes, routes.mem_used()),
    );

    assert_eq!(
        rows.len(),
        measured.len(),
        "embedded_32bit_benchmarks must publish exactly the workloads \
         crates/expanse/examples/bytes_per_key_32.rs reports a B/key for"
    );

    for row in rows {
        let workload = s(row, "workload");
        let (n, mem) = *measured
            .get(workload)
            .unwrap_or_else(|| panic!("unknown 32-bit workload {workload:?}"));
        assert_eq!(
            row["keys"].as_u64().expect("keys") as usize,
            n,
            "{workload}: population mismatch"
        );
        assert_eq!(
            row["mem_used_bytes"].as_u64().expect("mem_used_bytes") as usize,
            mem,
            "{workload}: published mem_used() disagrees with the engine"
        );
        let expected = round_to(mem as f64 / n as f64, 4);
        assert!(
            (f(row, "bytes_per_key") - expected).abs() < 1e-9,
            "{workload}: published {} B/key, engine measures {expected} B/key",
            f(row, "bytes_per_key")
        );
        // A comparative baseline that resolves to no measurement stays out.
        for gone in ["btreeset_bpk", "btreemap_bpk", "memory_saving", "notes"] {
            assert!(
                row.get(gone).is_none(),
                "{workload}: `{gone}` was removed for lack of a measured baseline (#384)"
            );
        }
    }
}

/// YCSB derived columns, plus: the concurrency section must be labelled for
/// what it actually measures.
#[test]
fn test_ycsb_derived_and_concurrency_labelled() {
    let data = load_json();
    let ycsb = &data["ycsb_benchmarks"];

    for w in ycsb["workloads"].as_array().expect("ycsb workloads") {
        let name = s(w, "workload");
        for engine in ["expanse_blobmap", "btreemap", "skipmap", "expanse_map_u64"] {
            let e = &w[engine];
            let expected = round_to(f(e, "mem_mb") * 1_048_576.0 / 100_000.0, 1);
            assert!(
                (f(e, "bytes_per_key") - expected).abs() < 1e-9,
                "{name}/{engine}: bytes_per_key must be mem_mb * 1 MiB / 100,000 keys \
                 = {expected}, got {}",
                f(e, "bytes_per_key")
            );
        }
        let blob = f(&w["expanse_blobmap"], "throughput_mops");
        assert_eq!(
            s(w, "speedup_vs_btree"),
            format!("{:.2}x", blob / f(&w["btreemap"], "throughput_mops")),
            "{name}: speedup_vs_btree must be derived from the two throughputs"
        );
        assert_eq!(
            s(w, "speedup_vs_skip"),
            format!("{:.2}x", blob / f(&w["skipmap"], "throughput_mops")),
            "{name}: speedup_vs_skip must be derived from the two throughputs"
        );
    }

    let conc = &ycsb["concurrency_scaling"];
    assert_eq!(
        s(conc, "harness"),
        "crates/expanse/benches/concurrency.rs",
        "concurrency_scaling must name its harness"
    );
    let workloads = conc["workloads"].as_array().expect("concurrency workloads");
    let mixed = workloads
        .iter()
        .find(|w| s(w, "workload").contains("50%"))
        .expect("concurrency_scaling must publish the 50/50 write-mixed rows");
    // benches/concurrency.rs alternates read and write inside a single thread
    // loop, so the 50/50 read-op column is a mixed-workload rate. Publishing it
    // as "read scaling" is exactly the mislabel this section had to correct.
    let note = s(mixed, "metric_note").to_lowercase();
    assert!(
        note.contains("mixed") && note.contains("not read scaling"),
        "the 50/50 rows must be labelled a mixed-workload read-op rate, NOT read \
         scaling -- each bench thread alternates read and write in one loop, so \
         reads are pinned 1:1 to write handoffs. Got: {note:?}"
    );
    let rows = mixed["rows"].as_array().expect("rows");
    let first = f(&rows[0], "read_mops");
    let last = f(&rows[rows.len() - 1], "read_mops");
    assert!(
        last < first,
        "the measured 50/50 curve is NEGATIVE scaling (throughput falls as threads \
         are added). A rising curve here means the retracted word-key figures have \
         come back -- docs/BENCHMARKING.md section 3"
    );
    // docs/BENCHMARKING.md publishes the per-thread rows rounded to 0.1 Mops/s
    // and the scaling factor from the full-precision run, so the two agree only
    // to within the rounding envelope -- but they must still agree.
    for w in workloads {
        let name = s(w, "workload");
        let r = w["rows"].as_array().expect("rows");
        let lo = f(&r[0], "read_mops");
        let hi = f(&r[r.len() - 1], "read_mops");
        let published: f64 = s(w, "scale_at_16t")
            .trim_end_matches('x')
            .parse()
            .expect("scale_at_16t must parse");
        let derived = hi / lo;
        assert!(
            (published - derived).abs() <= (derived * 0.05).max(0.02),
            "{name}: scale_at_16t {published} does not follow from the published rows \
             ({hi} / {lo} = {derived:.3})"
        );
    }
}

/// `modern_architecture` must match the compiled layout, not a stale draft.
#[test]
fn test_modern_architecture_matches_source() {
    let data = load_json();
    let ma = &data["modern_architecture"];

    assert_eq!(
        ma["value_slot"]["word_size_bytes"].as_u64(),
        Some(core::mem::size_of::<u64>() as u64)
    );
    // Tags as declared by `SlotTag` in crates/expanse/src/slot.rs. `ArenaShort`
    // (0x10) / `ArenaLong` (0x11) were replaced by the single `ArenaMeta`
    // encoding and must not reappear in the published artifact.
    let tags: Vec<&str> = ma["value_slot"]["modes"]
        .as_array()
        .expect("value_slot.modes")
        .iter()
        .map(|m| s(m, "tag"))
        .collect();
    assert_eq!(
        tags,
        vec!["0x00..=0x07", "0x10", "0x12", "0xFE", "0xFF"],
        "value_slot tags must match SlotTag in crates/expanse/src/slot.rs"
    );
    let modes: Vec<&str> = ma["value_slot"]["modes"]
        .as_array()
        .expect("value_slot.modes")
        .iter()
        .map(|m| s(m, "mode"))
        .collect();
    assert!(
        modes.contains(&"ArenaMeta") && !modes.iter().any(|m| m.contains("Arena Short")),
        "the sole arena encoding is `ArenaMeta` (tag 0x10); got {modes:?}"
    );

    assert_eq!(
        ma["edge_32"]["edge_size_bytes"].as_u64(),
        Some(core::mem::size_of::<Edge32>() as u64),
        "edge_32.edge_size_bytes must equal size_of::<Edge32>()"
    );
    assert_eq!(
        s(&ma["edge_32"], "edge_size_vs_64bit"),
        format!(
            "{} B (Edge32) vs {} B (Edge on 64-bit targets)",
            core::mem::size_of::<Edge32>(),
            core::mem::size_of::<Edge>()
        ),
        "the 32-bit edge comparison must be derived from the two compiled sizes"
    );
}

/// Large-value rows: derived ratios, and no back-solved absolutes.
#[test]
fn test_large_value_rows_derived() {
    let data = load_json();
    let lv = &data["large_value_benchmarks"];

    for row in lv["inlining_speedups"]
        .as_array()
        .expect("inlining_speedups")
    {
        let size = s(row, "size");
        // A cell with no committed measurement is null, never interpolated.
        let get_ours = row["expanse_get_ns"].as_f64();
        let get_theirs = row["btree_get_ns"].as_f64();
        match (get_ours, get_theirs) {
            (Some(a), Some(b)) => assert_eq!(
                s(row, "get_speedup"),
                format!("{:.2}x", b / a),
                "{size}: get_speedup must be derived from the two times"
            ),
            (None, None) => assert!(
                row.get("source").is_some(),
                "{size}: a row without absolute times must cite where its ratios come from"
            ),
            _ => panic!("{size}: one of the two get times is missing -- publish both or neither"),
        }
        assert!(
            row.get("expanse_ins_ns").is_some(),
            "{size}: insert columns must be present (null when unmeasured)"
        );
    }

    for row in lv["predicate_scan_selectivity"]
        .as_array()
        .expect("predicate_scan_selectivity")
    {
        let sel = s(row, "selectivity");
        let ratio = f(row, "naive_deref_ms") / f(row, "columnar_scan_ms");
        let published: f64 = s(row, "speedup")
            .trim_end_matches('x')
            .parse()
            .expect("speedup must parse");
        assert!(
            (published - ratio).abs() <= 0.011,
            "{sel}: speedup {published} does not follow from {} / {}",
            f(row, "naive_deref_ms"),
            f(row, "columnar_scan_ms")
        );
    }

    // Withdrawn for lack of a tagged host and a committed artifact (#384).
    assert!(
        lv.get("arena_gc_compaction").is_none(),
        "arena GC compaction pause times need a tagged reference-host run before \
         they are published again (#384)"
    );
}

/// The HTML's embedded standalone-mode copy must equal the JSON. Two
/// hand-maintained copies of the same table is how the memory rows, the
/// concurrency curve and the benchmark field names drifted apart.
#[test]
fn test_html_embedded_datasets_match_json() {
    let data = load_json();
    let html = load_html();

    for (var, key) in [
        ("BENCHMARK_DATA", "benchmarks"),
        ("STOCK_VS_EXPANSE_DATA", "stock_vs_expanse"),
        ("MEMORY_DATA", "memory_budget"),
        ("YCSB_BENCHMARKS_DATA", "ycsb_benchmarks"),
        ("LARGE_VALUE_BENCHMARKS_DATA", "large_value_benchmarks"),
        (
            "EMBEDDED_32BIT_BENCHMARKS_DATA",
            "embedded_32bit_benchmarks",
        ),
    ] {
        let embedded = extract_embedded_literal(&html, var);
        assert_eq!(
            &embedded, &data[key],
            "docs/architecture_visualizer.html `{var}` (the file:/// fallback copy) \
             has drifted from docs/visualizer_data.json `{key}`. The JSON is the \
             source of truth; regenerate the embedded literal from it."
        );
    }
}

/// The live-data loader must be able to render everything it loads.
#[test]
fn test_live_loader_covers_every_dataset() {
    let html = load_html();
    for var in [
        "BENCHMARK_DATA",
        "STOCK_VS_EXPANSE_DATA",
        "MEMORY_DATA",
        "YCSB_BENCHMARKS_DATA",
        "LARGE_VALUE_BENCHMARKS_DATA",
        "EMBEDDED_32BIT_BENCHMARKS_DATA",
    ] {
        // `const` would make checkLiveDataSource()'s reassignment throw.
        assert!(
            html.contains(&format!("let {var} = ")),
            "{var} must be declared with `let`: checkLiveDataSource() reassigns it \
             when the page is served over http(s)"
        );
    }
    for key in [
        "liveData.benchmarks",
        "liveData.stock_vs_expanse",
        "liveData.memory_budget",
        "liveData.ycsb_benchmarks",
        "liveData.large_value_benchmarks",
        "liveData.embedded_32bit_benchmarks",
    ] {
        assert!(
            html.contains(key),
            "checkLiveDataSource() must refresh {key} from docs/visualizer_data.json"
        );
    }
}

/// Markdown counterpart to [`test_no_stamped_narrative_claims`].
///
/// The JSON scanner bans narrative words outright, which works because
/// `visualizer_data.json` is data -- prose has no business in it. Markdown
/// cannot use that rule: `docs/BENCHMARKING.md` legitimately says "faster"
/// on most pages, and the corrections record legitimately *names* a
/// retracted figure in order to retract it.
///
/// So this gate matches a retracted figure only when it appears in its
/// original claim context, and only when no retraction marker sits within
/// three lines. That combination is what distinguishes republishing a
/// withdrawn number from documenting that it was withdrawn.
///
/// Calibrated against the tree at the commit that added it: a bare-figure
/// scan flagged 8 lines, 7 of them false positives (`32.3 ns` is stock
/// libjudy's real measured figure; `-3.08%` is an unrelated delta). This
/// form flagged exactly one -- `docs/DATABASE.md` republishing the
/// retracted Workload E row -- which is fixed in the same commit.
#[test]
fn test_retracted_figures_absent_from_markdown() {
    // (retracted figure, lowercase context that makes it *that* claim)
    const CLAIMS: &[(&str, &[&str])] = &[
        ("15.26", &["workload e", "range scan", "scan"]),
        ("265.8", &["m ops", "mops", "thread", "scal"]),
        ("284.9", &["m ops", "mops", "set"]),
        ("12.0\u{d7}", &["thread", "scal"]),
        ("4.33", &["workload e", "scan", "expanse"]),
        ("146.7", &["b/entry", "skiplist", "tower"]),
        ("45% faster", &["judy"]),
        ("11.1\u{d7}", &["densit", "rocksdb", "b/entry"]),
    ];
    const MARKERS: &[&str] = &[
        "retract",
        "withdraw",
        "supersed",
        "correct",
        "previously",
        "no longer",
        "refuted",
        "stale",
        "anti-example",
        "earlier",
        "was measured with",
        "both were",
    ];

    let mut offenders = Vec::new();
    for path in markdown_docs() {
        let text = fs::read_to_string(&path).expect("read markdown doc");
        let lines: Vec<&str> = text.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let lower = line.to_lowercase();
            for (figure, contexts) in CLAIMS {
                if !line.contains(figure) || !contexts.iter().any(|c| lower.contains(c)) {
                    continue;
                }
                // A retraction marker anywhere in the +/-3 line window means
                // the figure is being documented as withdrawn, not published.
                let lo = idx.saturating_sub(3);
                let hi = (idx + 4).min(lines.len());
                let window = lines[lo..hi].join("\n").to_lowercase();
                if MARKERS.iter().any(|m| window.contains(m)) {
                    continue;
                }
                offenders.push(format!(
                    "{}:{} republishes retracted figure {figure:?}: {}",
                    path.display(),
                    idx + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "retracted figures republished in markdown without a retraction marker:\n{}\n\n\
         AGENTS.md section 8.7 forbids in-place backfilling. Either drop the figure or \
         state, within three lines, that it was withdrawn and what replaced it.",
        offenders.join("\n")
    );
}

/// Every tracked markdown doc: `README.md` plus `docs/**/*.md`.
fn markdown_docs() -> Vec<std::path::PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    let root = repo_root();
    let mut out = vec![root.join("README.md")];
    walk(&root.join("docs"), &mut out);
    out.sort();
    out
}

/// `docs/BENCHMARKING.md` publishes the same bytes/key table as
/// `docs/visualizer_data.json`, in prose. [`test_memory_budget_matches_engine`]
/// gates the JSON copy; this gates the markdown one against the same engine
/// call, so the two artifacts cannot disagree.
///
/// They did. The `random (set)` row read `12.34 / 13.83 / 7.66` while the
/// engine measured `13.50 / 14.78 / 7.92` — same claimed method ("deterministic
/// allocation accounting via `NodeAlloc`, machine-independent"), different
/// numbers, and no shared source to reconcile them. Every other row happened to
/// agree, which is exactly why nobody noticed (#384).
#[test]
fn test_benchmarking_md_density_table_matches_engine() {
    // (markdown row label, `bytes_per_key.rs` distribution name)
    const ROWS: &[(&str, &str)] = &[
        ("sequential (set)", "sequential"),
        ("clustered 256-run (set)", "clustered"),
        ("clustered 4096-run (set)", "clustered-wide"),
        ("random (set)", "random"),
        ("sparse `i << 40` (set)", "sparse"),
    ];
    let md = fs::read_to_string(repo_root().join("docs").join("BENCHMARKING.md"))
        .expect("read docs/BENCHMARKING.md");

    for (label, dist) in ROWS {
        let prefix = format!("| {label} |");
        let line = md
            .lines()
            .find(|l| l.starts_with(&prefix))
            .unwrap_or_else(|| {
                panic!("bytes/key table row {label:?} not found in BENCHMARKING.md")
            });

        // `| label | 1k | 100k | 1M | note |` -- cells may carry ** emphasis.
        let cells: Vec<f64> = line
            .split('|')
            .skip(2)
            .take(3)
            .map(|c| {
                c.trim()
                    .trim_matches('*')
                    .parse::<f64>()
                    .unwrap_or_else(|_| panic!("row {label:?}: cell {c:?} is not a number"))
            })
            .collect();
        assert_eq!(cells.len(), 3, "row {label:?} must publish 1k/100k/1M");

        for (cell, pop) in cells.iter().zip([1_000usize, 100_000, 1_000_000]) {
            let (set_bpk, _) = bytes_per_key(dist, pop);
            let expected = round_to(set_bpk, 2);
            assert!(
                (cell - expected).abs() < 1e-9,
                "docs/BENCHMARKING.md bytes/key, row {label:?} at pop {pop}: \
                 publishes {cell} B/key but the engine measures {expected} B/key. \
                 This table and docs/visualizer_data.json must both derive from \
                 `cargo run --release -p expanse-trie --example bytes_per_key` -- \
                 regenerate both rather than editing either by hand."
            );
        }
    }
}
/// A doc or harness may only advertise Callgrind cache columns if some
/// harness actually passes `--cache-sim=yes`.
///
/// `docs/BENCHMARKING.md` and `vs_stock.rs` both promised L1/LL/RAM columns
/// while neither Callgrind harness enabled the simulator, so a cache
/// hypothesis could be stated as if it had been measured. The columns are
/// fine to advertise once the flag is set; this asserts the two move
/// together. Same shape as the retracted-figure gate above: the repo gates
/// published values, and this gates the instrument behind them.
#[test]
fn test_cache_columns_claimed_only_if_cache_sim_enabled() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let harnesses = [
        "crates/expanse-capi/benches/vs_stock.rs",
        "crates/expanse/benches/instructions.rs",
    ];
    // Only a real argument counts. The flag also appears in the retraction
    // comments explaining that it is *not* passed, and matching those would
    // let the gate declare itself satisfied by its own prose.
    let enabled = harnesses.iter().any(|h| {
        std::fs::read_to_string(root.join(h))
            .unwrap_or_else(|e| panic!("gate cannot read {h}: {e}"))
            .lines()
            .any(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && t.contains("--cache-sim=yes")
            })
    });
    if enabled {
        return;
    }

    // Phrases that assert the columns exist, as opposed to naming a cache
    // level in prose. Retraction text is allowed to mention them.
    let banned = ["L1/LL/RAM count", "LL/RAM hit", "LL/RAM column"];
    let mut hits = Vec::new();
    for f in ["docs/BENCHMARKING.md"].iter().chain(harnesses.iter()) {
        let text = std::fs::read_to_string(root.join(f))
            .unwrap_or_else(|e| panic!("gate cannot read {f}: {e}"));
        for (i, line) in text.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            let retracting = lower.contains("does not emit")
                || lower.contains("not yet available")
                || lower.contains("claimed the columns");
            if !retracting && banned.iter().any(|b| line.contains(b)) {
                hits.push(format!("{f}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "no harness passes --cache-sim=yes, but these claim cache columns exist:\n  {}\n\
         Either pass --cache-sim=yes in a Callgrind harness or drop the claim.",
        hits.join("\n  ")
    );
}
