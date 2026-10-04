//! Parser and reader for `/proc/<pid>/task/<tid>/stat` field 39 (#1292).
//!
//! Linux `proc(5)` specifies that `/proc/<pid>/task/<tid>/stat` contains
//! process and thread status. Field 2 is `comm`, the filename of the
//! executable enclosed in parentheses `(...)`. Because `comm` may itself
//! contain spaces or parentheses (including `)` followed by a space), parsing
//! cannot simply split on whitespace from the beginning of the line.
//! Instead, field 39 (`processor`, the CPU number last executed on) is
//! parsed by finding the LAST closing parenthesis `')'`, and taking field 36
//! (0-indexed, where field 3 `state` is index 0; `39 - 3 = 36`) of the
//! whitespace-separated fields after it.
use std::time::Duration;

/// The cadence at which worker thread CPU placement is sampled from
/// `/proc/<pid>/task/<tid>/stat` field 39 during the 500 ms measurement window (#1292).
pub const STAT_SAMPLE_CADENCE: Duration = Duration::from_millis(50);

/// Parses field 39 (the processor / CPU number the task last executed on)
/// from a Linux `/proc/<pid>/task/<tid>/stat` string.
///
/// Returns `Some(cpu)` if field 39 exists and parses as `i32`, or `None` otherwise.
pub fn parse_stat_cpu(stat_content: &str) -> Option<i32> {
    let rparen = stat_content.rfind(')')?;
    let rest = &stat_content[rparen + 1..];
    // Field 3 (state) is index 0 of rest.split_whitespace().
    // Field 39 (processor) is index 36 (39 - 3).
    let field = rest.split_whitespace().nth(36)?;
    field.parse::<i32>().ok()
}

/// Reads and parses field 39 from a stat file at `path`.
pub fn read_proc_stat_file(path: impl AsRef<std::path::Path>) -> Option<i32> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_stat_cpu(&content)
}

/// Reads and parses field 39 from `/proc/<pid>/task/<tid>/stat`.
#[cfg(target_os = "linux")]
pub fn read_proc_stat_cpu(pid: u32, tid: i32) -> Option<i32> {
    if tid < 0 {
        return None;
    }
    let path = format!("/proc/{pid}/task/{tid}/stat");
    read_proc_stat_file(path)
}

/// On non-Linux targets, `/proc` is not available.
#[cfg(not(target_os = "linux"))]
pub fn read_proc_stat_cpu(_pid: u32, _tid: i32) -> Option<i32> {
    if false {
        let _ = read_proc_stat_file("");
        let _ = parse_stat_cpu("");
    }
    None
}

/// Returns the current thread's thread ID (TID) on Linux.
#[cfg(target_os = "linux")]
pub fn get_thread_id() -> i32 {
    // SAFETY: SYS_gettid takes no arguments and has no preconditions.
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

/// On non-Linux targets, returns -1.
#[cfg(not(target_os = "linux"))]
pub fn get_thread_id() -> i32 {
    -1
}
