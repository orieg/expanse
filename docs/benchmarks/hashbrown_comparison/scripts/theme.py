"""
docs/benchmarks/hashbrown_comparison/scripts/theme.py

Shared reusable styling and SVG template generator adhering to the canonical design system:
- CSS variables with robust standalone fallbacks: var(--surface, #ffffff), var(--ink, #0f172a), etc.
- Dark theme support for GitHub dark mode, Hugo theme toggles, and system prefers-color-scheme.
- Standardized color palette:
    - Expanse: #16a34a (light) / #22c55e (dark) / #4ade80 (accent text)
    - Hashbrown (SwissTable): #2563eb (light) / #3b82f6 (dark) / #38bdf8 (blue text)
    - BTreeMap: #64748b (light) / #475569 (dark) / #94a3b8 (muted text)
    - Disqualified / Warn: #ef4444 (light) / #f87171 (dark)
- Typography: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif
"""

def svg_header(width: int = 960, height: int = 300, title: str = "Benchmark Chart") -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" role="img" viewBox="0 0 {width} {height}" width="100%" height="100%">
  <title>{title}</title>
  <defs>
    <style>
      /* Light theme (default with CSS var fallbacks) */
      .bg {{ fill: var(--surface, #ffffff); }}
      .border {{ stroke: var(--line, #e2e8f0); stroke-width: 1px; fill: none; }}
      .grid {{ stroke: var(--line-soft, #f1f5f9); stroke-width: 1px; stroke-dasharray: 2,3; }}
      .axis {{ stroke: var(--line, #cbd5e1); stroke-width: 1.5px; }}
      .divider {{ stroke: var(--line, #e2e8f0); stroke-width: 1px; }}
      
      .t-title {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11.5px; font-weight: 700; letter-spacing: 0.6px; fill: var(--ink, #0f172a); text-transform: uppercase; }}
      .t-sub {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10px; font-weight: 500; fill: var(--ink-soft, #475569); }}
      .t-unit {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 600; fill: var(--ink-soft, #475569); }}
      .t-axis-label {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9.5px; font-weight: 500; fill: var(--ink-soft, #64748b); }}
      .t-bar-label {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 11px; font-weight: 700; fill: var(--ink, #0f172a); }}
      .t-legend {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 10.5px; font-weight: 600; fill: var(--ink, #0f172a); }}
      
      .t-val-accent {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 700; fill: #15803d; text-anchor: middle; }}
      .t-val-blue {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 600; fill: #2563eb; text-anchor: middle; }}
      .t-val-muted {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; font-weight: 600; fill: var(--ink-soft, #475569); text-anchor: middle; }}
      .t-val-warn {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; font-weight: 700; fill: #b91c1c; }}

      .t-win {{ fill: #15803d; font-weight: 600; }}
      .t-note {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; font-size: 9.5px; font-weight: 500; fill: var(--ink-soft, #64748b); }}

      .b-expanse {{ fill: #16a34a; }}
      .b-hashbrown {{ fill: #2563eb; }}
      .b-btree {{ fill: #64748b; }}
      .b-disqualified {{ fill: #ef4444; stroke: #b91c1c; stroke-width: 1px; stroke-dasharray: 2,2; fill-opacity: 0.15; }}

      .badge-win {{ fill: #dcfce7; stroke: #86efac; stroke-width: 1px; rx: 3px; }}
      .badge-win-text {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9.5px; font-weight: 700; fill: #15803d; text-anchor: middle; }}

      .badge-disq {{ fill: #fee2e2; stroke: #fca5a5; stroke-width: 1px; rx: 3px; }}
      .badge-disq-text {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 9.5px; font-weight: 700; fill: #991b1b; text-anchor: middle; }}

      /* Dark theme overrides */
      @media (prefers-color-scheme: dark) {{
        :root:not(:has(input.theme.light:checked)) .bg {{ fill: #0d1117; }}
        :root:not(:has(input.theme.light:checked)) .border {{ stroke: #30363d; }}
        :root:not(:has(input.theme.light:checked)) .grid {{ stroke: #21262d; }}
        :root:not(:has(input.theme.light:checked)) .axis {{ stroke: #484f58; }}
        :root:not(:has(input.theme.light:checked)) .divider {{ stroke: #21262d; }}
        :root:not(:has(input.theme.light:checked)) .t-title {{ fill: #f0f6fc; }}
        :root:not(:has(input.theme.light:checked)) .t-sub {{ fill: #94a3b8; }}
        :root:not(:has(input.theme.light:checked)) .t-unit {{ fill: #94a3b8; }}
        :root:not(:has(input.theme.light:checked)) .t-axis-label {{ fill: #94a3b8; }}
        :root:not(:has(input.theme.light:checked)) .t-bar-label {{ fill: #f8fafc; }}
        :root:not(:has(input.theme.light:checked)) .t-legend {{ fill: #f8fafc; }}
        :root:not(:has(input.theme.light:checked)) .t-val-accent {{ fill: #4ade80; }}
        :root:not(:has(input.theme.light:checked)) .t-val-blue {{ fill: #38bdf8; }}
        :root:not(:has(input.theme.light:checked)) .t-val-muted {{ fill: #cbd5e1; }}
        :root:not(:has(input.theme.light:checked)) .t-val-warn {{ fill: #f87171; }}
        :root:not(:has(input.theme.light:checked)) .t-win {{ fill: #4ade80; }}
        :root:not(:has(input.theme.light:checked)) .t-note {{ fill: #94a3b8; }}
        :root:not(:has(input.theme.light:checked)) .b-expanse {{ fill: #22c55e; }}
        :root:not(:has(input.theme.light:checked)) .b-hashbrown {{ fill: #3b82f6; }}
        :root:not(:has(input.theme.light:checked)) .b-btree {{ fill: #475569; }}
        :root:not(:has(input.theme.light:checked)) .b-disqualified {{ fill: #ef4444; stroke: #f87171; fill-opacity: 0.25; }}
        :root:not(:has(input.theme.light:checked)) .badge-win {{ fill: #064e3b; stroke: #059669; }}
        :root:not(:has(input.theme.light:checked)) .badge-win-text {{ fill: #6ee7b7; }}
        :root:not(:has(input.theme.light:checked)) .badge-disq {{ fill: #450a0a; stroke: #dc2626; }}
        :root:not(:has(input.theme.light:checked)) .badge-disq-text {{ fill: #fca5a5; }}
      }}

      :root:has(input.theme.dark:checked) .bg, [data-theme="dark"] .bg {{ fill: #0d1117; }}
      :root:has(input.theme.dark:checked) .border, [data-theme="dark"] .border {{ stroke: #30363d; }}
      :root:has(input.theme.dark:checked) .grid, [data-theme="dark"] .grid {{ stroke: #21262d; }}
      :root:has(input.theme.dark:checked) .axis, [data-theme="dark"] .axis {{ stroke: #484f58; }}
      :root:has(input.theme.dark:checked) .divider, [data-theme="dark"] .divider {{ stroke: #21262d; }}
      :root:has(input.theme.dark:checked) .t-title, [data-theme="dark"] .t-title {{ fill: #f0f6fc; }}
      :root:has(input.theme.dark:checked) .t-sub, [data-theme="dark"] .t-sub {{ fill: #94a3b8; }}
      :root:has(input.theme.dark:checked) .t-unit, [data-theme="dark"] .t-unit {{ fill: #94a3b8; }}
      :root:has(input.theme.dark:checked) .t-axis-label, [data-theme="dark"] .t-axis-label {{ fill: #94a3b8; }}
      :root:has(input.theme.dark:checked) .t-bar-label, [data-theme="dark"] .t-bar-label {{ fill: #f8fafc; }}
      :root:has(input.theme.dark:checked) .t-legend, [data-theme="dark"] .t-legend {{ fill: #f8fafc; }}
      :root:has(input.theme.dark:checked) .t-val-accent, [data-theme="dark"] .t-val-accent {{ fill: #4ade80; }}
      :root:has(input.theme.dark:checked) .t-val-blue, [data-theme="dark"] .t-val-blue {{ fill: #38bdf8; }}
      :root:has(input.theme.dark:checked) .t-val-muted, [data-theme="dark"] .t-val-muted {{ fill: #cbd5e1; }}
      :root:has(input.theme.dark:checked) .t-val-warn, [data-theme="dark"] .t-val-warn {{ fill: #f87171; }}
      :root:has(input.theme.dark:checked) .t-win, [data-theme="dark"] .t-win {{ fill: #4ade80; }}
      :root:has(input.theme.dark:checked) .t-note, [data-theme="dark"] .t-note {{ fill: #94a3b8; }}
      :root:has(input.theme.dark:checked) .b-expanse, [data-theme="dark"] .b-expanse {{ fill: #22c55e; }}
      :root:has(input.theme.dark:checked) .b-hashbrown, [data-theme="dark"] .b-hashbrown {{ fill: #3b82f6; }}
      :root:has(input.theme.dark:checked) .b-btree, [data-theme="dark"] .b-btree {{ fill: #475569; }}
      :root:has(input.theme.dark:checked) .b-disqualified, [data-theme="dark"] .b-disqualified {{ fill: #ef4444; stroke: #f87171; fill-opacity: 0.25; }}
      :root:has(input.theme.dark:checked) .badge-win, [data-theme="dark"] .badge-win {{ fill: #064e3b; stroke: #059669; }}
      :root:has(input.theme.dark:checked) .badge-win-text, [data-theme="dark"] .badge-win-text {{ fill: #6ee7b7; }}
      :root:has(input.theme.dark:checked) .badge-disq, [data-theme="dark"] .badge-disq {{ fill: #450a0a; stroke: #dc2626; }}
      :root:has(input.theme.dark:checked) .badge-disq-text, [data-theme="dark"] .badge-disq-text {{ fill: #fca5a5; }}
    </style>
  </defs>
  <rect width="100%" height="100%" class="bg" rx="8"/>
  <rect width="100%" height="100%" class="border" rx="8"/>
"""
