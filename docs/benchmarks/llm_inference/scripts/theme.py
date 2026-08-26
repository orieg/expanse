"""
docs/benchmarks/llm_inference/scripts/theme.py

Shared reusable styling and SVG template generator adhering to the canonical design system:
- High contrast, clean default styles for standalone viewers (QuickLook, browsers, direct file access)
- Dark mode overrides via @media (prefers-color-scheme: dark) and data-theme="dark"
- Standardized color palette:
    - Expanse: #16a34a (light) / #22c55e (dark) / #4ade80 (accent text)
    - HuggingFace / Dict: #2563eb (light) / #3b82f6 (dark) / #38bdf8 (blue text)
    - NumPy / Baseline: #64748b (light) / #475569 (dark) / #94a3b8 (muted text)
    - Disqualified / Warn: #ef4444 (light) / #f87171 (dark)
"""

def svg_header(width: int = 960, height: int = 320, title: str = "Benchmark Chart") -> str:
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
      .t-val-muted {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11px; font-weight: 600; fill: #475569; }}
      .t-val-warn {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 10px; font-weight: 700; fill: #b91c1c; }}

      .t-win {{ fill: #15803d; font-weight: 600; }}
      .t-note {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 500; fill: #64748b; }}

      .b-expanse {{ fill: #16a34a; }}
      .b-hf {{ fill: #2563eb; }}
      .b-baseline {{ fill: #64748b; }}
      .b-disqualified {{ fill: #ef4444; stroke: #b91c1c; stroke-width: 1px; stroke-dasharray: 2,2; fill-opacity: 0.15; }}

      .badge-win {{ fill: #dcfce7; stroke: #86efac; stroke-width: 1px; rx: 3px; }}
      .badge-win-text {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 9.5px; font-weight: 700; fill: #15803d; text-anchor: middle; }}

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
        .t-val-muted {{ fill: #cbd5e1; }}
        .t-val-warn {{ fill: #f87171; }}
        .t-win {{ fill: #4ade80; }}
        .t-note {{ fill: #94a3b8; }}
        .b-expanse {{ fill: #22c55e; }}
        .b-hf {{ fill: #3b82f6; }}
        .b-baseline {{ fill: #475569; }}
        .b-disqualified {{ fill: #ef4444; stroke: #f87171; fill-opacity: 0.25; }}
        .badge-win {{ fill: #064e3b; stroke: #059669; }}
        .badge-win-text {{ fill: #6ee7b7; }}
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
      :root[data-theme="dark"] .t-val-muted, [data-theme="dark"] .t-val-muted {{ fill: #cbd5e1; }}
      :root[data-theme="dark"] .t-val-warn, [data-theme="dark"] .t-val-warn {{ fill: #f87171; }}
      :root[data-theme="dark"] .t-win, [data-theme="dark"] .t-win {{ fill: #4ade80; }}
      :root[data-theme="dark"] .t-note, [data-theme="dark"] .t-note {{ fill: #94a3b8; }}
      :root[data-theme="dark"] .b-expanse, [data-theme="dark"] .b-expanse {{ fill: #22c55e; }}
      :root[data-theme="dark"] .b-hf, [data-theme="dark"] .b-hf {{ fill: #3b82f6; }}
      :root[data-theme="dark"] .b-baseline, [data-theme="dark"] .b-baseline {{ fill: #475569; }}
      :root[data-theme="dark"] .b-disqualified, [data-theme="dark"] .b-disqualified {{ fill: #ef4444; stroke: #f87171; fill-opacity: 0.25; }}
      :root[data-theme="dark"] .badge-win, [data-theme="dark"] .badge-win {{ fill: #064e3b; stroke: #059669; }}
      :root[data-theme="dark"] .badge-win-text, [data-theme="dark"] .badge-win-text {{ fill: #6ee7b7; }}
    </style>
  </defs>
  <rect width="100%" height="100%" class="bg" rx="8"/>
  <rect width="100%" height="100%" class="border" rx="8"/>
"""
