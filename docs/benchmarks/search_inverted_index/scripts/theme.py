"""
docs/benchmarks/search_inverted_index/scripts/theme.py

Shared dual-theme SVG styling for the search / inverted-index suite, adhering
to the canonical design system used across the Expanse benchmark suites:
- High-contrast light default for standalone viewers (QuickLook, browsers)
- Dark overrides via @media (prefers-color-scheme: dark) and data-theme="dark"
- Palette:
    - Expanse: #16a34a (light) / #22c55e (dark) / #4ade80 (accent text)
    - Roaring: #2563eb (light) / #3b82f6 (dark) / #38bdf8 (blue text)
    - Loss / Warn: #ef4444 (light) / #f87171 (dark)
"""


def svg_header(width: int = 960, height: int = 300, title: str = "Benchmark Chart") -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" role="img" viewBox="0 0 {width} {height}" width="100%" height="100%">
  <title>{title}</title>
  <defs>
    <style>
      /* Light theme (default) */
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
      .t-val-amber {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11px; font-weight: 700; fill: #b45309; }}
      .t-val-muted {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11px; font-weight: 600; fill: #475569; }}
      .t-val-warn {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 10px; font-weight: 700; fill: #b91c1c; }}

      .t-note {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 500; fill: #64748b; }}

      .b-expanse {{ fill: #16a34a; }}
      .b-roaring {{ fill: #2563eb; }}
      .b-cursor {{ fill: #d97706; }}

      .badge-win {{ fill: #dcfce7; stroke: #86efac; stroke-width: 1px; rx: 3px; }}
      .badge-win-text {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 700; fill: #15803d; text-anchor: middle; }}

      .badge-loss {{ fill: #fee2e2; stroke: #fca5a5; stroke-width: 1px; rx: 3px; }}
      .badge-loss-text {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 700; fill: #991b1b; text-anchor: middle; }}

      /* Dark theme overrides */
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
        .t-val-amber {{ fill: #fbbf24; }}
        .t-val-muted {{ fill: #cbd5e1; }}
        .t-val-warn {{ fill: #f87171; }}
        .t-note {{ fill: #94a3b8; }}
        .b-expanse {{ fill: #22c55e; }}
        .b-roaring {{ fill: #3b82f6; }}
        .b-cursor {{ fill: #fbbf24; }}
        .badge-win {{ fill: #064e3b; stroke: #059669; }}
        .badge-win-text {{ fill: #6ee7b7; }}
        .badge-loss {{ fill: #450a0a; stroke: #dc2626; }}
        .badge-loss-text {{ fill: #fca5a5; }}
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
      :root[data-theme="dark"] .t-val-amber, [data-theme="dark"] .t-val-amber {{ fill: #fbbf24; }}
      :root[data-theme="dark"] .t-val-muted, [data-theme="dark"] .t-val-muted {{ fill: #cbd5e1; }}
      :root[data-theme="dark"] .t-val-warn, [data-theme="dark"] .t-val-warn {{ fill: #f87171; }}
      :root[data-theme="dark"] .t-note, [data-theme="dark"] .t-note {{ fill: #94a3b8; }}
      :root[data-theme="dark"] .b-expanse, [data-theme="dark"] .b-expanse {{ fill: #22c55e; }}
      :root[data-theme="dark"] .b-roaring, [data-theme="dark"] .b-roaring {{ fill: #3b82f6; }}
      :root[data-theme="dark"] .b-cursor, [data-theme="dark"] .b-cursor {{ fill: #fbbf24; }}
      :root[data-theme="dark"] .badge-win, [data-theme="dark"] .badge-win {{ fill: #064e3b; stroke: #059669; }}
      :root[data-theme="dark"] .badge-win-text, [data-theme="dark"] .badge-win-text {{ fill: #6ee7b7; }}
      :root[data-theme="dark"] .badge-loss, [data-theme="dark"] .badge-loss {{ fill: #450a0a; stroke: #dc2626; }}
      :root[data-theme="dark"] .badge-loss-text, [data-theme="dark"] .badge-loss-text {{ fill: #fca5a5; }}
    </style>
  </defs>
  <rect width="100%" height="100%" class="bg" rx="8"/>
  <rect width="100%" height="100%" class="border" rx="8"/>
"""
