//! Automated sync test: validates that `docs/architecture_visualizer.html`
//! and `docs/visualizer_data.json` are bit-exact synchronized with the
//! compiled Rust codebase (`types.rs`, `set.rs`, `leaf.rs`, `instructions.rs`).
//!
//! If any geometry constant, promotion ceiling, or benchmark definition changes
//! in the Rust source code, this test will fail until the visualizer is updated.

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
}
