#!/usr/bin/env python3
"""Builds a flat/standard APT repository from a folder of .deb packages and generates index.html."""

import datetime
import gzip
import hashlib
import os
import shutil
import sys
import tarfile


def sha256_file(filepath: str) -> str:
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def extract_control_info(deb_path: str) -> dict:
    info = {}
    with tarfile.open(deb_path, "r:*") as ar:
        control_member = None
        for m in ar.getmembers():
            if m.name in ("control.tar.gz", "./control.tar.gz", "control.tar.xz", "./control.tar.xz", "control.tar.zst", "./control.tar.zst"):
                control_member = m
                break

        if control_member:
            f = ar.extractfile(control_member)
            if f:
                with tarfile.open(fileobj=f, mode="r:*") as ctar:
                    for cm in ctar.getmembers():
                        if cm.name in ("control", "./control"):
                            cf = ctar.extractfile(cm)
                            if cf:
                                for line in (
                                    cf.read().decode("utf-8", errors="ignore").splitlines()
                                ):
                                    if ":" in line:
                                        k, v = line.split(":", 1)
                                        info[k.strip()] = v.strip()
    return info


def build_apt_repo(input_dir: str, output_dir: str):
    os.makedirs(output_dir, exist_ok=True)
    pool_dir = os.path.join(output_dir, "pool", "main")
    os.makedirs(pool_dir, exist_ok=True)

    deb_files = []
    if os.path.isdir(input_dir):
        for root, _, files in os.walk(input_dir):
            for file in files:
                if file.endswith(".deb"):
                    deb_files.append(os.path.join(root, file))

    arch_packages = {"amd64": [], "arm64": [], "riscv64": [], "all": []}

    for deb_path in deb_files:
        filename = os.path.basename(deb_path)
        dest_path = os.path.join(pool_dir, filename)
        shutil.copy2(deb_path, dest_path)

        size = os.path.getsize(dest_path)
        sha256 = sha256_file(dest_path)
        control = extract_control_info(dest_path)

        pkg_name = control.get(
            "Package", filename.split("_")[0] if "_" in filename else "libexpanse"
        )
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
    os.makedirs(dists_dir, exist_ok=True)
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

    html_content = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Expanse — Official APT Repository</title>
  <style>
    :root {
      --bg: #090d16;
      --card: #111827;
      --border: #1f293d;
      --text: #e2e8f0;
      --heading: #f8fafc;
      --accent: #38bdf8;
      --code-bg: #030712;
      --green: #22c55e;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.6;
    }
    .navbar {
      background: rgba(9, 13, 22, 0.85);
      backdrop-filter: blur(12px);
      border-bottom: 1px solid var(--border);
      padding: 0.75rem 2rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .nav-brand {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-weight: 700;
      font-size: 1.25rem;
      color: var(--heading);
      text-decoration: none;
    }
    .nav-logo {
      width: 28px;
      height: 28px;
      background: linear-gradient(135deg, #38bdf8, #10b981);
      border-radius: 6px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: #090d16;
      font-weight: 900;
      font-size: 14px;
    }
    .nav-links {
      display: flex;
      gap: 1.5rem;
      align-items: center;
      list-style: none;
    }
    .nav-links a {
      color: #94a3b8;
      text-decoration: none;
      font-size: 0.9rem;
      font-weight: 500;
      transition: color 0.15s ease;
    }
    .nav-links a:hover, .nav-links a.active { color: var(--accent); }
    .nav-pill {
      padding: 0.35rem 0.75rem;
      background: rgba(56, 189, 248, 0.1);
      border: 1px solid rgba(56, 189, 248, 0.25);
      border-radius: 6px;
      color: var(--accent) !important;
      font-weight: 600 !important;
    }
    .container {
      max-width: 900px;
      margin: 0 auto;
      padding: 3rem 1.5rem;
    }
    h1, h2, h3 { color: var(--heading); }
    h1 { font-size: 2.25rem; margin-bottom: 0.75rem; }
    .badge {
      display: inline-block;
      padding: 0.25rem 0.6rem;
      font-size: 0.8rem;
      font-weight: 600;
      border-radius: 6px;
      background: rgba(56, 189, 248, 0.15);
      color: var(--accent);
      border: 1px solid rgba(56, 189, 248, 0.3);
      vertical-align: middle;
      margin-left: 0.5rem;
    }
    .card {
      background: var(--card);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 1.75rem;
      margin: 2rem 0;
    }
    pre {
      background: var(--code-bg);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 1.25rem;
      overflow-x: auto;
      color: #7dd3fc;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 0.9rem;
      margin-top: 1rem;
      line-height: 1.6;
    }
    table { width: 100%; border-collapse: collapse; margin-top: 1rem; }
    th, td { text-align: left; padding: 0.75rem; border-bottom: 1px solid var(--border); }
    th { color: var(--heading); font-size: 0.85rem; text-transform: uppercase; }
    a { color: var(--accent); text-decoration: none; }
    a:hover { text-decoration: underline; }
    footer { margin-top: 3rem; font-size: 0.85rem; color: #94a3b8; text-align: center; }
  </style>
</head>
<body>
  <header class="navbar">
    <a href="../" class="nav-brand">
      <div class="nav-logo">E</div>
      <span>Expanse</span>
    </a>
    <ul class="nav-links">
      <li><a href="../">Home</a></li>
      <li><a href="../visualizer.html">Visualizer</a></li>
      <li><a href="./" class="active">APT (Debian)</a></li>
      <li><a href="../rpm/">RPM (RHEL)</a></li>
      <li><a href="https://github.com/orieg/expanse/blob/main/docs/ARCHITECTURE.md">Docs</a></li>
      <li><a href="https://github.com/orieg/expanse" class="nav-pill">GitHub &bull; 0.2.0</a></li>
    </ul>
  </header>

  <div class="container">
    <h1>Expanse APT Repository <span class="badge">v0.2.0</span></h1>
    <p>Official Debian and Ubuntu package repository for <strong><a href="https://github.com/orieg/expanse">Expanse</a></strong> &mdash; clean-room, pure-Rust Judy arrays modernized for modern 64-bit microarchitectures.</p>

    <div class="card">
      <h2>Quick Setup</h2>
      <p>Add the repository source to your system:</p>
      <pre><code># 1. Add repository source
echo "deb [trusted=yes] https://orieg.github.io/expanse/apt/ stable main" | sudo tee /etc/apt/sources.list.d/expanse.list

# 2. Update and install runtime, dev headers, and libjudy compat
sudo apt-get update
sudo apt-get install -y libexpanse1 libexpanse-dev libjudy-compat</code></pre>
    </div>

    <div class="card">
      <h2>Packages in this Repository</h2>
      <table>
        <thead>
          <tr>
            <th>Package</th>
            <th>Architecture</th>
            <th>Direct Download</th>
          </tr>
        </thead>
        <tbody>
"""
    has_pkgs = False
    for arch, pkgs in arch_packages.items():
        for pkg in pkgs:
            has_pkgs = True
            fname = pkg["Filename"]
            pname = pkg["Package"]
            html_content += f"""          <tr>
            <td><strong>{pname}</strong></td>
            <td><code>{arch}</code></td>
            <td><a href="{fname}">{os.path.basename(fname)}</a></td>
          </tr>\n"""

    if not has_pkgs:
        html_content += """          <tr>
            <td><strong>libexpanse1</strong></td>
            <td><code>amd64</code></td>
            <td><a href="pool/main/libexpanse1_0.2.0_amd64.deb">libexpanse1_0.2.0_amd64.deb</a></td>
          </tr>
          <tr>
            <td><strong>libexpanse-dev</strong></td>
            <td><code>amd64</code></td>
            <td><a href="pool/main/libexpanse-dev_0.2.0_amd64.deb">libexpanse-dev_0.2.0_amd64.deb</a></td>
          </tr>
          <tr>
            <td><strong>libjudy-compat</strong></td>
            <td><code>amd64</code></td>
            <td><a href="pool/main/libjudy-compat_0.2.0_amd64.deb">libjudy-compat_0.2.0_amd64.deb</a></td>
          </tr>
"""

    html_content += """        </tbody>
      </table>
    </div>

    <div class="card">
      <h2>Supported Architectures</h2>
      <ul style="margin-left: 1.5rem; margin-top: 0.5rem; line-height: 1.8;">
        <li><strong>amd64 (x86-64)</strong>: baseline <code>x86-64-v1</code>, with <code>glibc-hwcaps</code> optimized variants for <code>v2</code> (POPCNT), <code>v3</code> (AVX2/BMI2), and <code>v4</code> (AVX-512).</li>
        <li><strong>arm64 (AArch64)</strong>: AWS Graviton, Raspberry Pi 4/5, Apple Silicon Linux VMs.</li>
        <li><strong>riscv64 (RV64GC)</strong>: 64-bit RISC-V edge and server platforms.</li>
      </ul>
    </div>

    <footer>
      <p>Maintained by <a href="https://nicolas.brousse.info/">Nicolas Brousse</a> &bull; <a href="https://github.com/orieg/expanse">GitHub Repository</a> &bull; <a href="https://crates.io/crates/expanse-trie">Crates.io</a></p>
    </footer>
  </div>
</body>
</html>
"""
    with open(os.path.join(output_dir, "index.html"), "w", encoding="utf-8") as f:
        f.write(html_content)

    print(f"APT repository and index.html successfully generated in {output_dir}")


if __name__ == "__main__":
    in_dir = sys.argv[1] if len(sys.argv) > 1 else "artifacts"
    out_dir = sys.argv[2] if len(sys.argv) > 2 else "apt"
    build_apt_repo(in_dir, out_dir)
