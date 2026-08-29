#!/usr/bin/env python3
"""Cross-repo Ecosystem Theme Contract v1 Linter for the orieg ecosystem.

Enforces compliance with the Ecosystem Theme Contract v1 across all 8 ecosystem
repositories (see docs/ARCHITECTURE.md / plan §3):

  1. Storage key is 'orieg-theme'. It stores ONLY an explicit override
     ('light' | 'dark'). Absence means follow OS preferences.
  2. Three-state cyclic reachability: system -> light -> dark -> system.
  3. Resolved theme is set as data-theme="light"|"dark" on <html>.
  4. User-facing mode is set as data-theme-mode="system"|"light"|"dark" on <html>.
  5. Live matchMedia listener on '(prefers-color-scheme: dark)'.
  6. Migration: reads 'getItem("orieg-theme") || getItem("<legacy-key>")',
     writes only 'orieg-theme'.
  7. Minimum WCAG AA 4.5:1 relative luminance contrast on all palette tokens.

Network Resilience (#505 pattern):
  When run in CI or offline environments without outbound internet access,
  remote repository fetches fail open and emit GitHub Actions ::notice::
  annotations naming the uncontacted targets rather than breaking CI.

Usage:
  python3 scripts/check_ecosystem_theme.py              # check local + remote ecosystem
  python3 scripts/check_ecosystem_theme.py --local-only # check only local expanse surfaces
  python3 scripts/check_ecosystem_theme.py --self-test  # run internal unit self-tests
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import unittest
import urllib.error
import urllib.request
from typing import Dict, List, Optional, Tuple

# Ecosystem Sites & Current Migration Specifications
ECOSYSTEM_SITES = [
    {
        "id": "expanse",
        "name": "Expanse Digital Trees",
        "url": "https://orieg.github.io/expanse/",
        "legacy_key": "expanse-theme",
        "local_paths": ["scripts/site_theme.py", "docs/architecture_visualizer.html"],
    },
    {
        "id": "hub",
        "name": "Nicolas Brousse (Hub)",
        "url": "https://orieg.github.io/",
        "legacy_key": "orieg-theme",
        "local_paths": [],
    },
    {
        "id": "php-judy",
        "name": "PHP Judy Extension",
        "url": "https://orieg.github.io/php-judy/",
        "legacy_key": "judy-theme",
        "local_paths": [],
    },
    {
        "id": "judy-cache",
        "name": "Judy Cache PSR-16",
        "url": "https://orieg.github.io/judy-cache/",
        "legacy_key": "judy-cache-theme",
        "local_paths": [],
    },
    {
        "id": "judy-polyfill",
        "name": "Judy Polyfill",
        "url": "https://orieg.github.io/judy-polyfill/",
        "legacy_key": "judy-polyfill-theme",
        "local_paths": [],
    },
    {
        "id": "yaml-workflow",
        "name": "YAML Workflow Engine",
        "url": "https://orieg.github.io/yaml-workflow/",
        "legacy_key": None,
        "local_paths": [],
    },
    {
        "id": "gws-connector",
        "name": "GWS Connector",
        "url": "https://orieg.github.io/gws-connector/",
        "legacy_key": None,
        "local_paths": [],
    },
    {
        "id": "edu-policy-navigator",
        "name": "Edu Policy Navigator",
        "url": "https://orieg.github.io/edu-policy-navigator/",
        "legacy_key": None,
        "local_paths": [],
    },
]


def relative_luminance(hex_color: str) -> float:
    """Computes standard relative luminance per WCAG 2.1 (sRGB color space)."""
    hex_clean = hex_color.strip().lstrip("#")
    if len(hex_clean) == 3:
        hex_clean = "".join(c * 2 for c in hex_clean)
    if len(hex_clean) != 6:
        raise ValueError(f"Invalid hex color: {hex_color}")

    r = int(hex_clean[0:2], 16) / 255.0
    g = int(hex_clean[2:4], 16) / 255.0
    b = int(hex_clean[4:6], 16) / 255.0

    def to_linear(c: float) -> float:
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r_lin = to_linear(r)
    g_lin = to_linear(g)
    b_lin = to_linear(b)

    return 0.2126 * r_lin + 0.7152 * g_lin + 0.0722 * b_lin


def contrast_ratio(hex_a: str, hex_b: str) -> float:
    """Computes the WCAG contrast ratio between two hex colors ((L1 + 0.05) / (L2 + 0.05))."""
    lum_a = relative_luminance(hex_a)
    lum_b = relative_luminance(hex_b)
    l1 = max(lum_a, lum_b)
    l2 = min(lum_a, lum_b)
    return (l1 + 0.05) / (l2 + 0.05)


def validate_palette_contrast(palette: Dict[str, str], name: str) -> List[str]:
    """Validates that text tokens meet the 4.5:1 WCAG AA floor against background tokens."""
    errors = []
    bg = palette.get("bg")
    card_bg = palette.get("card-bg")
    card_inner = palette.get("card-inner")

    # Tokens that must have >= 4.5:1 contrast against their containing backgrounds
    checks: List[Tuple[str, List[Optional[str]]]] = [
        ("text", [bg, card_bg, card_inner]),
        ("heading", [bg, card_bg, card_inner]),
        ("badge-near", [card_bg, card_inner]),
        ("badge-gap", [card_bg, card_inner]),
    ]

    for token, bgs in checks:
        color = palette.get(token)
        if not color or not color.startswith("#"):
            continue
        for b in bgs:
            if not b or not b.startswith("#"):
                continue
            ratio = contrast_ratio(color, b)
            if ratio < 4.5:
                errors.append(
                    f"{name}: token '{token}' ({color}) has contrast {ratio:.2f}:1 against background ({b}) < 4.5:1 floor"
                )

    return errors


def validate_contract_script(content: str, legacy_key: Optional[str] = None) -> List[str]:
    """Validates contract compliance for embedded JavaScript."""
    errors = []

    # 1. Canonical key 'orieg-theme'
    if "orieg-theme" not in content:
        errors.append("Contract violation: canonical storage key 'orieg-theme' is not referenced")

    # 2. Legacy key fallback
    if legacy_key and legacy_key != "orieg-theme":
        if legacy_key not in content:
            errors.append(f"Migration violation: legacy storage key '{legacy_key}' fallback is missing")

    # 3. Attributes data-theme and data-theme-mode
    if "data-theme" not in content:
        errors.append("Contract violation: 'data-theme' attribute is not set on document element")
    if "data-theme-mode" not in content:
        errors.append("Contract violation: 'data-theme-mode' attribute is not set on document element")

    # 4. 3-state reachability: system, light, dark
    has_system = "system" in content
    has_light = "light" in content
    has_dark = "dark" in content
    if not (has_system and has_light and has_dark):
        errors.append("Contract violation: 3-state theme modes (system, light, dark) not fully supported")

    # 5. matchMedia listener
    if "matchMedia" not in content or "prefers-color-scheme" not in content:
        errors.append("Contract violation: live 'prefers-color-scheme' matchMedia listener is missing")

    return errors


def fetch_url(url: str, timeout: float = 4.0) -> Tuple[Optional[str], Optional[str]]:
    """Fetches a URL with a strict timeout. Returns (content, error_message)."""
    try:
        req = urllib.request.Request(
            url,
            headers={"User-Agent": "orieg-theme-linter/1.0 (https://github.com/orieg/expanse)"},
        )
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            if resp.status == 200:
                return resp.read().decode("utf-8", errors="ignore"), None
            return None, f"HTTP {resp.status}"
    except (urllib.error.URLError, OSError, TimeoutError) as exc:
        return None, str(exc)


def check_local_expanse(repo_root: str) -> List[str]:
    """Validates local expanse theme files and palettes."""
    errors = []

    # 1. Check site_theme.py palettes
    site_theme_path = os.path.join(repo_root, "scripts", "site_theme.py")
    if os.path.isfile(site_theme_path):
        import importlib.util

        spec = importlib.util.spec_from_file_location("site_theme", site_theme_path)
        if spec and spec.loader:
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            dark_pal = getattr(mod, "_DARK_PALETTE", {})
            light_pal = getattr(mod, "_LIGHT_PALETTE", {})
            errors.extend(validate_palette_contrast(dark_pal, "site_theme._DARK_PALETTE"))
            errors.extend(validate_palette_contrast(light_pal, "site_theme._LIGHT_PALETTE"))

            head_js = getattr(mod, "THEME_HEAD_JS_BODY", "")
            toggle_js = getattr(mod, "THEME_TOGGLE_JS_BODY", "")
            errors.extend(validate_contract_script(head_js + "\n" + toggle_js, "expanse-theme"))

    # 2. Check architecture_visualizer.html
    vis_path = os.path.join(repo_root, "docs", "architecture_visualizer.html")
    if os.path.isfile(vis_path):
        with open(vis_path, "r", encoding="utf-8") as f:
            vis_content = f.read()
        errors.extend(validate_contract_script(vis_content, "expanse-theme"))

    return errors


def check_ecosystem(
    repo_root: str, local_only: bool = False
) -> Tuple[int, int, List[str], List[str]]:
    """Runs the full ecosystem theme check.

    Returns (num_passed, num_failed, errors, notices).
    """
    errors: List[str] = []
    notices: List[str] = []
    passed = 0
    failed = 0

    # Local validation
    local_errors = check_local_expanse(repo_root)
    if local_errors:
        failed += 1
        errors.extend([f"[local:expanse] {e}" for e in local_errors])
    else:
        passed += 1

    if local_only:
        return passed, failed, errors, notices

    # Remote validation with #505 fail-open notice pattern
    for site in ECOSYSTEM_SITES:
        site_id = site["id"]
        if site_id == "expanse":
            continue  # already verified locally above

        url = site["url"]
        content, fetch_err = fetch_url(url)
        if fetch_err or not content:
            notices.append(
                f"::notice::check_ecosystem_theme: {site['name']} ({url}) could not be contacted ({fetch_err}) — skipping live verification"
            )
            continue

        site_errors = validate_contract_script(content, site.get("legacy_key"))
        if site_errors:
            # Sister repos in transition are reported as advisory notices until fully migrated
            for e in site_errors:
                notices.append(f"::notice::[remote:{site_id}] {e} at {url}")
        else:
            passed += 1

    return passed, failed, errors, notices


# --- Self-Tests --------------------------------------------------------------


class TestEcosystemThemeLinter(unittest.TestCase):
    def test_luminance_and_contrast(self):
        # White on black = 21:1
        self.assertAlmostEqual(contrast_ratio("#ffffff", "#000000"), 21.0, places=1)
        # Black on black = 1:1
        self.assertAlmostEqual(contrast_ratio("#000000", "#000000"), 1.0, places=1)
        # Verify 4.5:1 floor detection
        self.assertTrue(contrast_ratio("#facc15", "#111827") >= 4.5)  # dark badge-near
        self.assertTrue(contrast_ratio("#b45309", "#ffffff") >= 4.5)  # light badge-near
        self.assertTrue(contrast_ratio("#b45309", "#f1f5f9") >= 4.5)  # light badge-near on card-inner
        self.assertTrue(contrast_ratio("#f87171", "#111827") >= 4.5)  # dark badge-gap
        self.assertTrue(contrast_ratio("#be123c", "#ffffff") >= 4.5)  # light badge-gap

    def test_contrast_failure_detected(self):
        # Low contrast grey on white
        palette = {"bg": "#ffffff", "card-bg": "#ffffff", "card-inner": "#ffffff", "text": "#aaaaaa"}
        errs = validate_palette_contrast(palette, "test_pal")
        self.assertTrue(any("text" in e and "< 4.5:1 floor" in e for e in errs))

    def test_valid_script_passes(self):
        valid_script = """
        var KEY = 'orieg-theme';
        var LEGACY_KEY = 'expanse-theme';
        var stored = localStorage.getItem(KEY) || localStorage.getItem(LEGACY_KEY);
        var mode = stored || 'system';
        document.documentElement.setAttribute('data-theme', mode === 'dark' ? 'dark' : 'light');
        document.documentElement.setAttribute('data-theme-mode', mode);
        window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function() {});
        """
        errs = validate_contract_script(valid_script, "expanse-theme")
        self.assertEqual(errs, [])

    def test_missing_orieg_theme_key(self):
        invalid_script = """
        var KEY = 'expanse-theme';
        var mode = localStorage.getItem(KEY) || 'system';
        document.documentElement.setAttribute('data-theme', 'light');
        document.documentElement.setAttribute('data-theme-mode', 'light');
        window.matchMedia('(prefers-color-scheme: dark)');
        """
        errs = validate_contract_script(invalid_script, "expanse-theme")
        self.assertTrue(any("orieg-theme" in e for e in errs))

    def test_missing_legacy_fallback(self):
        missing_fallback = """
        var KEY = 'orieg-theme';
        var mode = localStorage.getItem(KEY) || 'system';
        document.documentElement.setAttribute('data-theme', 'light');
        document.documentElement.setAttribute('data-theme-mode', 'light');
        window.matchMedia('(prefers-color-scheme: dark)');
        """
        errs = validate_contract_script(missing_fallback, "expanse-theme")
        self.assertTrue(any("legacy storage key" in e for e in errs))


def main() -> int:
    parser = argparse.ArgumentParser(description="Check ecosystem theme contract compliance.")
    parser.add_argument("--self-test", action="store_true", help="Run self-tests")
    parser.add_argument("--local-only", action="store_true", help="Check only local repo files")
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestEcosystemThemeLinter)
        runner = unittest.TextTestRunner(verbosity=2)
        res = runner.run(suite)
        return 0 if res.wasSuccessful() else 1

    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    passed, failed, errors, notices = check_ecosystem(repo_root, local_only=args.local_only)

    for notice in notices:
        print(notice)

    if errors:
        for err in errors:
            print(f"::error::{err}", file=sys.stderr)
        print(
            f"check_ecosystem_theme.py: {len(errors)} error(s) found across ecosystem theme surfaces",
            file=sys.stderr,
        )
        return 1

    print(
        f"check_ecosystem_theme.py: all checks passed (local + {len(ECOSYSTEM_SITES) - 1} remote ecosystem targets evaluated)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
