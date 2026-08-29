#!/usr/bin/env python3
"""
scripts/bump_version.py — Multi-Ecosystem Version Synchronizer for Expanse.

Synchronizes and validates version strings across all 12 package manifests:
  1. Cargo.toml (workspace [workspace.package] version)
  2. crates/expanse/Cargo.toml ([package] version)
  3. crates/expanse-capi/Cargo.toml ([package] version & expanse-trie dependency)
  4. crates/expanse-py/Cargo.toml ([package] version & expanse-trie dependency)
  5. crates/expanse-node/Cargo.toml ([package] version & expanse-trie dependency)
  6. crates/expanse-node/package.json ("version")
  7. pyproject.toml ([project] version)
  8. bindings/dotnet/src/Expanse.NET/Expanse.NET.csproj (<Version>, <PackageVersion>, <AssemblyVersion>)
  9. bindings/java/pom.xml (<version>)
 10. bindings/java/build.gradle (version = '...')
 11. bindings/ruby/expanse.gemspec (spec.version = '...')
 12. bindings/ruby/lib/expanse.rb (VERSION = '...')
 (And extra manifests: extra/vcpkg/vcpkg.json, extra/nuget/expanse.nuspec)

Usage:
  python3 scripts/bump_version.py <NEW_VERSION> [--dry-run]
  python3 scripts/bump_version.py --check [<EXPECTED_VERSION>]
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

SEMVER_REGEX = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
)


def get_repo_root() -> Path:
    """Returns the repository root directory."""
    script_dir = Path(__file__).resolve().parent
    return script_dir.parent


def validate_semver(version: str) -> bool:
    """Validates semantic versioning format."""
    return bool(SEMVER_REGEX.match(version))


class ManifestHandler:
    def __init__(self, rel_path: str, name: str):
        self.rel_path = rel_path
        self.name = name

    def get_path(self, root: Path) -> Path:
        return root / self.rel_path

    def exists(self, root: Path) -> bool:
        return self.get_path(root).is_file()

    def get_versions(self, root: Path) -> Dict[str, str]:
        """Extracts version strings from the manifest."""
        raise NotImplementedError

    def set_version(self, root: Path, new_version: str) -> str:
        """Updates manifest with new_version and returns the updated text."""
        raise NotImplementedError


class RootCargoHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r'\[workspace\.package\][^\[]*?version\s*=\s*"([^"]+)"', text, re.DOTALL)
        if match:
            return {"workspace.package": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        # Check if version exists in [workspace.package]
        if re.search(r'(\[workspace\.package\][^\[]*?version\s*=\s*)"([^"]+)"', text, re.DOTALL):
            def repl(m: re.Match) -> str:
                return f'{m.group(1)}"{new_version}"'
            new_text = re.sub(
                r'(\[workspace\.package\][^\[]*?version\s*=\s*)"[^"]+"',
                repl,
                text,
                count=1,
                flags=re.DOTALL,
            )
        else:
            # Insert version right after [workspace.package]
            new_text = re.sub(
                r'(\[workspace\.package\]\n)',
                rf'\1version = "{new_version}"\n',
                text,
                count=1,
            )
        return new_text


class CrateCargoHandler(ManifestHandler):
    def __init__(self, rel_path: str, name: str, has_internal_dep: bool = False):
        super().__init__(rel_path, name)
        self.has_internal_dep = has_internal_dep

    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        res = {}
        pkg_match = re.search(r'\[package\][^\[]*?version\s*=\s*"([^"]+)"', text, re.DOTALL)
        if pkg_match:
            res["package.version"] = pkg_match.group(1)
        if self.has_internal_dep:
            dep_match = re.search(r'expanse-trie\s*=\s*\{[^}]*version\s*=\s*"([^"]+)"', text)
            if dep_match:
                res["dep:expanse-trie"] = dep_match.group(1)
        return res

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        # Replace [package] version
        new_text = re.sub(
            r'(\[package\][^\[]*?version\s*=\s*)"[^"]+"',
            rf'\g<1>"{new_version}"',
            text,
            count=1,
            flags=re.DOTALL,
        )
        if self.has_internal_dep:
            new_text = re.sub(
                r'(expanse-trie\s*=\s*\{[^}]*version\s*=\s*)"[^"]+"',
                rf'\g<1>"{new_version}"',
                new_text,
            )
        return new_text


class PackageJsonHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r'"version"\s*:\s*"([^"]+)"', text)
        if match:
            return {"version": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        text = re.sub(
            r'("version"\s*:\s*)"[^"]+"',
            rf'\g<1>"{new_version}"',
            text,
            count=1,
        )
        # Keep the napi platform optionalDependencies (@orieg/expanse-<platform>)
        # in lockstep so the published main package pins the matching prebuilds.
        text = re.sub(
            r'("@orieg/expanse-[a-z0-9-]+"\s*:\s*)"[^"]+"',
            rf'\g<1>"{new_version}"',
            text,
        )
        return text


class PyprojectHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r'\[project\][^\[]*?version\s*=\s*"([^"]+)"', text, re.DOTALL)
        if match:
            return {"project.version": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        return re.sub(
            r'(\[project\][^\[]*?version\s*=\s*)"[^"]+"',
            rf'\g<1>"{new_version}"',
            text,
            count=1,
            flags=re.DOTALL,
        )


class CsprojHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        res = {}
        for tag in ["Version", "PackageVersion", "AssemblyVersion"]:
            m = re.search(rf"<{tag}>([^<]+)</{tag}>", text)
            if m:
                res[tag] = m.group(1)
        return res

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        new_text = text
        for tag in ["Version", "PackageVersion", "AssemblyVersion"]:
            if re.search(rf"<{tag}>[^<]+</{tag}>", new_text):
                new_text = re.sub(
                    rf"<{tag}>[^<]+</{tag}>",
                    rf"<{tag}>{new_version}</{tag}>",
                    new_text,
                )
            else:
                # Add after <PackageId> if present or at first PropertyGroup
                if tag in ["PackageVersion", "AssemblyVersion"] and "<Version>" in new_text:
                    new_text = re.sub(
                        r"(<Version>[^<]+</Version>)",
                        rf"\1\n    <{tag}>{new_version}</{tag}>",
                        new_text,
                        count=1,
                    )
        return new_text


class PomXmlHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        # Match project version right after artifactId expanse-java
        match = re.search(
            r"<artifactId>expanse-java</artifactId>\s*<version>([^<]+)</version>", text
        )
        if match:
            return {"version": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        return re.sub(
            r"(<artifactId>expanse-java</artifactId>\s*<version>)[^<]+(</version>)",
            rf"\g<1>{new_version}\g<2>",
            text,
            count=1,
        )


class BuildGradleHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r"(?m)^version\s*=\s*['\"]([^'\"]+)['\"]", text)
        if match:
            return {"version": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        return re.sub(
            r"(?m)^(version\s*=\s*)['\"][^'\"]+['\"]",
            rf"\g<1>'{new_version}'",
            text,
            count=1,
        )


class JsonVersionHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r'"version"\s*:\s*"([^"]+)"', text)
        if match:
            return {"version": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        return re.sub(
            r'("version"\s*:\s*)"[^"]+"',
            rf'\g<1>"{new_version}"',
            text,
            count=1,
        )


# Versioned snippet patterns shared by the doc-pin handlers below. Each is a
# 3-group regex (prefix, version, suffix) and MUST be unique within the file it
# is registered for, so the lockstep check fails loudly when a doc rewrite
# drops or duplicates a pin instead of silently untracking it.
PIN_CARGO_DEP = re.compile(r'(expanse-trie = ")([^"]+)(")')
PIN_WINDOWS_BUNDLE = re.compile(r"(expanse-v)([0-9]+\.[0-9]+\.[0-9]+)(-x86_64-pc-windows-msvc\.zip)")
PIN_MAVEN_SNIPPET = re.compile(r"(<version>)([^<]+)(</version>)")
PIN_ESP_IDF = re.compile(r'(version: "\^)([^"]+)(")')
PIN_NUGET_PACKAGEREF = re.compile(r'(<PackageReference Include="Orieg\.Expanse" Version=")([^"]+)(")')
# CITATION.cff `version:` is validated against the workspace by
# scripts/validate_citation.py, so it has to move with the bump.
PIN_CFF_VERSION = re.compile(r"(^version: )(\d[^\s]*)($)", re.MULTILINE)
# Go modules resolve by git tag, so the install snippet must track the release.
PIN_GO_MODULE = re.compile(
    r"(go get github\.com/orieg/expanse/bindings/go@v)(\d[^\s]*)(\s|$)", re.MULTILINE
)


class DocPinsHandler(ManifestHandler):
    """Versioned snippets inside READMEs/docs (install examples, bundle names).

    Untracked doc pins are how stale versions leak into published pages; a
    hardcoded literal of this class broke the v0.4.1 bump. `pins` is a list of
    (name, pattern) using the shared PIN_* patterns above.
    """

    def __init__(self, rel_path: str, name: str, pins: List[Tuple[str, "re.Pattern[str]"]]):
        super().__init__(rel_path, name)
        self.pins = pins

    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        out: Dict[str, str] = {}
        for name, pat in self.pins:
            match = pat.search(text)
            if match:
                out[name] = match.group(2)
        return out

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        for _, pat in self.pins:
            text = pat.sub(rf"\g<1>{new_version}\g<3>", text, count=1)
        return text


class GemspecHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r'spec\.version\s*=\s*"([^"]+)"', text)
        if match:
            return {"spec.version": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        return re.sub(
            r'(spec\.version\s*=\s*)"[^"]+"',
            rf'\g<1>"{new_version}"',
            text,
            count=1,
        )


class RubyVersionHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r'VERSION\s*=\s*"([^"]+)"', text)
        if match:
            return {"VERSION": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        return re.sub(
            r'(VERSION\s*=\s*)"[^"]+"',
            rf'\g<1>"{new_version}"',
            text,
            count=1,
        )


class NuspecHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r"<version>([^<]+)</version>", text)
        if match:
            return {"version": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        return re.sub(
            r"(<version>)[^<]+(</version>)",
            rf"\g<1>{new_version}\g<2>",
            text,
            count=1,
        )


class YamlVersionHandler(ManifestHandler):
    def get_versions(self, root: Path) -> Dict[str, str]:
        text = self.get_path(root).read_text(encoding="utf-8")
        match = re.search(r'^version\s*:\s*"([^"]+)"', text, re.MULTILINE)
        if match:
            return {"version": match.group(1)}
        return {}

    def set_version(self, root: Path, new_version: str) -> str:
        text = self.get_path(root).read_text(encoding="utf-8")
        return re.sub(
            r'^(version\s*:\s*)"[^"]+"',
            rf'\g<1>"{new_version}"',
            text,
            count=1,
            flags=re.MULTILINE,
        )


def get_handlers(root: Path) -> List[ManifestHandler]:
    """Returns the list of configured manifest handlers."""
    dotnet_path = "bindings/dotnet/src/Expanse.NET/Expanse.NET.csproj"
    if not (root / dotnet_path).exists() and (root / "bindings/dotnet/Expanse.NET.csproj").exists():
        dotnet_path = "bindings/dotnet/Expanse.NET.csproj"

    handlers: List[ManifestHandler] = [
        RootCargoHandler("Cargo.toml", "Cargo.toml (workspace)"),
        CrateCargoHandler("crates/expanse/Cargo.toml", "crates/expanse/Cargo.toml"),
        CrateCargoHandler(
            "crates/expanse-capi/Cargo.toml",
            "crates/expanse-capi/Cargo.toml",
            has_internal_dep=True,
        ),
        CrateCargoHandler(
            "crates/expanse-py/Cargo.toml",
            "crates/expanse-py/Cargo.toml",
            has_internal_dep=True,
        ),
        CrateCargoHandler(
            "crates/expanse-node/Cargo.toml",
            "crates/expanse-node/Cargo.toml",
            has_internal_dep=True,
        ),
        CrateCargoHandler(
            "crates/expanse-php/Cargo.toml",
            "crates/expanse-php/Cargo.toml",
            has_internal_dep=True,
        ),
        CrateCargoHandler(
            "crates/expanse-rb/Cargo.toml",
            "crates/expanse-rb/Cargo.toml",
            has_internal_dep=False,
        ),
        CrateCargoHandler(
            "crates/expanse-wasm/Cargo.toml",
            "crates/expanse-wasm/Cargo.toml",
            has_internal_dep=False,
        ),
        PackageJsonHandler("crates/expanse-node/package.json", "crates/expanse-node/package.json"),
        PackageJsonHandler("crates/expanse-wasm/package.json", "crates/expanse-wasm/package.json"),
        JsonVersionHandler("bindings/php/composer.json", "bindings/php/composer.json"),
        PyprojectHandler("pyproject.toml", "pyproject.toml"),
        CsprojHandler(
            dotnet_path,
            dotnet_path,
        ),
        PomXmlHandler("bindings/java/pom.xml", "bindings/java/pom.xml"),
        BuildGradleHandler("bindings/java/build.gradle", "bindings/java/build.gradle"),
        GemspecHandler("bindings/ruby/expanse.gemspec", "bindings/ruby/expanse.gemspec"),
        RubyVersionHandler("bindings/ruby/lib/expanse.rb", "bindings/ruby/lib/expanse.rb"),
        YamlVersionHandler(
            "components/expanse/idf_component.yml",
            "components/expanse/idf_component.yml",
        ),
        DocPinsHandler(
            "README.md",
            "README.md (version pins)",
            [
                ("cargo-dep", PIN_CARGO_DEP),
                ("windows-bundle", PIN_WINDOWS_BUNDLE),
                ("maven-snippet", PIN_MAVEN_SNIPPET),
                ("esp-idf", PIN_ESP_IDF),
            ],
        ),
        DocPinsHandler(
            "CITATION.cff",
            "CITATION.cff (version pin)",
            [("version", PIN_CFF_VERSION)],
        ),
        DocPinsHandler(
            "bindings/go/README.md",
            "bindings/go/README.md (version pins)",
            [("go-module", PIN_GO_MODULE)],
        ),
        DocPinsHandler(
            "docs/PACKAGING.md",
            "docs/PACKAGING.md (version pins)",
            [
                ("nuget-packageref", PIN_NUGET_PACKAGEREF),
                ("esp-idf", PIN_ESP_IDF),
                ("go-module", PIN_GO_MODULE),
            ],
        ),
        DocPinsHandler(
            "docs/bindings/java.md",
            "docs/bindings/java.md (version pins)",
            [("maven-snippet", PIN_MAVEN_SNIPPET)],
        ),
        DocPinsHandler(
            "components/expanse/README.md",
            "components/expanse/README.md (version pins)",
            [("esp-idf", PIN_ESP_IDF)],
        ),
    ]

    # Check for optional manifests
    optional_handlers = [
        JsonVersionHandler("extra/vcpkg/vcpkg.json", "extra/vcpkg/vcpkg.json"),
        NuspecHandler("extra/nuget/expanse.nuspec", "extra/nuget/expanse.nuspec"),
    ]
    return handlers + optional_handlers


def run_check(root: Path, expected_version: Optional[str] = None) -> int:
    """Verifies that all manifests are in 100% lockstep."""
    handlers = get_handlers(root)
    all_versions: Dict[str, Dict[str, str]] = {}
    found_versions: set[str] = set()

    print("=" * 70)
    print("Expanse Multi-Ecosystem Version Lockstep Check")
    print("=" * 70)

    for h in handlers:
        if not h.exists(root):
            # Optional handlers might not exist; core manifests should
            if h.rel_path.startswith("extra/"):
                continue
            print(f"[ERROR] Missing manifest: {h.rel_path}")
            return 1

        versions = h.get_versions(root)
        if not versions:
            print(f"[ERROR] Could not parse any version from {h.rel_path}")
            return 1

        all_versions[h.rel_path] = versions
        for v_name, v_val in versions.items():
            found_versions.add(v_val)
            print(f"  ✓ {h.rel_path:<52} [{v_name}] = {v_val}")

    print("-" * 70)

    if expected_version:
        if not validate_semver(expected_version):
            print(f"[ERROR] Specified expected version '{expected_version}' is not valid SemVer.")
            return 1
        mismatches = []
        for path, vdict in all_versions.items():
            for vname, vval in vdict.items():
                if vval != expected_version:
                    mismatches.append((path, vname, vval, expected_version))

        if mismatches:
            print(f"[FAILED] Version mismatch against expected '{expected_version}':")
            for path, vname, vval, exp in mismatches:
                print(f"  - {path} ({vname}): found '{vval}', expected '{exp}'")
            return 1
        print(f"[SUCCESS] All manifests match expected version: {expected_version}")
        return 0

    # No specific version given: verify all found versions are identical
    if len(found_versions) == 1:
        v = found_versions.pop()
        print(f"[SUCCESS] All {len(all_versions)} manifests are in 100% lockstep at version: {v}")
        return 0
    else:
        print(f"[FAILED] Multiple differing versions detected across manifests: {found_versions}")
        for path, vdict in all_versions.items():
            for vname, vval in vdict.items():
                print(f"  - {path:<50} {vname} = {vval}")
        return 1


def run_bump(root: Path, new_version: str, dry_run: bool = False, no_cargo_check: bool = False) -> int:
    """Updates version strings across all manifests."""
    if not validate_semver(new_version):
        print(f"[ERROR] Invalid SemVer version: '{new_version}'")
        return 1

    handlers = get_handlers(root)

    print("=" * 70)
    mode_str = "[DRY RUN] " if dry_run else ""
    print(f"{mode_str}Bumping Expanse multi-ecosystem version -> {new_version}")
    print("=" * 70)

    updated_count = 0
    for h in handlers:
        if not h.exists(root):
            if h.rel_path.startswith("extra/"):
                continue
            print(f"[ERROR] Required manifest missing: {h.rel_path}")
            return 1

        old_versions = h.get_versions(root)
        old_str = ", ".join(f"{k}={v}" for k, v in old_versions.items())
        new_content = h.set_version(root, new_version)

        if not dry_run:
            h.get_path(root).write_text(new_content, encoding="utf-8")

        print(f"  {'[DRY-RUN]' if dry_run else '[UPDATED]'} {h.rel_path:<52} ({old_str}) -> {new_version}")
        updated_count += 1

    if not dry_run and not no_cargo_check:
        print("-" * 70)
        print("Re-generating Cargo.lock via 'cargo check --workspace'...")
        try:
            subprocess.run(
                ["cargo", "check", "--workspace"],
                cwd=root,
                check=True,
                capture_output=True,
                text=True,
            )
            print("  ✓ Cargo.lock synchronized successfully.")
        except subprocess.CalledProcessError as e:
            print(f"[ERROR] cargo check failed:\n{e.stderr}")
            return 1
        except FileNotFoundError:
            print("[WARN] 'cargo' command not found. Skipping Cargo.lock re-generation.")

    print("=" * 70)
    print(f"[SUCCESS] {updated_count} manifests updated to version {new_version}.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Expanse multi-ecosystem version bump & verification tool."
    )
    parser.add_argument(
        "version",
        nargs="?",
        default=None,
        help="New version string (e.g., 0.4.0) or expected version when used with --check",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify that all manifests are in 100% version lockstep",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate the version bump without modifying files",
    )
    parser.add_argument(
        "--no-cargo-check",
        action="store_true",
        help="Skip running 'cargo check --workspace' after version bump",
    )

    args = parser.parse_args()
    root = get_repo_root()

    if args.check:
        return run_check(root, args.version)

    if not args.version:
        parser.error("Version argument is required unless --check is specified.")

    return run_bump(root, args.version, dry_run=args.dry_run, no_cargo_check=args.no_cargo_check)


if __name__ == "__main__":
    sys.exit(main())
