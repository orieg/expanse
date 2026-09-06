"""
docs/benchmarks/masstree_comparison/scripts/theme.py

Shared dual-theme SVG styling for the Masstree vs Expanse benchmark comparison suite.
High-contrast light defaults for standalone viewers with dark-mode overrides
via both `@media (prefers-color-scheme: dark)` and `[data-theme="dark"]`.

Palette:
  - Expanse: #16a34a (light) / #22c55e (dark)
  - Masstree: #2563eb (light) / #3b82f6 (dark)  -- `.b-masstree`, this suite's declared accent

The chrome selectors below are byte-identical to the other suites' copies, which
`scripts/check_chart_themes.py` enforces. `.b-masstree` is the one addition and is an
alias of the competitor slot the other suites spell `.b-art`; naming it for the
system it draws keeps the generator readable.
"""


def svg_header(width: int = 960, height: int = 340, title: str = "Benchmark Chart") -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" role="img" viewBox="0 0 {width} {height}" width="100%" height="100%">
  <title>{title}</title>
  <defs>
    <style>
      .bg {{ fill: #ffffff; }}
      .border {{ stroke: #e2e8f0; stroke-width: 1px; fill: none; }}
      .grid {{ stroke: #f1f5f9; stroke-width: 1px; stroke-dasharray: 2,3; }}
      .axis {{ stroke: #cbd5e1; stroke-width: 1.5px; }}
      .divider {{ stroke: #e2e8f0; stroke-width: 1px; }}

      .t-title {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 11.5px; font-weight: 700; letter-spacing: 0.6px; fill: #0f172a; text-transform: uppercase; }}
      .t-sub {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 10px; font-weight: 500; fill: #475569; }}
      .t-unit {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 10px; font-weight: 600; fill: #475569; }}
      .t-axis-label {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 500; fill: #64748b; }}
      .t-bar-label {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 11px; font-weight: 700; fill: #0f172a; }}
      .t-legend {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 10.5px; font-weight: 600; fill: #0f172a; }}

      .t-val-accent {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11px; font-weight: 700; fill: #15803d; }}
      .t-val-blue {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11px; font-weight: 600; fill: #2563eb; }}
      .t-note {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 500; fill: #64748b; }}

      .b-expanse {{ fill: #16a34a; }}
      .b-art {{ fill: #2563eb; }}
      .b-masstree {{ fill: #2563eb; }}
      .b-btree {{ fill: #d97706; }}
      .b-hashmap {{ fill: #7c3aed; }}

      .badge-win {{ fill: #dcfce7; stroke: #86efac; stroke-width: 1px; }}
      .badge-win-text {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 700; fill: #15803d; text-anchor: middle; }}
      .badge-loss {{ fill: #fef3c7; stroke: #fcd34d; stroke-width: 1px; }}
      .badge-loss-text {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 700; fill: #b45309; text-anchor: middle; }}

      @media (prefers-color-scheme: dark) {{
        .bg {{ fill: #0d1117; }}
        .border {{ stroke: #30363d; }}
        .grid {{ stroke: #21262d; }}
        .axis {{ stroke: #484f58; }}
        .divider {{ stroke: #21262d; }}
        .t-title {{ fill: #f0f6fc; }}
        .t-sub {{ fill: #94a3b8; }}
        .t-unit {{ fill: #94a3b8; }}
        .t-axis-label {{ fill: #94a3b8; }}
        .t-bar-label {{ fill: #f8fafc; }}
        .t-legend {{ fill: #f8fafc; }}
        .t-val-accent {{ fill: #4ade80; }}
        .t-val-blue {{ fill: #38bdf8; }}
        .t-note {{ fill: #94a3b8; }}
        .b-expanse {{ fill: #22c55e; }}
        .b-art {{ fill: #3b82f6; }}
        .b-masstree {{ fill: #3b82f6; }}
        .b-btree {{ fill: #f59e0b; }}
        .b-hashmap {{ fill: #a855f7; }}
        .badge-win {{ fill: #052e16; stroke: #166534; }}
        .badge-win-text {{ fill: #4ade80; }}
        .badge-loss {{ fill: #451a03; stroke: #92400e; }}
        .badge-loss-text {{ fill: #fbbf24; }}
      }}

      :root[data-theme="dark"] .bg, [data-theme="dark"] .bg {{ fill: #0d1117; }}
      :root[data-theme="dark"] .border, [data-theme="dark"] .border {{ stroke: #30363d; }}
      :root[data-theme="dark"] .grid, [data-theme="dark"] .grid {{ stroke: #21262d; }}
      :root[data-theme="dark"] .axis, [data-theme="dark"] .axis {{ stroke: #484f58; }}
      :root[data-theme="dark"] .divider, [data-theme="dark"] .divider {{ stroke: #21262d; }}
      :root[data-theme="dark"] .t-title, [data-theme="dark"] .t-title {{ fill: #f0f6fc; }}
      :root[data-theme="dark"] .t-sub, [data-theme="dark"] .t-sub {{ fill: #94a3b8; }}
      :root[data-theme="dark"] .t-unit, [data-theme="dark"] .t-unit {{ fill: #94a3b8; }}
      :root[data-theme="dark"] .t-axis-label, [data-theme="dark"] .t-axis-label {{ fill: #94a3b8; }}
      :root[data-theme="dark"] .t-bar-label, [data-theme="dark"] .t-bar-label {{ fill: #f8fafc; }}
      :root[data-theme="dark"] .t-legend, [data-theme="dark"] .t-legend {{ fill: #f8fafc; }}
      :root[data-theme="dark"] .t-val-accent, [data-theme="dark"] .t-val-accent {{ fill: #4ade80; }}
      :root[data-theme="dark"] .t-val-blue, [data-theme="dark"] .t-val-blue {{ fill: #38bdf8; }}
      :root[data-theme="dark"] .t-note, [data-theme="dark"] .t-note {{ fill: #94a3b8; }}
      :root[data-theme="dark"] .b-expanse, [data-theme="dark"] .b-expanse {{ fill: #22c55e; }}
      :root[data-theme="dark"] .b-art, [data-theme="dark"] .b-art {{ fill: #3b82f6; }}
      :root[data-theme="dark"] .b-masstree, [data-theme="dark"] .b-masstree {{ fill: #3b82f6; }}
      :root[data-theme="dark"] .b-btree, [data-theme="dark"] .b-btree {{ fill: #f59e0b; }}
      :root[data-theme="dark"] .b-hashmap, [data-theme="dark"] .b-hashmap {{ fill: #a855f7; }}
      :root[data-theme="dark"] .badge-win, [data-theme="dark"] .badge-win {{ fill: #052e16; stroke: #166534; }}
      :root[data-theme="dark"] .badge-win-text, [data-theme="dark"] .badge-win-text {{ fill: #4ade80; }}
      :root[data-theme="dark"] .badge-loss, [data-theme="dark"] .badge-loss {{ fill: #451a03; stroke: #92400e; }}
      :root[data-theme="dark"] .badge-loss-text, [data-theme="dark"] .badge-loss-text {{ fill: #fbbf24; }}
    </style>
  </defs>
  <rect width="100%" height="100%" class="bg" rx="6" />
  <rect width="100%" height="100%" class="border" rx="6" />
"""


def svg_footer() -> str:
    return "</svg>\n"
