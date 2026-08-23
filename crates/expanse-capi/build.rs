//! Build script for `expanse-capi` providing build-time Git SHA / -dev version stamping.

use std::env;
use std::process::Command;

fn main() {
    let pkg_version = env::var("CARGO_PKG_VERSION").unwrap_or_default();

    println!("cargo:rerun-if-changed=../../.git/HEAD");

    let git_describe = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            } else {
                None
            }
        });

    let version_full = match git_describe {
        Some(describe) => {
            let release_tag = format!("v{pkg_version}");
            if describe == release_tag {
                pkg_version
            } else {
                format!("{pkg_version}-dev ({describe})")
            }
        }
        None => pkg_version,
    };

    println!("cargo:rustc-env=EXPANSE_VERSION_FULL={version_full}");
}
