//! Unit tests for `/proc/<pid>/task/<tid>/stat` field 39 parser (#1292).

#![cfg(not(miri))]

#[path = "../benches/concurrency_stat.rs"]
mod concurrency_stat;

use concurrency_stat::{
    get_thread_id, parse_stat_cpu, read_proc_stat_cpu, read_proc_stat_file, STAT_SAMPLE_CADENCE,
};

#[test]
fn test_cadence_and_thread_id() {
    assert_eq!(STAT_SAMPLE_CADENCE.as_millis(), 50);
    let _tid = get_thread_id();
    assert_eq!(read_proc_stat_cpu(999_999_999, -1), None);
}

#[test]
fn test_parse_stat_cpu_standard() {
    // 52 fields total. Field 39 is '5'.
    let stat = "1234 (expanse_bench) R 1000 1234 1000 0 -1 4194304 300 0 0 0 10 5 0 0 20 0 1 0 12345 100000 500 18446744073709551615 4194304 4200000 140737488349000 0 0 0 0 0 0 0 0 0 17 5 0 0 0 0 0 0 0 0 0 0 0";
    assert_eq!(parse_stat_cpu(stat), Some(5));
}

#[test]
fn test_parse_stat_cpu_comm_with_spaces() {
    // comm has spaces: (reader thread 0). Field 39 is '11'.
    let stat = "5678 (reader thread 0) S 1000 1234 1000 0 -1 4194304 300 0 0 0 10 5 0 0 20 0 1 0 12345 100000 500 18446744073709551615 4194304 4200000 140737488349000 0 0 0 0 0 0 0 0 0 17 11 0 0 0 0 0 0 0 0 0 0 0";
    assert_eq!(parse_stat_cpu(stat), Some(11));
}

#[test]
fn test_parse_stat_cpu_comm_with_nested_parens() {
    // comm has nested parentheses: (worker (1)). Field 39 is '3'.
    let stat = "9999 (worker (1)) R 1000 1234 1000 0 -1 4194304 300 0 0 0 10 5 0 0 20 0 1 0 12345 100000 500 18446744073709551615 4194304 4200000 140737488349000 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0 0 0 0 0 0 0";
    assert_eq!(parse_stat_cpu(stat), Some(3));
}

#[test]
fn test_parse_stat_cpu_comm_with_rparen_space_inside() {
    // Required by prompt: "Unit-test the parser on fixtures including a comm with ') ' inside."
    // comm contains ') ': (worker) 1) task). Field 39 is '14'.
    let stat = "8888 (worker) 1) task) R 1000 1234 1000 0 -1 4194304 300 0 0 0 10 5 0 0 20 0 1 0 12345 100000 500 18446744073709551615 4194304 4200000 140737488349000 0 0 0 0 0 0 0 0 0 17 14 0 0 0 0 0 0 0 0 0 0 0";
    assert_eq!(parse_stat_cpu(stat), Some(14));
}

#[test]
fn test_parse_stat_cpu_malformed_and_truncated() {
    // Missing closing paren
    assert_eq!(parse_stat_cpu("1234 (unclosed R 1 2 3"), None);

    // Truncated line (fewer than 39 fields)
    assert_eq!(parse_stat_cpu("1234 (comm) R 1 2 3 4 5"), None);

    // Field 39 is non-numeric
    let bad_field = "1234 (comm) R 1000 1234 1000 0 -1 4194304 300 0 0 0 10 5 0 0 20 0 1 0 12345 100000 500 18446744073709551615 4194304 4200000 140737488349000 0 0 0 0 0 0 0 0 0 17 NOT_A_CPU 0 0 0 0 0 0 0 0 0 0 0";
    assert_eq!(parse_stat_cpu(bad_field), None);
}

#[test]
fn test_read_proc_stat_file_fixture() {
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join("test_proc_stat_fixture.txt");
    let content = "1234 (worker) name) R 1000 1234 1000 0 -1 4194304 300 0 0 0 10 5 0 0 20 0 1 0 12345 100000 500 18446744073709551615 4194304 4200000 140737488349000 0 0 0 0 0 0 0 0 0 17 8 0 0 0 0 0 0 0 0 0 0 0\n";
    std::fs::write(&tmp_path, content).expect("write temp fixture");
    assert_eq!(read_proc_stat_file(&tmp_path), Some(8));
    let _ = std::fs::remove_file(&tmp_path);
}
