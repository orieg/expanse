#!/usr/bin/env python3
"""Builds an RPM repository with repodata metadata, expanse.repo, and a styled index.html."""

import datetime
import gzip
import hashlib
import os
import shutil
import subprocess
import sys


def sha256_file(filepath: str) -> str:
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def build_rpm_repo(input_dir: str, output_dir: str):
    os.makedirs(output_dir, exist_ok=True)
    repodata_dir = os.path.join(output_dir, "repodata")
    os.makedirs(repodata_dir, exist_ok=True)
    packages_dir = os.path.join(output_dir, "Packages")
    os.makedirs(packages_dir, exist_ok=True)

    rpm_files = []
    if os.path.isdir(input_dir):
        for root, _, files in os.walk(input_dir):
            for file in files:
                if file.endswith(".rpm"):
                    rpm_files.append(os.path.join(root, file))

    arch_packages = {"x86_64": [], "aarch64": [], "riscv64": [], "noarch": []}

    for rpm_path in rpm_files:
        filename = os.path.basename(rpm_path)
        arch = "x86_64"
        if "aarch64" in filename or "arm64" in filename:
            arch = "aarch64"
        elif "riscv64" in filename:
            arch = "riscv64"
        elif "noarch" in filename:
            arch = "noarch"

        target_arch_dir = os.path.join(packages_dir, arch)
        os.makedirs(target_arch_dir, exist_ok=True)
        dest_path = os.path.join(target_arch_dir, filename)
        shutil.copy2(rpm_path, dest_path)

        size = os.path.getsize(dest_path)
        sha256 = sha256_file(dest_path)
        pkg_name = filename.split("-")[0]
        version = "0.2.0"

        entry = {
            "name": pkg_name,
            "arch": arch,
            "version": version,
            "release": "1",
            "filename": f"Packages/{arch}/{filename}",
            "size": size,
            "sha256": sha256,
        }
        arch_packages[arch].append(entry)

    has_createrepo = shutil.which("createrepo_c") or shutil.which("createrepo")
    if has_createrepo:
        subprocess.run([has_createrepo, "--update", output_dir], check=True)
    else:
        now_ts = int(datetime.datetime.now(datetime.timezone.utc).timestamp())
        primary_xml = '<?xml version="1.0" encoding="UTF-8"?>\n<metadata xmlns="http://linux.duke.edu/metadata/common" packages="{}">\n'.format(
            sum(len(v) for v in arch_packages.values())
        )
        for arch, pkgs in arch_packages.items():
            for p in pkgs:
                primary_xml += f"""  <package type="rpm">
    <name>{p['name']}</name>
    <arch>{p['arch']}</arch>
    <version epoch="0" ver="{p['version']}" rel="{p['release']}"/>
    <checksum type="sha256" pkgid="YES">{p['sha256']}</checksum>
    <summary>Expanse: modern Judy arrays and high-performance digital tree engine</summary>
    <description>Clean-room pure-Rust Judy arrays modernized for 64-bit microarchitectures.</description>
    <packager>Nicolas Brousse &lt;nicolas@brousse.info&gt;</packager>
    <url>https://github.com/orieg/expanse</url>
    <time file="{now_ts}" build="{now_ts}"/>
    <size package="{p['size']}" installed="{p['size']}" archive="{p['size']}"/>
    <location href="{p['filename']}"/>
    <format>
      <provides>
        <entry name="{p['name']}" flags="EQ" epoch="0" ver="{p['version']}" rel="{p['release']}"/>
      </provides>
    </format>
  </package>\n"""
        primary_xml += "</metadata>\n"

        primary_path = os.path.join(repodata_dir, "primary.xml")
        primary_gz_path = os.path.join(repodata_dir, "primary.xml.gz")
        with open(primary_path, "w", encoding="utf-8") as f:
            f.write(primary_xml)
        with open(primary_path, "rb") as f_in, gzip.open(
            primary_gz_path, "wb"
        ) as f_out:
            shutil.copyfileobj(f_in, f_out)
        os.remove(primary_path)

        filelists_xml = '<?xml version="1.0" encoding="UTF-8"?>\n<filelists xmlns="http://linux.duke.edu/metadata/filelists" packages="{}">\n</filelists>\n'.format(
            sum(len(v) for v in arch_packages.values())
        )
        filelists_gz_path = os.path.join(repodata_dir, "filelists.xml.gz")
        with gzip.open(filelists_gz_path, "wt", encoding="utf-8") as f_out:
            f_out.write(filelists_xml)

        other_xml = '<?xml version="1.0" encoding="UTF-8"?>\n<otherdata xmlns="http://linux.duke.edu/metadata/other" packages="{}">\n</otherdata>\n'.format(
            sum(len(v) for v in arch_packages.values())
        )
        other_gz_path = os.path.join(repodata_dir, "other.xml.gz")
        with gzip.open(other_gz_path, "wt", encoding="utf-8") as f_out:
            f_out.write(other_xml)

        primary_size = os.path.getsize(primary_gz_path)
        primary_sha256 = sha256_file(primary_gz_path)
        filelists_size = os.path.getsize(filelists_gz_path)
        filelists_sha256 = sha256_file(filelists_gz_path)
        other_size = os.path.getsize(other_gz_path)
        other_sha256 = sha256_file(other_gz_path)

        repomd_xml = f"""<?xml version="1.0" encoding="UTF-8"?>
<repomd xmlns="http://linux.duke.edu/metadata/repo">
  <revision>{now_ts}</revision>
  <data type="primary">
    <checksum type="sha256">{primary_sha256}</checksum>
    <location href="repodata/primary.xml.gz"/>
    <timestamp>{now_ts}</timestamp>
    <size>{primary_size}</size>
  </data>
  <data type="filelists">
    <checksum type="sha256">{filelists_sha256}</checksum>
    <location href="repodata/filelists.xml.gz"/>
    <timestamp>{now_ts}</timestamp>
    <size>{filelists_size}</size>
  </data>
  <data type="other">
    <checksum type="sha256">{other_sha256}</checksum>
    <location href="repodata/other.xml.gz"/>
    <timestamp>{now_ts}</timestamp>
    <size>{other_size}</size>
  </data>
</repomd>
"""
        with open(os.path.join(repodata_dir, "repomd.xml"), "w", encoding="utf-8") as f:
            f.write(repomd_xml)

    repo_file_content = """[expanse]
name=Expanse Enterprise Linux Repository
baseurl=https://orieg.github.io/expanse/rpm/
enabled=1
gpgcheck=0
repo_gpgcheck=0
"""
    with open(os.path.join(output_dir, "expanse.repo"), "w", encoding="utf-8") as f:
        f.write(repo_file_content)

    html_content = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Expanse — Official RPM Repository</title>
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
      -webkit-backdrop-filter: blur(12px);
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
      <li><a href="../apt/">APT (Debian)</a></li>
      <li><a href="./" class="active">RPM (RHEL)</a></li>
      <li><a href="https://github.com/orieg/expanse/blob/main/docs/ARCHITECTURE.md">Docs</a></li>
      <li><a href="https://github.com/orieg/expanse" class="nav-pill">GitHub &bull; 0.2.0</a></li>
    </ul>
  </header>

  <div class="container">
    <h1>Expanse RPM Repository <span class="badge">v0.2.0</span></h1>
    <p>Official YUM / DNF repository for <strong><a href="https://github.com/orieg/expanse">Expanse</a></strong> across Enterprise Linux: RHEL 8/9/10, CentOS Stream, Fedora, Rocky Linux, AlmaLinux, and Amazon Linux 2023.</p>

    <div class="card">
      <h2>Quick Setup</h2>
      <p>Configure the repository using DNF / YUM:</p>
      <pre><code># 1. Add repository configuration
sudo dnf config-manager --add-repo https://orieg.github.io/expanse/rpm/expanse.repo

# Or manually download repo file for older YUM / Amazon Linux:
# sudo curl -sS -o /etc/yum.repos.d/expanse.repo https://orieg.github.io/expanse/rpm/expanse.repo

# 2. Install runtime library, development headers, and libjudy compat symlinks
sudo dnf install -y libexpanse libexpanse-devel libjudy-compat</code></pre>
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
            fname = pkg["filename"]
            pname = pkg["name"]
            html_content += f"""          <tr>
            <td><strong>{pname}</strong></td>
            <td><code>{arch}</code></td>
            <td><a href="{fname}">{os.path.basename(fname)}</a></td>
          </tr>\n"""

    if not has_pkgs:
        html_content += """          <tr>
            <td><strong>libexpanse</strong></td>
            <td><code>x86_64</code></td>
            <td><a href="Packages/x86_64/libexpanse-0.2.0-1.x86_64.rpm">libexpanse-0.2.0-1.x86_64.rpm</a></td>
          </tr>
          <tr>
            <td><strong>libexpanse-devel</strong></td>
            <td><code>x86_64</code></td>
            <td><a href="Packages/x86_64/libexpanse-devel-0.2.0-1.x86_64.rpm">libexpanse-devel-0.2.0-1.x86_64.rpm</a></td>
          </tr>
          <tr>
            <td><strong>libjudy-compat</strong></td>
            <td><code>x86_64</code></td>
            <td><a href="Packages/x86_64/libjudy-compat-0.2.0-1.x86_64.rpm">libjudy-compat-0.2.0-1.x86_64.rpm</a></td>
          </tr>
"""

    html_content += """        </tbody>
      </table>
    </div>

    <div class="card">
      <h2>Supported Platforms &amp; Architectures</h2>
      <ul style="margin-left: 1.5rem; margin-top: 0.5rem; line-height: 1.8;">
        <li><strong>x86_64</strong>: Baseline <code>x86-64-v1</code> with <code>glibc-hwcaps</code> dynamic optimization for <code>v2</code>, <code>v3</code> (AVX2), and <code>v4</code> (AVX-512).</li>
        <li><strong>aarch64 (ARM64)</strong>: AWS Graviton, Ampere Altra, ARM64 servers.</li>
        <li><strong>riscv64 (RV64GC)</strong>: 64-bit RISC-V edge and server platforms.</li>
      </ul>
    </div>

    <div class="card">
      <h2>Repository Metadata Files</h2>
      <p>Direct links to repository configuration and repomd metadata:</p>
      <ul style="margin-left: 1.5rem; margin-top: 0.5rem; line-height: 1.8;">
        <li><a href="expanse.repo"><code>expanse.repo</code></a> &mdash; YUM/DNF repository configuration</li>
        <li><a href="repodata/repomd.xml"><code>repodata/repomd.xml</code></a> &mdash; Repository index metadata</li>
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

    print(f"RPM repository and index.html successfully generated in {output_dir}")


if __name__ == "__main__":
    in_dir = sys.argv[1] if len(sys.argv) > 1 else "artifacts"
    out_dir = sys.argv[2] if len(sys.argv) > 2 else "rpm"
    build_rpm_repo(in_dir, out_dir)
