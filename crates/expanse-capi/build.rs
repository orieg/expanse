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

    link_soname();
}

/// The `DT_SONAME` a packaged Linux `libexpanse` carries. `libexpanse1`,
/// `libjudy-compat`'s `libJudy.so.1 -> libexpanse.so.1` link and the `-lJudy`
/// dev link all name it (docs/COMPAT.md, "Library names and SONAME").
const SONAME: &str = "libexpanse.so.1";

/// Records [`SONAME`] in the Linux cdylib when `EXPANSE_LINK_SONAME=1`.
///
/// Opt-in, because cargo emits only the unversioned `libexpanse.so`: a library
/// carrying `DT_SONAME libexpanse.so.1` makes every consumer that links it
/// record `DT_NEEDED libexpanse.so.1`, a file no `target/` directory holds, so
/// the in-tree consumers that link `target/release/libexpanse.so` directly would
/// stop loading. The release build sets it; `scripts/package_deb.sh` and
/// `scripts/package_rpm.sh` refuse a library without it.
fn link_soname() {
    println!("cargo:rerun-if-env-changed=EXPANSE_LINK_SONAME");
    match env::var("EXPANSE_LINK_SONAME").as_deref() {
        Err(_) | Ok("") | Ok("0") => {}
        Ok("1") => {
            if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
                println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,{SONAME}");
            } else {
                println!(
                    "cargo:warning=EXPANSE_LINK_SONAME=1 ignored: DT_SONAME applies to Linux targets only"
                );
            }
        }
        Ok(other) => panic!("EXPANSE_LINK_SONAME must be unset, 0 or 1, not {other:?}"),
    }
}
