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

    # Generate styled index.html landing page
    html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Expanse — Official APT Repository</title>
  <style>
    :root {{
      --bg: #0d1117;
      --card: #161b22;
      --border: #30363d;
      --text: #c9d1d9;
      --heading: #f0f6fc;
      --accent: #58a6ff;
      --code-bg: #090d13;
      --green: #3fb950;
    }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
      background: var(--bg);
      color: var(--text);
      margin: 0;
      padding: 2rem 1rem;
      display: flex;
      justify-content: center;
    }}
    .container {{
      max-width: 800px;
      width: 100%;
    }}
    h1, h2, h3 {{
      color: var(--heading);
    }}
    h1 {{
      font-size: 2rem;
      margin-bottom: 0.5rem;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }}
    .badge {{
      display: inline-block;
      padding: 0.25rem 0.5rem;
      font-size: 0.8rem;
      font-weight: 600;
      border-radius: 6px;
      background: rgba(88, 166, 255, 0.15);
      color: var(--accent);
      border: 1px solid rgba(88, 166, 255, 0.3);
    }}
    p {{
      line-height: 1.6;
    }}
    pre {{
      background: var(--code-bg);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 1rem;
      overflow-x: auto;
      color: #79c0ff;
      font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
      font-size: 0.9rem;
    }}
    .card {{
      background: var(--card);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 1.5rem;
      margin: 1.5rem 0;
    }}
    table {{
      width: 100%;
      border-collapse: collapse;
      margin-top: 1rem;
    }}
    th, td {{
      text-align: left;
      padding: 0.75rem;
      border-bottom: 1px solid var(--border);
    }}
    th {{
      color: var(--heading);
      font-size: 0.85rem;
      text-transform: uppercase;
    }}
    a {{
      color: var(--accent);
      text-decoration: none;
    }}
    a:hover {{
      text-decoration: underline;
    }}
    .footer {{
      margin-top: 2rem;
      font-size: 0.85rem;
      color: #8b949e;
      text-align: center;
    }}
  </style>
</head>
<body>
  <div class="container">
    <h1>Expanse APT Repository <span class="badge">v0.2.0</span></h1>
    <p>Official Debian and Ubuntu package repository for <strong><a href="https://github.com/orieg/expanse">Expanse</a></strong> — clean-room, pure-Rust Judy arrays modernized for modern 64-bit microarchitectures.</p>

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
    for arch, pkgs in arch_packages.items():
        for pkg in pkgs:
            fname = pkg["Filename"]
            pname = pkg["Package"]
            html_content += f"""          <tr>
            <td><strong>{pname}</strong></td>
            <td><code>{arch}</code></td>
            <td><a href="{fname}">{os.path.basename(fname)}</a></td>
          </tr>\n"""

    html_content += f"""        </tbody>
      </table>
    </div>

    <div class="card">
      <h2>Supported Architectures</h2>
      <ul>
        <li><strong>amd64 (x86-64)</strong>: baseline <code>x86-64-v1</code>, with <code>glibc-hwcaps</code> optimized variants for <code>v2</code> (POPCNT), <code>v3</code> (AVX2/BMI2), and <code>v4</code> (AVX-512).</li>
        <li><strong>arm64 (AArch64)</strong>: AWS Graviton, Raspberry Pi 4/5, Apple Silicon Linux VMs.</li>
        <li><strong>riscv64 (RV64GC)</strong>: 64-bit RISC-V edge and server platforms.</li>
      </ul>
    </div>

    <div class="footer">
      <p>Maintained by <a href="https://nicolas.brousse.info/">Nicolas Brousse</a> &bull; <a href="https://github.com/orieg/expanse">GitHub Repository</a> &bull; <a href="https://crates.io/crates/expanse-trie">Crates.io</a></p>
    </div>
  </div>
</body>
</html>
"""
    with open(os.path.join(output_dir, "index.html"), "w", encoding="utf-8") as f:
        f.write(html_content)

    apt_sub = os.path.join(output_dir, "apt")
    if os.path.isdir(apt_sub):
        with open(os.path.join(apt_sub, "index.html"), "w", encoding="utf-8") as f:
            f.write(html_content)

    print(f"APT repository and index.html successfully generated in {output_dir}")


if __name__ == "__main__":
    in_dir = sys.argv[1] if len(sys.argv) > 1 else "artifacts"
    out_dir = sys.argv[2] if len(sys.argv) > 2 else "apt"
    build_apt_repo(in_dir, out_dir)

