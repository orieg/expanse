#!/usr/bin/env python3
"""Builds the complete GitHub Pages web distribution with portal index, visualizer, APT, and RPM repositories."""

import datetime
import os
import shutil
import subprocess
import sys
from build_apt_repo import build_apt_repo
from build_rpm_repo import build_rpm_repo


def get_workspace_version(repo_root: str) -> str:
    """Inspects workspace and crate Cargo.toml files to extract the package version."""
    for toml_path in [
        os.path.join(repo_root, "Cargo.toml"),
        os.path.join(repo_root, "crates", "expanse", "Cargo.toml"),
        os.path.join(repo_root, "crates", "expanse-capi", "Cargo.toml"),
    ]:
        if os.path.isfile(toml_path):
            with open(toml_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line.startswith("version = ") or line.startswith("version="):
                        v = line.split("=", 1)[1].strip().strip('"\'')
                        if v and not v.endswith(".workspace"):
                            return v
    return "0.3.0"


def get_git_metadata(repo_root: str, default_version: str) -> dict:
    """Extracts latest release tag, short commit SHA, and commit timestamp via git."""
    tag = None
    try:
        res = subprocess.run(
            ["git", "describe", "--tags", "--abbrev=0"],
            cwd=repo_root,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            check=False,
        )
        if res.returncode == 0 and res.stdout.strip():
            tag = res.stdout.strip()
    except Exception:
        pass
    if not tag:
        tag = f"v{default_version}"

    commit_sha = ""
    try:
        res = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=repo_root,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            check=False,
        )
        if res.returncode == 0 and res.stdout.strip():
            commit_sha = res.stdout.strip()
    except Exception:
        pass
    if not commit_sha:
        commit_sha = "unknown"

    build_date = ""
    try:
        res = subprocess.run(
            ["git", "log", "-1", "--format=%cs"],
            cwd=repo_root,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            check=False,
        )
        if res.returncode == 0 and res.stdout.strip():
            build_date = res.stdout.strip()
    except Exception:
        pass
    if not build_date:
        build_date = datetime.date.today().isoformat()

    return {
        "tag": tag,
        "commit_sha": commit_sha,
        "build_date": build_date,
    }


MAIN_CSS = """
    :root {
      --bg: #090d16;
      --card-bg: #111827;
      --card-inner: #0b1120;
      --border: #1f293d;
      --border-accent: rgba(56, 189, 248, 0.4);
      --text: #e2e8f0;
      --text-muted: #94a3b8;
      --heading: #f8fafc;
      --accent: #38bdf8;
      --accent-hover: #7dd3fc;
      --accent-green: #10b981;
      --code-bg: #030712;
      --bench-bg: #0d1117;
      --quote-bg: linear-gradient(180deg, rgba(30, 41, 59, 0.5) 0%, rgba(15, 23, 42, 0.8) 100%);
      --navbar-bg: rgba(9, 13, 22, 0.85);
      --nav-pill-bg: rgba(56, 189, 248, 0.1);
      --nav-pill-border: rgba(56, 189, 248, 0.25);
      --btn-secondary-bg: #111827;
      --btn-secondary-hover: #1e293b;
      --btn-secondary-border: #1f293d;
      --btn-secondary-text: #f8fafc;
      --badge-bg: #111827;
      --badge-border: #1f293d;
      --badge-text: #38bdf8;
      --spotlight-bg: linear-gradient(135deg, #0f172a 0%, #1e1b4b 50%, #0f172a 100%);
      --spotlight-border: rgba(99, 102, 241, 0.4);
      --tab-active-bg: rgba(56, 189, 248, 0.05);
      --table-header-color: #f8fafc;
      --table-row-border: #1f293d;
      color-scheme: dark;
    }

    [data-theme="light"] {
      --bg: #f8fafc;
      --card-bg: #ffffff;
      --card-inner: #f1f5f9;
      --border: #e2e8f0;
      --border-accent: rgba(2, 132, 199, 0.4);
      --text: #334155;
      --text-muted: #64748b;
      --heading: #0f172a;
      --accent: #0284c7;
      --accent-hover: #0369a1;
      --accent-green: #059669;
      --code-bg: #0f172a;
      --bench-bg: #ffffff;
      --quote-bg: linear-gradient(180deg, rgba(241, 245, 249, 0.9) 0%, rgba(226, 232, 240, 0.7) 100%);
      --navbar-bg: rgba(248, 250, 252, 0.88);
      --nav-pill-bg: rgba(2, 132, 199, 0.08);
      --nav-pill-border: rgba(2, 132, 199, 0.25);
      --btn-secondary-bg: #ffffff;
      --btn-secondary-hover: #f1f5f9;
      --btn-secondary-border: #cbd5e1;
      --btn-secondary-text: #0f172a;
      --badge-bg: #ffffff;
      --badge-border: #e2e8f0;
      --badge-text: #0284c7;
      --spotlight-bg: linear-gradient(135deg, #f0f9ff 0%, #e0e7ff 50%, #f0fdf4 100%);
      --spotlight-border: rgba(99, 102, 241, 0.3);
      --tab-active-bg: rgba(2, 132, 199, 0.08);
      --table-header-color: #0f172a;
      --table-row-border: #e2e8f0;
      color-scheme: light;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.6;
      overflow-x: hidden;
      -webkit-font-smoothing: antialiased;
    }
    .navbar {
      position: sticky;
      top: 0;
      z-index: 100;
      background: var(--navbar-bg);
      backdrop-filter: blur(12px);
      -webkit-backdrop-filter: blur(12px);
      border-bottom: 1px solid var(--border);
      padding: 0.75rem 2rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 1.5rem;
    }
    .nav-top {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.75rem;
    }
    .nav-brand {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-weight: 700;
      font-size: 1.2rem;
      color: var(--heading);
      text-decoration: none;
      flex-shrink: 0;
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
    .nav-scroll {
      display: flex;
      align-items: center;
      gap: 1.25rem;
    }
    .nav-links {
      display: flex;
      gap: 1.25rem;
      align-items: center;
      list-style: none;
      margin: 0;
      padding: 0;
    }
    .nav-links a {
      color: var(--text-muted);
      text-decoration: none;
      font-size: 0.9rem;
      font-weight: 500;
      transition: color 0.15s ease;
      white-space: nowrap;
    }
    .nav-links a:hover, .nav-links a.active { color: var(--accent); }
    .nav-pill {
      padding: 0.35rem 0.75rem;
      background: var(--nav-pill-bg);
      border: 1px solid var(--nav-pill-border);
      border-radius: 6px;
      color: var(--accent) !important;
      font-weight: 600 !important;
    }
    .theme-toggle {
      background: var(--card-inner);
      border: 1px solid var(--border);
      border-radius: 6px;
      width: 34px;
      height: 34px;
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      color: var(--heading);
      font-size: 16px;
      flex-shrink: 0;
      transition: all 0.15s ease;
    }
    .theme-toggle:hover { border-color: var(--accent); }
    [data-theme="dark"] .theme-icon-sun { display: inline; }
    [data-theme="dark"] .theme-icon-moon { display: none; }
    [data-theme="light"] .theme-icon-sun { display: none; }
    [data-theme="light"] .theme-icon-moon { display: inline; }
    :root:not([data-theme]) .theme-icon-sun { display: inline; }
    :root:not([data-theme]) .theme-icon-moon { display: none; }

    .theme-toggle-mobile { display: flex; }
    .theme-toggle-desktop { display: none; }

    @media (min-width: 769px) {
      .theme-toggle-mobile { display: none !important; }
      .theme-toggle-desktop { display: flex !important; }
    }

    .container {
      max-width: 1140px;
      margin: 0 auto;
      padding: 0 1.5rem;
    }
    .hero {
      padding: 5rem 0 3.5rem;
      text-align: center;
    }
    .badge-bar {
      display: flex;
      justify-content: center;
      gap: 0.5rem;
      flex-wrap: wrap;
      margin-bottom: 1.5rem;
      max-width: 100%;
    }
    .badge {
      display: inline-flex;
      align-items: center;
      gap: 0.4rem;
      padding: 0.3rem 0.75rem;
      font-size: 0.8rem;
      font-weight: 600;
      border-radius: 20px;
      background: var(--badge-bg);
      border: 1px solid var(--badge-border);
      color: var(--badge-text);
    }
    .badge-green {
      color: var(--accent-green);
      border-color: rgba(16, 185, 129, 0.3);
      background: rgba(16, 185, 129, 0.08);
    }
    .hero h1 {
      font-size: clamp(2rem, 5vw, 3.25rem);
      font-weight: 800;
      letter-spacing: -0.03em;
      color: var(--heading);
      line-height: 1.15;
      margin-bottom: 1.25rem;
      overflow-wrap: break-word;
      word-break: normal;
    }
    .hero p {
      font-size: 1.15rem;
      color: var(--text-muted);
      max-width: 780px;
      margin: 0 auto 2.5rem;
      line-height: 1.6;
    }
    .hero-actions {
      display: flex;
      justify-content: center;
      gap: 1rem;
      flex-wrap: wrap;
    }
    .btn {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 0.5rem;
      padding: 0.75rem 1.5rem;
      font-size: 0.95rem;
      font-weight: 600;
      border-radius: 8px;
      text-decoration: none;
      transition: all 0.15s ease;
      cursor: pointer;
    }
    .btn-primary {
      background: var(--accent);
      color: #090d16;
    }
    .btn-primary:hover {
      background: var(--accent-hover);
      box-shadow: 0 0 20px rgba(56, 189, 248, 0.4);
    }
    .btn-secondary {
      background: var(--btn-secondary-bg);
      color: var(--btn-secondary-text);
      border: 1px solid var(--btn-secondary-border);
    }
    .btn-secondary:hover {
      background: var(--btn-secondary-hover);
      border-color: var(--accent);
    }
    section {
      padding: 3.5rem 0;
      border-top: 1px solid var(--border);
    }
    .section-header {
      text-align: center;
      margin-bottom: 2.5rem;
    }
    .section-tag {
      text-transform: uppercase;
      font-size: 0.8rem;
      letter-spacing: 0.1em;
      font-weight: 700;
      color: var(--accent);
      margin-bottom: 0.5rem;
      display: block;
    }
    .section-title {
      font-size: clamp(1.6rem, 3.5vw, 2rem);
      font-weight: 700;
      color: var(--heading);
      letter-spacing: -0.02em;
    }
    .section-desc {
      color: var(--text-muted);
      max-width: 650px;
      margin: 0.5rem auto 0;
      font-size: 1rem;
    }
    .quote-box {
      background: var(--quote-bg);
      border: 1px solid var(--border);
      border-left: 4px solid var(--accent);
      border-radius: 8px;
      padding: 1.75rem 2rem;
      margin: 1.5rem 0;
    }
    .quote-text {
      font-size: 1.05rem;
      font-style: italic;
      color: var(--text);
      margin-bottom: 0.75rem;
      line-height: 1.6;
    }
    .quote-author {
      font-size: 0.85rem;
      font-weight: 600;
      color: var(--accent);
      text-align: right;
    }
    .grid-3 {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
      gap: 1.5rem;
    }
    .card {
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 1.75rem;
      transition: transform 0.2s ease, border-color 0.2s ease;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
    }
    .card:hover {
      transform: translateY(-2px);
      border-color: var(--border-accent);
    }
    .card-icon {
      width: 42px;
      height: 42px;
      border-radius: 8px;
      background: var(--nav-pill-bg);
      border: 1px solid var(--nav-pill-border);
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 1.25rem;
      margin-bottom: 1.25rem;
      color: var(--accent);
    }
    .card-title {
      font-size: 1.2rem;
      font-weight: 700;
      color: var(--heading);
      margin-bottom: 0.5rem;
    }
    .card-p {
      color: var(--text-muted);
      font-size: 0.95rem;
      line-height: 1.6;
    }
    .spotlight {
      background: var(--spotlight-bg);
      border: 1px solid var(--spotlight-border);
      border-radius: 12px;
      padding: 2.5rem;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 2rem;
      margin: 2rem 0;
      box-shadow: 0 10px 30px -10px rgba(99, 102, 241, 0.2);
    }
    .spotlight-content h3 {
      font-size: 1.6rem;
      color: var(--heading);
      margin-bottom: 0.5rem;
    }
    .spotlight-content p {
      color: var(--text-muted);
      font-size: 1rem;
      max-width: 600px;
    }
    .bench-container {
      margin: 2rem 0;
      display: flex;
      flex-direction: column;
      gap: 2rem;
    }
    .bench-wrapper {
      background: var(--bench-bg);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 1.25rem;
      overflow-x: auto;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
    }
    .bench-wrapper svg {
      width: 100%;
      height: auto;
      display: block;
      min-width: 540px;
    }
    /* SVG Chart Theme Recolor Rules */
    [data-theme="light"] .bench-wrapper svg .bg { fill: #ffffff !important; }
    [data-theme="light"] .bench-wrapper svg .border { stroke: #d0d7de !important; }
    [data-theme="light"] .bench-wrapper svg .grid { stroke: #eaeef2 !important; }
    [data-theme="light"] .bench-wrapper svg .axis { stroke: #afb8c1 !important; }
    [data-theme="light"] .bench-wrapper svg .divider { stroke: #eaeef2 !important; }
    [data-theme="light"] .bench-wrapper svg .t-chart-title,
    [data-theme="light"] .bench-wrapper svg .t-title { fill: #57606a !important; }
    [data-theme="light"] .bench-wrapper svg .t-chart-sub,
    [data-theme="light"] .bench-wrapper svg .t-sub { fill: #8c959f !important; }
    [data-theme="light"] .bench-wrapper svg .t-unit-header,
    [data-theme="light"] .bench-wrapper svg .t-unit { fill: #57606a !important; }
    [data-theme="light"] .bench-wrapper svg .t-axis-label,
    [data-theme="light"] .bench-wrapper svg .t-tick { fill: #57606a !important; }
    [data-theme="light"] .bench-wrapper svg .t-bar-label,
    [data-theme="light"] .bench-wrapper svg .t-legend { fill: #1f2328 !important; }
    [data-theme="light"] .bench-wrapper svg .t-val-accent,
    [data-theme="light"] .bench-wrapper svg .t-tag { fill: #1a7f37 !important; }
    [data-theme="light"] .bench-wrapper svg .t-val-blue { fill: #0969da !important; }
    [data-theme="light"] .bench-wrapper svg .t-val-muted { fill: #656d76 !important; }
    [data-theme="light"] .bench-wrapper svg .b-expanse { fill: #1a7f37 !important; }
    [data-theme="light"] .bench-wrapper svg .b-roaring,
    [data-theme="light"] .bench-wrapper svg .b-btreemap { fill: #0969da !important; }
    [data-theme="light"] .bench-wrapper svg .b-other,
    [data-theme="light"] .bench-wrapper svg .b-skipmap { fill: #d0d7de !important; }
    [data-theme="light"] .bench-wrapper svg .line-occ { stroke: #1a7f37 !important; }
    [data-theme="light"] .bench-wrapper svg .dot-occ { fill: #1a7f37 !important; stroke: #ffffff !important; }
    [data-theme="light"] .bench-wrapper svg .line-linear { stroke: #8c959f !important; }

    [data-theme="dark"] .bench-wrapper svg .bg { fill: #0d1117; }
    [data-theme="dark"] .bench-wrapper svg .border { stroke: #30363d; }
    [data-theme="dark"] .bench-wrapper svg .grid { stroke: #21262d; }
    [data-theme="dark"] .bench-wrapper svg .axis { stroke: #484f58; }
    [data-theme="dark"] .bench-wrapper svg .divider { stroke: #21262d; }
    [data-theme="dark"] .bench-wrapper svg .t-chart-title,
    [data-theme="dark"] .bench-wrapper svg .t-title { fill: #8b949e; }
    [data-theme="dark"] .bench-wrapper svg .t-chart-sub,
    [data-theme="dark"] .bench-wrapper svg .t-sub { fill: #6e7681; }
    [data-theme="dark"] .bench-wrapper svg .t-unit-header,
    [data-theme="dark"] .bench-wrapper svg .t-unit { fill: #8b949e; }
    [data-theme="dark"] .bench-wrapper svg .t-axis-label,
    [data-theme="dark"] .bench-wrapper svg .t-tick { fill: #8b949e; }
    [data-theme="dark"] .bench-wrapper svg .t-bar-label,
    [data-theme="dark"] .bench-wrapper svg .t-legend { fill: #c9d1d9; }
    [data-theme="dark"] .bench-wrapper svg .t-val-accent,
    [data-theme="dark"] .bench-wrapper svg .t-tag { fill: #3fb950; }
    [data-theme="dark"] .bench-wrapper svg .t-val-blue { fill: #58a6ff; }
    [data-theme="dark"] .bench-wrapper svg .t-val-muted { fill: #8b949e; }
    [data-theme="dark"] .bench-wrapper svg .b-expanse { fill: #2ea043; }
    [data-theme="dark"] .bench-wrapper svg .b-roaring,
    [data-theme="dark"] .bench-wrapper svg .b-btreemap { fill: #1f6feb; }
    [data-theme="dark"] .bench-wrapper svg .b-other,
    [data-theme="dark"] .bench-wrapper svg .b-skipmap { fill: #30363d; }
    [data-theme="dark"] .bench-wrapper svg .line-occ { stroke: #2ea043; }
    [data-theme="dark"] .bench-wrapper svg .dot-occ { fill: #2ea043; stroke: #0d1117; }
    [data-theme="dark"] .bench-wrapper svg .line-linear { stroke: #6e7681; }
    .install-box {
      background: var(--card-inner);
      border: 1px solid var(--border);
      border-radius: 10px;
      overflow: hidden;
      margin-top: 1.5rem;
    }
    .install-nav {
      display: flex;
      background: var(--card-bg);
      border-bottom: 1px solid var(--border);
      overflow-x: auto;
      -webkit-overflow-scrolling: touch;
    }
    .tab-btn {
      padding: 0.85rem 1.5rem;
      background: none;
      border: none;
      color: var(--text-muted);
      font-size: 0.9rem;
      font-weight: 600;
      cursor: pointer;
      border-bottom: 2px solid transparent;
      white-space: nowrap;
      transition: all 0.15s ease;
    }
    .tab-btn.active {
      color: var(--accent);
      border-bottom-color: var(--accent);
      background: var(--tab-active-bg);
    }
    .install-panel { padding: 1.5rem; }
    pre {
      background: var(--code-bg);
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 1.25rem;
      overflow-x: auto;
      color: #7dd3fc;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 0.9rem;
      line-height: 1.6;
      max-width: 100%;
    }
    .docs-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: 1rem;
      margin-top: 1.5rem;
    }
    .doc-link-card {
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 1.25rem;
      text-decoration: none;
      transition: all 0.15s ease;
      display: block;
      box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
    }
    .doc-link-card:hover {
      border-color: var(--accent);
      transform: translateY(-2px);
    }
    .doc-link-title {
      font-size: 1rem;
      font-weight: 700;
      color: var(--heading);
      margin-bottom: 0.25rem;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }
    .doc-link-desc {
      font-size: 0.85rem;
      color: var(--text-muted);
    }
    footer {
      border-top: 1px solid var(--border);
      padding: 3rem 0;
      text-align: center;
      color: var(--text-muted);
      font-size: 0.9rem;
    }
    footer a { color: var(--accent); text-decoration: none; }
    footer a:hover { text-decoration: underline; }

    @media (max-width: 768px) {
      .navbar {
        padding: 0.6rem 1rem;
        flex-direction: column;
        align-items: stretch;
        gap: 0.5rem;
      }
      .nav-top {
        width: 100%;
      }
      .nav-scroll {
        width: 100%;
        overflow-x: auto;
        -webkit-overflow-scrolling: touch;
        padding-bottom: 0.2rem;
        justify-content: flex-start;
      }
      .nav-links {
        gap: 0.85rem;
      }
      .theme-toggle-mobile { display: flex !important; }
      .theme-toggle-desktop { display: none !important; }
      .hero { padding: 3rem 0 2rem; }
      .hero h1 {
        font-size: 1.35rem !important; line-height: 1.3 !important; hyphens: none !important; -webkit-hyphens: none !important;
        line-height: 1.25 !important;
        padding: 0 0.25rem;
      }
      .hero p { font-size: 1rem !important; margin-bottom: 1.75rem !important; }
      .hero-actions { flex-direction: column; width: 100%; }
      .hero-actions .btn { width: 100%; }
      .badge-bar { gap: 0.35rem !important; }
      .badge { font-size: 0.72rem !important; padding: 0.25rem 0.55rem !important; }
      .spotlight { flex-direction: column; text-align: center; padding: 1.5rem; }
      .spotlight-content p { font-size: 0.95rem; }
      .quote-box { padding: 1.25rem; }
      .quote-text { font-size: 0.95rem !important; }
      .card { padding: 1.25rem; }
      .install-panel { padding: 1rem; }
    }
"""

NAV_HTML = """  <header class="navbar">
    <div class="nav-top">
      <a href="./" class="nav-brand">
        <div class="nav-logo">E</div>
        <span>Expanse</span>
      </a>
      <button class="theme-toggle theme-toggle-mobile" onclick="toggleTheme()" aria-label="Toggle theme" title="Toggle theme">
        <span class="theme-icon-sun">&#9728;</span>
        <span class="theme-icon-moon">&#9790;</span>
      </button>
    </div>
    <div class="nav-scroll">
      <ul class="nav-links">
        <li><a href="./" class="active">Home</a></li>
        <li><a href="./#benchmarks">Benchmarks</a></li>
        <li><a href="./visualizer.html" class="">Visualizer</a></li>
        <li><a href="./apt/" class="">APT (Debian)</a></li>
        <li><a href="./rpm/" class="">RPM (RHEL)</a></li>
        <li><a href="https://github.com/orieg/expanse/blob/main/docs/ARCHITECTURE.md">Docs</a></li>
        <li><a href="https://github.com/orieg/expanse" class="nav-pill">GitHub &bull; 0.3.0</a></li>
      </ul>
      <button class="theme-toggle theme-toggle-desktop" onclick="toggleTheme()" aria-label="Toggle theme" title="Toggle theme">
        <span class="theme-icon-sun">&#9728;</span>
        <span class="theme-icon-moon">&#9790;</span>
      </button>
    </div>
  </header>"""

NAV_VIS_HTML = """  <header class="navbar">
    <div class="nav-top">
      <a href="./" class="nav-brand">
        <div class="nav-logo">E</div>
        <span>Expanse</span>
      </a>
      <button class="theme-toggle theme-toggle-mobile" onclick="toggleTheme()" aria-label="Toggle theme" title="Toggle theme">
        <span class="theme-icon-sun">&#9728;</span>
        <span class="theme-icon-moon">&#9790;</span>
      </button>
    </div>
    <div class="nav-scroll">
      <ul class="nav-links">
        <li><a href="./" class="">Home</a></li>
        <li><a href="./#benchmarks">Benchmarks</a></li>
        <li><a href="./visualizer.html" class="active">Visualizer</a></li>
        <li><a href="./apt/" class="">APT (Debian)</a></li>
        <li><a href="./rpm/" class="">RPM (RHEL)</a></li>
        <li><a href="https://github.com/orieg/expanse/blob/main/docs/ARCHITECTURE.md">Docs</a></li>
        <li><a href="https://github.com/orieg/expanse" class="nav-pill">GitHub &bull; 0.3.0</a></li>
      </ul>
      <button class="theme-toggle theme-toggle-desktop" onclick="toggleTheme()" aria-label="Toggle theme" title="Toggle theme">
        <span class="theme-icon-sun">&#9728;</span>
        <span class="theme-icon-moon">&#9790;</span>
      </button>
    </div>
  </header>"""

def build_pages(artifacts_dir: str, output_dir: str):
    os.makedirs(output_dir, exist_ok=True)

    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    version = get_workspace_version(repo_root)
    git_meta = get_git_metadata(repo_root, version)

    nav_html = NAV_HTML.replace("GitHub &bull; 0.3.0", f"GitHub &bull; {version}")
    nav_vis_html = NAV_VIS_HTML.replace("GitHub &bull; 0.3.0", f"GitHub &bull; {version}")

    footer_meta = (
        f'      <p style="margin-bottom: 0.75rem;">\n'
        f'        <span class="badge" style="font-size: 0.78rem;">Release {git_meta["tag"]}</span> &bull; '
        f'Commit <a href="https://github.com/orieg/expanse/commit/{git_meta["commit_sha"]}">'
        f'<code>{git_meta["commit_sha"]}</code></a> &bull; Built on {git_meta["build_date"]}\n'
        f'      </p>'
    )

    # 1. Copy documentation assets
    assets_dest = os.path.join(output_dir, "docs", "assets")
    os.makedirs(assets_dest, exist_ok=True)
    repo_assets = os.path.join(os.path.dirname(__file__), "..", "docs", "assets")
    if os.path.isdir(repo_assets):
        for f in os.listdir(repo_assets):
            src = os.path.join(repo_assets, f)
            if os.path.isfile(src):
                shutil.copy2(src, os.path.join(assets_dest, f))

    # Copy visualizer_data.json to output_dir/docs/
    json_src = os.path.join(os.path.dirname(__file__), "..", "docs", "visualizer_data.json")
    if os.path.isfile(json_src):
        shutil.copy2(json_src, os.path.join(output_dir, "docs", "visualizer_data.json"))

    # Read SVGs to inline in the main index
    comp_svg_path = os.path.join(repo_assets, "bench_comparative.svg")
    conc_svg_path = os.path.join(repo_assets, "bench_concurrency.svg")
    ycsb_svg_path = os.path.join(repo_assets, "bench_ycsb.svg")
    rocksdb_svg_path = os.path.join(repo_assets, "bench_rocksdb.svg")

    comp_svg = ""
    conc_svg = ""
    ycsb_svg = ""
    rocksdb_svg = ""
    if os.path.isfile(comp_svg_path):
        with open(comp_svg_path, "r", encoding="utf-8") as f:
            comp_svg = f.read()
    if os.path.isfile(conc_svg_path):
        with open(conc_svg_path, "r", encoding="utf-8") as f:
            conc_svg = f.read()
    if os.path.isfile(ycsb_svg_path):
        with open(ycsb_svg_path, "r", encoding="utf-8") as f:
            ycsb_svg = f.read()
    if os.path.isfile(rocksdb_svg_path):
        with open(rocksdb_svg_path, "r", encoding="utf-8") as f:
            rocksdb_svg = f.read()

    # 2. Main Portal index.html
    main_html_template = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Expanse — Modern Judy Arrays & Digital Tree Engine in Rust</title>
  <meta name="description" content="Clean-room, pure-Rust Judy arrays modernized for 64-bit microarchitectures with zero-allocation immediates, SWAR/SIMD vectorization, and lock-free OCC concurrency.">
  <script>
    (function() {
      const saved = localStorage.getItem('expanse-theme');
      if (saved) {
        document.documentElement.setAttribute('data-theme', saved);
      } else {
        const isDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
        document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
      }

      if (window.matchMedia) {
        const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
        const updateTheme = function(e) {
          if (!localStorage.getItem('expanse-theme')) {
            document.documentElement.setAttribute('data-theme', e.matches ? 'dark' : 'light');
          }
        };
        if (mediaQuery.addEventListener) {
          mediaQuery.addEventListener('change', updateTheme);
        } else if (mediaQuery.addListener) {
          mediaQuery.addListener(updateTheme);
        }
      }
    })();
  </script>
  <style>
""" + MAIN_CSS + """
  </style>
</head>
<body>
""" + nav_html + """

  <div class="container">
    <div class="hero">
      <div class="badge-bar">
        <span class="badge badge-green">Pure Rust &bull; #![no_std]</span>
        <span class="badge">Rust &bull; Python &bull; Node.js &bull; .NET &bull; C++20 &bull; Java &bull; C ABI</span>
        <span class="badge">64-Bit &amp; 32-Bit Embedded</span>
        <span class="badge">RocksDB MemTable Plugin</span>
        <span class="badge">glibc-hwcaps (x86-64-v1..v4)</span>
      </div>
      <h1>Modern Judy Arrays &amp; High-Performance Digital Tree Engine</h1>
      <p>Clean-room pure-Rust implementation of digital trees modernized for modern 64-bit microarchitectures with zero-allocation immediates, SWAR/SIMD vectorization, and lock-free OCC reader concurrency.</p>
      <div class="hero-actions">
        <a href="#benchmarks" class="btn btn-primary" style="background: linear-gradient(135deg, #38bdf8, #2563eb); color: #ffffff;">&#9889; Performance Benchmarks &#8595;</a>
        <a href="./visualizer.html" class="btn btn-secondary">Architecture Visualizer &#8594;</a>
        <a href="#quickstart" class="btn btn-secondary">Quickstart</a>
        <a href="https://github.com/orieg/expanse" class="btn btn-secondary">GitHub</a>
      </div>
    </div>
  </div>

  <section>
    <div class="container">
      <div class="section-header">
        <span class="section-tag">Design Philosophy</span>
        <h2 class="section-title">Why &ldquo;Expanse&rdquo;?</h2>
        <p class="section-desc">Partitioning digital trees by key <em>expanse</em>, rather than population.</p>
      </div>

      <p style="color: var(--text); max-width: 860px; margin: 0 auto 1.5rem; line-height: 1.7;">
        Expanse is the Judy design's own defining term &mdash; so central that the published descriptions stop to define it before anything else, and use it as the precise contrast with population-partitioned trees (B-trees, binary trees):
      </p>

      <div class="quote-box">
        <div class="quote-text">&ldquo;Expanse, population, and density are not commonly used terms in tree search literature, so let's define them here: Expanse is a range of possible keys [&hellip;]&rdquo;</div>
        <div class="quote-author">&mdash; Doug Baskins, <em>A 10-Minute Description of How Judy Arrays Work and Why They Are So Fast</em> (2002)</div>
      </div>

      <div class="quote-box">
        <div class="quote-text">&ldquo;A digital tree divides up the population (index set) uniformly by expanse (dividing and redividing the initial expanse evenly), while other methods, such as b-trees, divide up the population by the distribution of the population itself.&rdquo;</div>
        <div class="quote-author">&mdash; Alan Silverstein, <em>Judy IV Shop Manual</em> (2002), &ldquo;Digital Trees&rdquo;</div>
      </div>

      <p style="color: var(--text-muted); max-width: 860px; margin: 1.5rem auto 0; font-size: 0.95rem; line-height: 1.6;">
        Naming the project after the underlying mechanism honors the algorithm itself without inheriting legacy C codebase baggage. Expanse is developed with <strong>strict clean-room discipline</strong>: zero exposure to LGPL source code, adhering exclusively to published design specifications and black-box differential test suites.
      </p>
    </div>
  </section>

  <section>
    <div class="container">
      <div class="section-header">
        <span class="section-tag">Core Engine</span>
        <h2 class="section-title">Architectural Highlights</h2>
        <p class="section-desc">Engineered for cache-line density, hardware SIMD lanes, and lock-free multi-core throughput.</p>
      </div>

      <div class="grid-3">
        <div class="card">
          <div class="card-icon">&#9889;</div>
          <h3 class="card-title">Zero-Alloc Immediates</h3>
          <p class="card-p">Up to 7 keys in sets and up to 3 key-value pairs in maps are packed directly inside tagged 64-bit edge words, bypassing heap allocation entirely for small collections.</p>
        </div>

        <div class="card">
          <div class="card-icon">&#9881;</div>
          <h3 class="card-title">Adaptive Compression Ladder</h3>
          <p class="card-p">Trie branches dynamically morph between Linear leaves (sorted key arrays), Bitmap leaves (64-bit subexpanse bitboards), and full uncompressed 256-way digital branches.</p>
        </div>

        <div class="card">
          <div class="card-icon">&#128640;</div>
          <h3 class="card-title">Lock-Free OCC Concurrency</h3>
          <p class="card-p"><code>SyncExpanseMap</code> and <code>SyncExpanseSet</code> employ epoch-based optimistic concurrency control (OCC). Readers perform lock-free traversals with zero reader-lock cache-line bouncing.</p>
        </div>

        <div class="card">
          <div class="card-icon">&#127919;</div>
          <h3 class="card-title">SIMD &amp; SWAR Acceleration</h3>
          <p class="card-p">Search kernels utilize vector instructions (AVX2, AVX-512, ARM NEON) with bitwise SWAR fallbacks, scanning linear leaves in single clock cycles.</p>
        </div>

        <div class="card">
          <div class="card-icon">&#128230;</div>
          <h3 class="card-title">glibc-hwcaps Multi-Arch</h3>
          <p class="card-p">Debian and RPM packages provide optimized runtime libraries automatically selected by the dynamic loader for <code>x86-64-v2</code>, <code>v3</code> (AVX2), and <code>v4</code> (AVX-512).</p>
        </div>

        <div class="card">
          <div class="card-icon">&#128279;</div>
          <h3 class="card-title">Drop-in Judy C ABI Parity</h3>
          <p class="card-p">Provides 100% C ABI compatibility with stock <code>libjudy</code> (<code>Judy1</code>, <code>JudyL</code>, <code>JudySL</code>, <code>JudyHS</code>) alongside modern, type-safe <code>expanse_*</code> C interfaces.</p>
        </div>

        <div class="card">
          <div class="card-icon">&#127793;</div>
          <h3 class="card-title">32-Bit Embedded (#![no_std])</h3>
          <p class="card-p">Compact 8-byte <code>Edge32</code> layout saving 50% structural SRAM on ARM Cortex-M and RISC-V RV32 microcontrollers, with 32-byte cache alignment and zero-alloc inlined payloads.</p>
        </div>
      </div>
    </div>
  </section>

  <div class="container">
    <div class="spotlight">
      <div class="spotlight-content">
        <h3>Explore the Interactive Data Visualizer</h3>
        <p>Inspect tagged pointer layouts, simulate dynamic compression transitions across the ladder, and step through branch bitboard operations in real-time.</p>
      </div>
      <a href="./visualizer.html" class="btn btn-primary" style="white-space: nowrap;">Launch Visualizer &#8594;</a>
    </div>
  </div>

  <section id="benchmarks">
    <div class="container">
      <div class="section-header">
        <span class="section-tag">Performance</span>
        <h2 class="section-title">Measured Micro-Benchmarks</h2>
        <p class="section-desc">Deterministic instruction counting and latency benchmarks against industry data structures.</p>
      </div>

      <div class="bench-container">
        <div class="bench-wrapper">
          BENCH_COMP_SVG_PLACEHOLDER
        </div>
        <div class="bench-wrapper">
          BENCH_CONC_SVG_PLACEHOLDER
        </div>
        <div class="bench-wrapper">
          BENCH_YCSB_SVG_PLACEHOLDER
        </div>
      </div>
    </div>
  </section>

  <section id="quickstart">
    <div class="container">
      <div class="section-header">
        <span class="section-tag">Ecosystem &amp; Distribution</span>
        <h2 class="section-title">Installation &amp; Quickstart Hub</h2>
        <p class="section-desc">Zero-cost bindings and native packages across Rust, Python, Node.js/Bun, .NET/C#, C++20, Java, C ABI, RocksDB, Linux APT/RPM repos, and PHP.</p>
      </div>

      <div class="install-box">
        <div class="install-nav">
          <button class="tab-btn active" onclick="switchTab('tab-cargo')">Rust (Cargo)</button>
          <button class="tab-btn" onclick="switchTab('tab-python')">Python (PyPI)</button>
          <button class="tab-btn" onclick="switchTab('tab-node')">Node.js / Bun (npm)</button>
          <button class="tab-btn" onclick="switchTab('tab-dotnet')">.NET / C# (NuGet)</button>
          <button class="tab-btn" onclick="switchTab('tab-cpp')">C++20 (Header-Only)</button>
          <button class="tab-btn" onclick="switchTab('tab-c')">C ABI (expanse.h)</button>
          <button class="tab-btn" onclick="switchTab('tab-apt')">Debian / Ubuntu (APT)</button>
          <button class="tab-btn" onclick="switchTab('tab-rpm')">RHEL / Fedora (RPM)</button>
          <button class="tab-btn" onclick="switchTab('tab-java')">Java / JVM (Maven)</button>
          <button class="tab-btn" onclick="switchTab('tab-rocksdb')">RocksDB MemTable</button>
          <button class="tab-btn" onclick="switchTab('tab-php')">PHP Judy &amp; Cache</button>
        </div>

        <div id="tab-cargo" class="install-panel">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Add core Expanse engine to your <code>Cargo.toml</code>:</p>
          <pre><code>cargo add expanse-trie</code></pre>
          <p style="margin-top: 1rem; margin-bottom: 0.75rem; color: var(--text-muted);">Usage example in Rust (Maps, Sets, Off-Heap Blobs, Lock-Free OCC):</p>
          <pre><code>use expanse_trie::{ExpanseMap, ExpanseSet, ExpanseBlobMap, SyncExpanseMap};

// 64-bit integer map with zero-allocation immediates
let mut map = ExpanseMap::new();
map.insert(42, 100);
assert_eq!(map.get(42), Some(100));

// Variable-length byte blob map with slab arena &amp; hot metadata
let mut blobs = ExpanseBlobMap::new();
blobs.insert(1, b"hello expanse", 0x2A);
assert_eq!(blobs.get(1), Some(&amp;b"hello expanse"[..]));

// Thread-safe lock-free OCC map (zero reader locks)
let sync_map = SyncExpanseMap::new();
sync_map.insert(99, 500);
let reader = sync_map.reader();
assert_eq!(reader.get(99), Some(500));</code></pre>
        </div>

        <div id="tab-python" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Install the official Python extension from PyPI (binary wheels for Linux, macOS, Windows):</p>
          <pre><code>pip install expanse-trie</code></pre>
          <p style="margin-top: 1rem; margin-bottom: 0.75rem; color: var(--text-muted);">Usage example in Python:</p>
          <pre><code>from expanse import ExpanseMap, ExpanseSet, ExpanseStrMap, ExpanseBlobMap

# High-performance integer word map
m = ExpanseMap()
m[42] = 100
assert m[42] == 100

# Lexicographical String Trie (JudySL) with zero-copy prefix range scans
st = ExpanseStrMap()
st["https://api.service.internal/v1/users"] = 1001
assert "https://api.service.internal/v1/users" in st

# Off-heap Blob Map with 32-bit hot metadata &amp; TTL pruning
bm = ExpanseBlobMap()
bm.insert(42, b"raw binary payload data", hot_meta=1750000000)
print(bm.get(42))  # b'raw binary payload data'</code></pre>
        </div>

        <div id="tab-node" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Install native Node.js / Bun / Deno bindings via npm (napi-rs v8+ with zero-copy Buffer views):</p>
          <pre><code>npm install @orieg/expanse
# or: bun add @orieg/expanse / pnpm add @orieg/expanse</code></pre>
          <p style="margin-top: 1rem; margin-bottom: 0.75rem; color: var(--text-muted);">Usage example in TypeScript / JavaScript:</p>
          <pre><code>import { ExpanseMap, ExpanseSet, ExpanseStrMap, ExpanseBlobMap } from '@orieg/expanse';

// Integer map with BigInt key support
const map = new ExpanseMap();
map.set(42n, 100n);
console.log(map.get(42n)); // 100n

// Off-heap blob map with Buffer / Uint8Array slices &amp; hot metadata
const blobs = new ExpanseBlobMap();
blobs.set(1001n, Buffer.from("payload bytes"), 42);
const entry = blobs.getWithMeta(1001n);
console.log(entry?.payload.toString()); // "payload bytes"</code></pre>
        </div>

        <div id="tab-dotnet" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Install the official .NET package from NuGet (multi-targeting .NET 8.0 &amp; 9.0 with SafeHandle zero-GC memory safety):</p>
          <pre><code>dotnet add package Orieg.Expanse</code></pre>
          <p style="margin-top: 1rem; margin-bottom: 0.75rem; color: var(--text-muted);">Usage example in C#:</p>
          <pre><code>using Expanse;

// High-speed integer word map
using var map = new ExpanseMap();
map[42] = 100;
if (map.TryGet(42, out var val)) {
    Console.WriteLine($"Found: {val}");
}

// Off-heap blob map with zero-copy ReadOnlySpan&lt;byte&gt; views
using var blobs = new ExpanseBlobMap();
blobs.Set(1001, "Hello .NET"u8, hotMeta: 1);
if (blobs.TryGet(1001, out var payload, out var meta)) {
    Console.WriteLine($"Payload: {System.Text.Encoding.UTF8.GetString(payload)}");
}</code></pre>
        </div>

        <div id="tab-cpp" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Header-only modern C++20 RAII container wrapper (<code>include/expanse.hpp</code>):</p>
          <pre><code>// main.cpp - Header-only C++20 STL-compatible RAII wrapper
#include &lt;expanse.hpp&gt;
#include &lt;iostream&gt;
#include &lt;span&gt;

int main() {
    // RAII word map with std::forward_iterator compatibility
    expanse::map&lt;uint64_t, uint64_t&gt; map;
    map.insert(42, 100);
    std::cout &lt;&lt; "Key 42 -&gt; " &lt;&lt; *map.get(42) &lt;&lt; "\n";

    // Binary-safe byte map (JudyHS) with std::string_view / std::span
    expanse::bytes_map&lt;uint64_t&gt; bmap;
    bmap.insert("embedded\0binary\0key"sv, 999);

    // Off-heap blob map with zero-copy std::span
    expanse::blob_map blobs;
    std::string data = "cache-line aligned blob";
    blobs.insert(1, std::as_bytes(std::span(data)), 0x1F);
    return 0;
}</code></pre>
          <p style="margin-top: 1rem; margin-bottom: 0.75rem; color: var(--text-muted);">Compile with any C++20 compiler:</p>
          <pre><code>clang++ -std=c++20 -I/usr/include main.cpp -lexpanse -o main</code></pre>
        </div>

        <div id="tab-c" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Compile with modern <code>expanse.h</code> or drop-in <code>Judy.h</code> via pkg-config:</p>
          <pre><code>// main.c - Using modern Expanse C API
#include &lt;expanse.h&gt;
#include &lt;stdio.h&gt;

int main() {
    expanse_map_t map = NULL;
    expanse_map_insert(&amp;map, 100, 500);
    
    uint64_t val = 0;
    if (expanse_map_get(map, 100, &amp;val)) {
        printf("Found key 100 -&gt; %llu\n", (unsigned long long)val);
    }
    expanse_map_free(&amp;map);
    return 0;
}</code></pre>
          <p style="margin-top: 1rem; margin-bottom: 0.75rem; color: var(--text-muted);">Build with gcc/clang:</p>
          <pre><code>gcc $(pkg-config --cflags expanse) main.c $(pkg-config --libs expanse) -o main</code></pre>
        </div>

        <div id="tab-apt" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Configure the official Expanse APT repository (<a href="./apt/">view APT repository page</a>):</p>
          <pre><code># 1. Add repository source
echo "deb [trusted=yes] https://orieg.github.io/expanse/apt/ stable main" | sudo tee /etc/apt/sources.list.d/expanse.list

# 2. Update index and install runtime, dev headers, and compat symlinks
sudo apt-get update
sudo apt-get install -y libexpanse1 libexpanse-dev libjudy-compat</code></pre>
        </div>

        <div id="tab-rpm" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Configure the official Expanse YUM/DNF repository (<a href="./rpm/">view RPM repository page</a>):</p>
          <pre><code># 1. Add repository configuration
sudo dnf config-manager --add-repo https://orieg.github.io/expanse/rpm/expanse.repo

# 2. Install runtime library, development headers, and libjudy compat
sudo dnf install -y libexpanse libexpanse-devel libjudy-compat</code></pre>
        </div>

        <div id="tab-java" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">Add Java / JVM bindings via Maven Central (Java 22+ Foreign Function &amp; Memory / JNI):</p>
          <pre><code>&lt;!-- Maven pom.xml --&gt;
&lt;dependency&gt;
    &lt;groupId&gt;io.github.orieg&lt;/groupId&gt;
    &lt;artifactId&gt;expanse&lt;/artifactId&gt;
    &lt;version&gt;""" + version + """&lt;/version&gt;
&lt;/dependency&gt;</code></pre>
          <p style="margin-top: 1rem; margin-bottom: 0.75rem; color: var(--text-muted);">Usage example in Java:</p>
          <pre><code>import io.github.orieg.expanse.ExpanseMap;

try (var map = new ExpanseMap()) {
    map.put(42L, 100L);
    System.out.println("Value: " + map.get(42L));
}</code></pre>
        </div>

        <div id="tab-rocksdb" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">RocksDB pluggable MemTable plugin (<code>ExpanseMemTableRepFactory</code> / <code>integrations/rocksdb</code>):</p>
          <pre><code>// RocksDB integration: 2x-3x higher key density in RAM, fewer SSTable flushes
#include &lt;expanse_memtable.h&gt;
#include &lt;rocksdb/db.h&gt;
#include &lt;rocksdb/options.h&gt;

int main() {
    rocksdb::Options options;
    // Plug in Expanse digital trie MemTable representation
    options.memtable_factory.reset(
        rocksdb::NewExpanseMemTableRepFactory(/*leaf_capacity=*/64, /*enable_prefix_trie=*/true)
    );

    rocksdb::DB* db;
    rocksdb::DB::Open(options, "/tmp/rocksdb_expanse", &amp;db);
    db-&gt;Put(rocksdb::WriteOptions(), "user:1001", "payload");
    // ...
}</code></pre>
          <div style="margin-top: 1.5rem; border: 1px solid var(--border-color); border-radius: 8px; overflow: hidden; background: var(--bg-card);">
            <div style="padding: 0.75rem 1rem; border-bottom: 1px solid var(--border-color); font-size: 0.85rem; font-weight: 600; color: var(--text-muted);">
              Micro-Benchmark: ExpanseMemTable vs. RocksDB SkipListRep vs. VectorRep (100K keys, 16B key / 64B val)
            </div>
            <div style="padding: 1rem;">
              BENCH_ROCKSDB_SVG_PLACEHOLDER
            </div>
          </div>
        </div>

        <div id="tab-php" class="install-panel" style="display: none;">
          <p style="margin-bottom: 0.75rem; color: var(--text-muted);">PHP C extension (<code>ext-judy</code>), pure-PHP polyfill (<code>judy-polyfill</code>), and high-density PSR-16 cache (<code>judy-cache</code>):</p>
          <pre><code># Install Judy polyfill &amp; high-density cache
composer require orieg/judy-cache orieg/judy-polyfill</code></pre>
          <p style="margin-top: 1rem; margin-bottom: 0.75rem; color: var(--text-muted);">Usage example in PHP:</p>
          <pre><code>use Judy;
use Orieg\\JudyCache\\JudySimpleCache;

// Core digital trie array
$judy = new Judy(Judy::INT_TO_INT);
$judy[42] = 100;

// High-density PSR-16 cache with native TTL pruning &amp; compression
$cache = new JudySimpleCache();
$cache-&gt;set('user:42:profile', ['name' =&gt; 'Alice', 'role' =&gt; 'admin'], ttl: 3600);
$data = $cache-&gt;get('user:42:profile');</code></pre>
        </div>
      </div>
    </div>
  </section>

  <section>
    <div class="container">
      <div class="section-header">
        <span class="section-tag">Engineering</span>
        <h2 class="section-title">Canonical Documentation</h2>
        <p class="section-desc">Comprehensive architectural and algorithmic references.</p>
      </div>

      <div class="docs-grid">
        <a href="https://github.com/orieg/expanse/blob/main/docs/ARCHITECTURE.md" class="doc-link-card">
          <div class="doc-link-title">ARCHITECTURE.md &#8599;</div>
          <div class="doc-link-desc">Trie node layouts, memory packing, pointer tagging, concurrency design.</div>
        </a>
        <a href="https://github.com/orieg/expanse/blob/main/docs/DATABASE.md" class="doc-link-card">
          <div class="doc-link-title">DATABASE.md &#8599;</div>
          <div class="doc-link-desc">Database engine subsystems, MVCC visibility, string dictionaries, and RocksDB MemTable.</div>
        </a>
        <a href="https://github.com/orieg/expanse/blob/main/docs/BINDINGS_PYTHON.md" class="doc-link-card">
          <div class="doc-link-title">BINDINGS_PYTHON.md &#8599;</div>
          <div class="doc-link-desc">Pythonic bindings (expanse-trie), zero-copy buffers, and dictionary encoders.</div>
        </a>
        <a href="https://github.com/orieg/expanse/blob/main/docs/BINDINGS_JAVA.md" class="doc-link-card">
          <div class="doc-link-title">BINDINGS_JAVA.md &#8599;</div>
          <div class="doc-link-desc">JVM Foreign Function &amp; Memory (FFM) bindings with zero-GC overhead.</div>
        </a>
        <a href="https://github.com/orieg/expanse/blob/main/docs/ALGORITHMS.md" class="doc-link-card">
          <div class="doc-link-title">ALGORITHMS.md &#8599;</div>
          <div class="doc-link-desc">Algorithmic specifications, search kernels, SIMD/SWAR vectorization.</div>
        </a>
        <a href="https://github.com/orieg/expanse/blob/main/docs/COMPAT.md" class="doc-link-card">
          <div class="doc-link-title">COMPAT.md &#8599;</div>
          <div class="doc-link-desc">C ABI contracts, drop-in parity gates, error handling, packaging specifications.</div>
        </a>
        <a href="https://github.com/orieg/expanse/blob/main/docs/CI_CD_GUIDE.md" class="doc-link-card">
          <div class="doc-link-title">CI_CD_GUIDE.md &#8599;</div>
          <div class="doc-link-desc">CI/CD engineering standards, zero-regression gating, and multi-architecture matrices.</div>
        </a>
        <a href="https://github.com/orieg/expanse/blob/main/docs/BENCHMARKING.md" class="doc-link-card">
          <div class="doc-link-title">BENCHMARKING.md &#8599;</div>
          <div class="doc-link-desc">Instruction counting methodology, hardware counters, profiling guides.</div>
        </a>
        <a href="https://github.com/orieg/expanse/blob/main/docs/PACKAGING.md" class="doc-link-card">
          <div class="doc-link-title">PACKAGING.md &#8599;</div>
          <div class="doc-link-desc">Multi-arch packaging, glibc-hwcaps, APT/RPM repos, Windows/vcpkg distributions.</div>
        </a>
      </div>
    </div>
  </section>

  <footer>
    <div class="container">
""" + footer_meta + """
      <p>
        <strong>Expanse</strong> is open source software released under dual MIT / Apache-2.0 licenses.<br>
        Maintained by <a href="https://nicolas.brousse.info/">Nicolas Brousse</a> &bull; <a href="https://github.com/orieg/expanse">GitHub</a> &bull; <a href="https://crates.io/crates/expanse-trie">Crates.io</a> &bull; <a href="./apt/">APT Repo</a> &bull; <a href="./rpm/">RPM Repo</a>
      </p>
    </div>
  </footer>

  <script>
    function switchTab(tabId) {
      document.querySelectorAll('.tab-btn').forEach(btn => btn.classList.remove('active'));
      document.querySelectorAll('.install-panel').forEach(panel => panel.style.display = 'none');
      
      event.target.classList.add('active');
      document.getElementById(tabId).style.display = 'block';
    }

    function toggleTheme() {
      const current = document.documentElement.getAttribute('data-theme') || 
        (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
      const next = current === 'dark' ? 'light' : 'dark';
      document.documentElement.setAttribute('data-theme', next);
      localStorage.setItem('expanse-theme', next);
    }
  </script>
</body>
</html>
"""
    main_html = (
        main_html_template.replace("BENCH_ROCKSDB_SVG_PLACEHOLDER", rocksdb_svg)
        .replace("BENCH_COMP_SVG_PLACEHOLDER", comp_svg)
        .replace("BENCH_CONC_SVG_PLACEHOLDER", conc_svg)
        .replace("BENCH_YCSB_SVG_PLACEHOLDER", ycsb_svg)
    )

    with open(os.path.join(output_dir, "index.html"), "w", encoding="utf-8") as f:
        f.write(main_html)

    # 3. Visualizer
    visualizer_src = os.path.join(
        os.path.dirname(__file__), "..", "docs", "architecture_visualizer.html"
    )
    if os.path.isfile(visualizer_src):
        with open(visualizer_src, "r", encoding="utf-8") as f:
            v_content = f.read()

        v_content = v_content.replace(
            "localStorage.setItem('theme', theme);",
            "localStorage.setItem('expanse-theme', theme); localStorage.setItem('theme', theme);",
        )
        v_content = v_content.replace(
            "const savedTheme = localStorage.getItem('theme');",
            "const savedTheme = localStorage.getItem('expanse-theme') || localStorage.getItem('theme');",
        )

        nav_style = """<style>
""" + MAIN_CSS + """
</style>"""
        v_content = v_content.replace("</head>", f"{nav_style}\n</head>", 1)
        v_content = v_content.replace("<body>", f"<body>\n{nav_vis_html}", 1)
        with open(os.path.join(output_dir, "visualizer.html"), "w", encoding="utf-8") as f:
            f.write(v_content)

    # 4. APT & RPM repositories
    apt_out = os.path.join(output_dir, "apt")
    rpm_out = os.path.join(output_dir, "rpm")
    build_apt_repo(artifacts_dir, apt_out)
    build_rpm_repo(artifacts_dir, rpm_out)

    print(f"Complete GitHub Pages site generated in {output_dir}")

if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(
        description="Build complete GitHub Pages web distribution."
    )
    parser.add_argument(
        "artifacts_pos",
        nargs="?",
        default=None,
        help="Directory containing package artifacts",
    )
    parser.add_argument(
        "output_pos",
        nargs="?",
        default=None,
        help="Output directory for generated site",
    )
    parser.add_argument(
        "--artifacts-dir",
        "--input-dir",
        dest="artifacts_dir",
        default=None,
        help="Directory containing package artifacts",
    )
    parser.add_argument(
        "--output-dir",
        dest="output_dir",
        default=None,
        help="Output directory for generated site",
    )
    args = parser.parse_args()

    art_dir = args.artifacts_dir or args.artifacts_pos or "artifacts"
    out_dir = args.output_dir or args.output_pos or "pages-root"
    build_pages(art_dir, out_dir)
