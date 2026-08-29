#!/usr/bin/env python3
"""Canonical theme, navigation, and shared front-end assets for the GitHub Pages site.

Single source of truth consumed by build_pages.py, build_apt_repo.py, and
build_rpm_repo.py. The theme model is a three-state cycle:

  system (default, follows prefers-color-scheme live) -> light -> dark -> system

localStorage['expanse-theme'] stores only an explicit override ('light'/'dark');
absence (or any other value) means "follow the system preference". The resolved
theme is always set explicitly as data-theme="light"|"dark" on <html>, and the
user-facing mode as data-theme-mode="system"|"light"|"dark" (drives toggle icons).

docs/architecture_visualizer.html embeds a byte-identical copy of the head and
toggle scripts so it works standalone; build_pages.py asserts the copies have
not drifted from the canonical bodies below.
"""

THEME_STORAGE_KEY = "expanse-theme"

# Palette single-source: THEME_CSS_VARS and the namespaced visualizer nav bundle
# are both generated from these dicts.
_DARK_PALETTE = {
    "bg": "#090d16",
    "card-bg": "#111827",
    "card-inner": "#0b1120",
    "border": "#1f293d",
    "border-accent": "rgba(56, 189, 248, 0.4)",
    "text": "#e2e8f0",
    "text-muted": "#94a3b8",
    "heading": "#f8fafc",
    "accent": "#38bdf8",
    "accent-hover": "#7dd3fc",
    "accent-green": "#10b981",
    "code-bg": "#030712",
    "bench-bg": "#0d1117",
    "quote-bg": "linear-gradient(180deg, rgba(30, 41, 59, 0.5) 0%, rgba(15, 23, 42, 0.8) 100%)",
    "navbar-bg": "rgba(9, 13, 22, 0.85)",
    "nav-pill-bg": "rgba(56, 189, 248, 0.1)",
    "nav-pill-border": "rgba(56, 189, 248, 0.25)",
    "btn-secondary-bg": "#111827",
    "btn-secondary-hover": "#1e293b",
    "btn-secondary-border": "#1f293d",
    "btn-secondary-text": "#f8fafc",
    "badge-bg": "#111827",
    "badge-border": "#1f293d",
    "badge-text": "#38bdf8",
    "badge-near": "#facc15",
    "badge-gap": "#f87171",
    "spotlight-bg": "linear-gradient(135deg, #0f172a 0%, #1e1b4b 50%, #0f172a 100%)",
    "spotlight-border": "rgba(99, 102, 241, 0.4)",
    "tab-active-bg": "rgba(56, 189, 248, 0.05)",
    "table-header-color": "#f8fafc",
    "table-row-border": "#1f293d",
}

_LIGHT_PALETTE = {
    "bg": "#f8fafc",
    "card-bg": "#ffffff",
    "card-inner": "#f1f5f9",
    "border": "#e2e8f0",
    "border-accent": "rgba(2, 132, 199, 0.4)",
    "text": "#334155",
    "text-muted": "#64748b",
    "heading": "#0f172a",
    "accent": "#0284c7",
    "accent-hover": "#0369a1",
    "accent-green": "#059669",
    "code-bg": "#0f172a",
    "bench-bg": "#ffffff",
    "quote-bg": "linear-gradient(180deg, rgba(241, 245, 249, 0.9) 0%, rgba(226, 232, 240, 0.7) 100%)",
    "navbar-bg": "rgba(248, 250, 252, 0.88)",
    "nav-pill-bg": "rgba(2, 132, 199, 0.08)",
    "nav-pill-border": "rgba(2, 132, 199, 0.25)",
    "btn-secondary-bg": "#ffffff",
    "btn-secondary-hover": "#f1f5f9",
    "btn-secondary-border": "#cbd5e1",
    "btn-secondary-text": "#0f172a",
    "badge-bg": "#ffffff",
    "badge-border": "#e2e8f0",
    "badge-text": "#0284c7",
    "badge-near": "#b45309",
    "badge-gap": "#be123c",
    "spotlight-bg": "linear-gradient(135deg, #f0f9ff 0%, #e0e7ff 50%, #f0fdf4 100%)",
    "spotlight-border": "rgba(99, 102, 241, 0.3)",
    "tab-active-bg": "rgba(2, 132, 199, 0.08)",
    "table-header-color": "#0f172a",
    "table-row-border": "#e2e8f0",
}


def _vars_block(selector: str, palette: dict, color_scheme: str) -> str:
    lines = "\n".join(f"      --{name}: {value};" for name, value in palette.items())
    return f"    {selector} {{\n{lines}\n      color-scheme: {color_scheme};\n    }}"


THEME_CSS_VARS = (
    "\n"
    + _vars_block(":root", _DARK_PALETTE, "dark")
    + "\n\n"
    + _vars_block('[data-theme="light"]', _LIGHT_PALETTE, "light")
    + "\n"
)

BASE_CSS = """
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.6;
      overflow-x: hidden;
      -webkit-font-smoothing: antialiased;
    }
"""

NAV_CSS = """
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
    @media (max-width: 768px) {
      .navbar {
        padding: 0.6rem 1rem;
        flex-direction: column;
        align-items: stretch;
        gap: 0.5rem;
      }
      .nav-top { width: 100%; }
      .nav-scroll {
        width: 100%;
        overflow-x: auto;
        -webkit-overflow-scrolling: touch;
        padding-bottom: 0.2rem;
        justify-content: flex-start;
        scrollbar-width: none;
        -webkit-mask-image: linear-gradient(to right, #000 0, #000 calc(100% - 28px), transparent 100%);
        mask-image: linear-gradient(to right, #000 0, #000 calc(100% - 28px), transparent 100%);
      }
      .nav-scroll::-webkit-scrollbar { display: none; }
      .nav-scroll.at-end {
        -webkit-mask-image: none;
        mask-image: none;
      }
      .nav-links { gap: 0.85rem; }
    }
"""

THEME_TOGGLE_CSS = """
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
    .theme-icon-system, .theme-icon-sun, .theme-icon-moon { display: none; }
    [data-theme-mode="system"] .theme-icon-system { display: inline; }
    [data-theme-mode="light"] .theme-icon-sun { display: inline; }
    [data-theme-mode="dark"] .theme-icon-moon { display: inline; }
    :root:not([data-theme-mode]) .theme-icon-system { display: inline; }

    .theme-toggle-mobile { display: flex; }
    .theme-toggle-desktop { display: none; }

    @media (min-width: 769px) {
      .theme-toggle-mobile { display: none !important; }
      .theme-toggle-desktop { display: flex !important; }
    }
    @media (max-width: 768px) {
      .theme-toggle-mobile { display: flex !important; }
      .theme-toggle-desktop { display: none !important; }
    }
"""

COPY_BTN_CSS = """
    .code-copy-wrap { position: relative; }
    .code-copy-btn {
      position: absolute;
      top: 0.5rem;
      right: 0.5rem;
      padding: 0.3rem 0.6rem;
      font-size: 0.75rem;
      font-weight: 600;
      font-family: inherit;
      border-radius: 6px;
      border: 1px solid var(--border);
      background: var(--card-bg);
      color: var(--text-muted);
      cursor: pointer;
      opacity: 0.75;
      transition: opacity 0.15s ease, color 0.15s ease, border-color 0.15s ease;
    }
    .code-copy-btn:hover, .code-copy-btn:focus-visible {
      opacity: 1;
      color: var(--accent);
      border-color: var(--accent);
    }
    .code-copy-btn.copied {
      opacity: 1;
      color: var(--accent-green);
      border-color: var(--accent-green);
    }
"""

# --- Canonical theme JS -----------------------------------------------------
# THEME_HEAD_JS_BODY and THEME_TOGGLE_JS_BODY are embedded byte-identically in
# docs/architecture_visualizer.html (standalone operation); build_pages.py
# asserts they have not drifted. Edit them ONLY here, then mirror into the
# visualizer.

THEME_HEAD_JS_BODY = """    (function() {
      'use strict';
      if (window.__expanseApplyTheme) { return; }
      var KEY = 'expanse-theme';
      var LABELS = {
        system: 'Color theme: system (click to switch to light)',
        light: 'Color theme: light (click to switch to dark)',
        dark: 'Color theme: dark (click to switch to system)'
      };
      function stored() {
        try {
          var v = localStorage.getItem(KEY);
          return (v === 'light' || v === 'dark') ? v : null;
        } catch (e) { return null; }
      }
      function systemTheme() {
        return (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light';
      }
      function apply() {
        var override = stored();
        var theme = override || systemTheme();
        var mode = override || 'system';
        var root = document.documentElement;
        root.setAttribute('data-theme', theme);
        root.setAttribute('data-theme-mode', mode);
        var buttons = document.querySelectorAll('.theme-toggle');
        for (var i = 0; i < buttons.length; i++) {
          buttons[i].setAttribute('aria-label', LABELS[mode]);
          buttons[i].setAttribute('title', LABELS[mode]);
        }
        if (window.__expanseThemeChanged) { window.__expanseThemeChanged(theme); }
      }
      window.__expanseApplyTheme = apply;
      apply();
      if (window.matchMedia) {
        var mq = window.matchMedia('(prefers-color-scheme: dark)');
        if (mq.addEventListener) { mq.addEventListener('change', apply); }
        else if (mq.addListener) { mq.addListener(apply); }
      }
      if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', apply);
      }
    })();
"""

THEME_HEAD_JS = "<script>\n" + THEME_HEAD_JS_BODY + "  </script>"

THEME_TOGGLE_JS_BODY = """    function toggleTheme() {
      var KEY = 'expanse-theme';
      var v = null;
      try { v = localStorage.getItem(KEY); } catch (e) {}
      var mode = (v === 'light' || v === 'dark') ? v : 'system';
      var next = mode === 'system' ? 'light' : (mode === 'light' ? 'dark' : 'system');
      try {
        if (next === 'system') { localStorage.removeItem(KEY); }
        else { localStorage.setItem(KEY, next); }
      } catch (e) {}
      if (window.__expanseApplyTheme) { window.__expanseApplyTheme(); }
    }
"""

THEME_TOGGLE_JS = "<script>\n" + THEME_TOGGLE_JS_BODY + "  </script>"

# Progressive enhancements shared by the portal, APT, and RPM pages:
# copy-to-clipboard buttons on every <pre>, and removal of the right-edge
# fade mask on horizontal scrollers once scrolled to the end.
SITE_JS = """<script>
    (function() {
      'use strict';
      document.querySelectorAll('.nav-scroll, .install-nav').forEach(function(el) {
        function update() {
          el.classList.toggle('at-end', el.scrollLeft + el.clientWidth >= el.scrollWidth - 4);
        }
        el.addEventListener('scroll', update, { passive: true });
        window.addEventListener('resize', update);
        update();
      });

      if (!navigator.clipboard) { return; }
      document.querySelectorAll('pre').forEach(function(pre) {
        var wrap = document.createElement('div');
        wrap.className = 'code-copy-wrap';
        pre.parentNode.insertBefore(wrap, pre);
        wrap.appendChild(pre);
        var btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'code-copy-btn';
        btn.textContent = 'Copy';
        btn.setAttribute('aria-label', 'Copy code to clipboard');
        btn.setAttribute('aria-live', 'polite');
        btn.addEventListener('click', function() {
          navigator.clipboard.writeText(pre.innerText.replace(/\\n$/, '')).then(function() {
            btn.textContent = 'Copied \\u2713';
            btn.classList.add('copied');
            setTimeout(function() {
              btn.textContent = 'Copy';
              btn.classList.remove('copied');
            }, 2000);
          });
        });
        wrap.appendChild(btn);
      });
    })();
  </script>"""


_TOGGLE_BUTTON = """<button class="theme-toggle theme-toggle-{variant}" type="button" onclick="toggleTheme()" aria-label="Toggle color theme" title="Toggle color theme">
        <span class="theme-icon-system" aria-hidden="true">&#9681;</span>
        <span class="theme-icon-sun" aria-hidden="true">&#9728;</span>
        <span class="theme-icon-moon" aria-hidden="true">&#9790;</span>
      </button>"""


def make_nav(version: str, active: str, base: str = "./") -> str:
    """Renders the shared navbar. active in {home, benchmarks, visualizer, apt, rpm};
    base is './' for root-level pages and '../' for apt/rpm subpages."""
    links = [
        ("home", base, "Home"),
        ("benchmarks", f"{base}#benchmarks", "Benchmarks"),
        ("visualizer", f"{base}visualizer.html", "Visualizer"),
        ("apt", f"{base}apt/", "APT"),
        ("rpm", f"{base}rpm/", "RPM"),
    ]
    items = "\n".join(
        f'        <li><a href="{href}"{static_cls}>{label}</a></li>'
        for key, href, label in links
        for static_cls in [' class="active"' if key == active else ""]
    )
    return f"""  <header class="navbar">
    <div class="nav-top">
      <a href="{base}" class="nav-brand">
        <div class="nav-logo">E</div>
        <span>Expanse</span>
      </a>
      {_TOGGLE_BUTTON.format(variant="mobile")}
    </div>
    <nav class="nav-scroll" aria-label="Site">
      <ul class="nav-links">
{items}
        <li><a href="https://github.com/orieg/expanse/blob/main/docs/ARCHITECTURE.md">Docs</a></li>
        <li><a href="https://github.com/orieg/expanse" class="nav-pill">GitHub &bull; {version}</a></li>
      </ul>
      {_TOGGLE_BUTTON.format(variant="desktop")}
    </nav>
  </header>"""


# --- Namespaced bundle for the visualizer -----------------------------------
# The visualizer defines its own palette with overlapping var names (--bg,
# --border, --card-inner, ...). To inject the shared navbar without clobbering
# it (or being clobbered), every variable the nav/toggle/copy CSS consumes is
# rewritten to an --exp-nav-* alias with its own dark/light definitions.

_NAV_VAR_NAMES = [
    "bg",
    "navbar-bg",
    "border",
    "heading",
    "text",
    "text-muted",
    "accent",
    "accent-green",
    "nav-pill-bg",
    "nav-pill-border",
    "card-inner",
    "card-bg",
]


def _namespace_css(css: str) -> str:
    for name in _NAV_VAR_NAMES:
        css = css.replace(f"var(--{name})", f"var(--exp-nav-{name})")
    return css


def _nav_var_prelude() -> str:
    dark = "\n".join(f"      --exp-nav-{n}: {_DARK_PALETTE[n]};" for n in _NAV_VAR_NAMES)
    light = "\n".join(f"      --exp-nav-{n}: {_LIGHT_PALETTE[n]};" for n in _NAV_VAR_NAMES)
    return (
        "    :root {\n" + dark + "\n    }\n"
        '    [data-theme="light"] {\n' + light + "\n    }\n"
    )


VISUALIZER_NAV_BUNDLE_CSS = _nav_var_prelude() + _namespace_css(
    NAV_CSS + THEME_TOGGLE_CSS + COPY_BTN_CSS
)
