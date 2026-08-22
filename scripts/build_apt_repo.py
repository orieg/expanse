#!/usr/bin/env python3
"""Builds a standard Debian/Ubuntu APT repository layout from .deb files.

Outputs:
  <output_dir>/pool/main/*.deb
  <output_dir>/dists/stable/main/binary-<arch>/Packages[.gz]
  <output_dir>/dists/stable/Release
"""

import datetime
import gzip
import hashlib
import os
import shutil
import subprocess
import sys


def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def parse_control_from_deb(deb_path: str) -> dict[str, str]:
    """Extracts control fields from a .deb package."""
    fields = {}
    try:
        output = subprocess.check_output(
            ["dpkg-deb", "-f", deb_path], stderr=subprocess.DEVNULL, text=True
        )
        for line in output.splitlines():
            if ":" in line and not line.startswith(" "):
                k, v = line.split(":", 1)
                fields[k.strip()] = v.strip()
    except Exception:
        pass
    return fields


def build_apt_repo(input_dir: str, output_dir: str):
    if not os.path.isdir(input_dir):
        print(f"Directory {input_dir} does not exist.")
        return

    deb_files = [
        os.path.join(input_dir, f)
        for f in os.listdir(input_dir)
        if f.endswith(".deb")
    ]
    if not deb_files:
        print(f"No .deb files found in {input_dir}")
        return

    pool_dir = os.path.join(output_dir, "pool", "main")
    os.makedirs(pool_dir, exist_ok=True)

    arch_packages: dict[str, list[dict]] = {
        "amd64": [],
        "arm64": [],
        "riscv64": [],
    }

    for deb in deb_files:
        filename = os.path.basename(deb)
        dest_path = os.path.join(pool_dir, filename)
        shutil.copy2(deb, dest_path)

        size = os.path.getsize(dest_path)
        sha256 = sha256_file(dest_path)

        control = parse_control_from_deb(dest_path)
        pkg_name = control.get("Package", filename.split("_")[0])
        version = control.get(
            "Version", filename.split("_")[1] if "_" in filename else "0.2.0"
        )
        arch = control.get(
            "Architecture",
            filename.split("_")[2].replace(".deb", "")
            if "_" in filename
            else "amd64",
        )
        maintainer = control.get(
            "Maintainer", "Nicolas Brousse <nicolas@brousse.info>"
        )
        description = control.get(
            "Description", "Expanse trie engine: modern Judy replacement"
        )
        depends = control.get("Depends", "")
        provides = control.get("Provides", "")
        conflicts = control.get("Conflicts", "")

        entry = {
            "Package": pkg_name,
            "Version": version,
            "Architecture": arch,
            "Maintainer": maintainer,
            "Description": description,
            "Filename": f"pool/main/{filename}",
            "Size": str(size),
            "SHA256": sha256,
        }
        if depends:
            entry["Depends"] = depends
        if provides:
            entry["Provides"] = provides
        if conflicts:
            entry["Conflicts"] = conflicts

        if arch in arch_packages:
            arch_packages[arch].append(entry)
        else:
            arch_packages.setdefault(arch, []).append(entry)

    dists_dir = os.path.join(output_dir, "dists", "stable")
    file_manifests = []

    for arch, pkgs in arch_packages.items():
        if not pkgs:
            continue
        arch_dir = os.path.join(dists_dir, "main", f"binary-{arch}")
        os.makedirs(arch_dir, exist_ok=True)

        pkg_file_path = os.path.join(arch_dir, "Packages")
        with open(pkg_file_path, "w", encoding="utf-8") as f:
            for pkg in pkgs:
                for k, v in pkg.items():
                    f.write(f"{k}: {v}\n")
                f.write("\n")

        gz_file_path = f"{pkg_file_path}.gz"
        with open(pkg_file_path, "rb") as f_in, gzip.open(
            gz_file_path, "wb"
        ) as f_out:
            shutil.copyfileobj(f_in, f_out)

        rel_plain = os.path.relpath(pkg_file_path, dists_dir)
        rel_gz = os.path.relpath(gz_file_path, dists_dir)

        file_manifests.append(
            (rel_plain, os.path.getsize(pkg_file_path), sha256_file(pkg_file_path))
        )
        file_manifests.append(
            (rel_gz, os.path.getsize(gz_file_path), sha256_file(gz_file_path))
        )

    release_path = os.path.join(dists_dir, "Release")
    now_utc = datetime.datetime.now(datetime.timezone.utc).strftime(
        "%a, %d %b %Y %H:%M:%S UTC"
    )

    with open(release_path, "w", encoding="utf-8") as f:
        f.write("Origin: Expanse\n")
        f.write("Label: Expanse\n")
        f.write("Suite: stable\n")
        f.write("Codename: stable\n")
        f.write(f"Architectures: {' '.join([a for a, p in arch_packages.items() if p])}\n")
        f.write("Components: main\n")
        f.write("Description: Official Expanse APT Repository\n")
        f.write(f"Date: {now_utc}\n")
        f.write("SHA256:\n")
        for rel_path, size, sha256 in file_manifests:
            f.write(f" {sha256} {size} {rel_path}\n")

    print(f"APT repository successfully generated in {output_dir}")


if __name__ == "__main__":
    in_dir = sys.argv[1] if len(sys.argv) > 1 else "artifacts"
    out_dir = sys.argv[2] if len(sys.argv) > 2 else "apt"
    build_apt_repo(in_dir, out_dir)
