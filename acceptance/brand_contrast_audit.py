#!/usr/bin/env python3
"""Brand palette contrast audit and review-sheet generator (EPIC-XF-10-S09).

Two jobs, one source of truth:

  1. **Audit** — check every foreground/background pair the console actually
     composes against WCAG 2.1 AA, and report failures with the selector that
     produces them.
  2. **Review sheet** — emit a single self-contained HTML file that renders the
     palette as swatches plus a realistic mock of the console, so the tenant
     can approve their identity by looking at it rather than by reading hex
     codes.

Both read the palette **out of `backoffice/src/theme.ts`**, so the sheet sent
for sign-off is always the palette that is actually in the code. Hardcoding a
copy here would let the two drift, and the whole point of the exercise is that
what gets signed is what ships.

**There is no accessibility requirement in this project.** `requirements.md`
contains no WCAG or contrast clause, so a failure reported here is *not* a
requirement violation -- it is a quality finding offered to whoever signs, so
that the decision is taken knowingly. Do not let this script's output be
reported as a failed requirement.

stdlib only.

    python3 acceptance/brand_contrast_audit.py
    python3 acceptance/brand_contrast_audit.py --html docs/brand/review.html
"""
import argparse, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
THEME_TS = os.path.join(REPO, "backoffice", "src", "theme.ts")


# ----------------------------------------------------------------- palette
def load_palette(path=THEME_TS):
    """Parse `transmegaTheme`'s variables block out of theme.ts.

    Deliberately scoped to that one object: theme.ts also lists every variable
    name in a TypeScript union type, and a looser regex would happily collect
    those and report a palette that does not exist.
    """
    src = open(path, encoding="utf-8").read()
    start = src.find("export const transmegaTheme")
    if start == -1:
        raise SystemExit("transmegaTheme not found in {}".format(path))
    block = src[start:src.find("}", src.find("variables: {", start))]
    pairs = re.findall(r"'(--[a-z-]+)':\s*'(#[0-9a-fA-F]{6})'", block)
    if not pairs:
        raise SystemExit("no colour pairs parsed from transmegaTheme")
    return dict(pairs)


def is_draft(path=THEME_TS):
    """Is the palette still marked as unsigned in the source?"""
    src = open(path, encoding="utf-8").read().upper()
    return "DRAFT" in src


# ----------------------------------------------------------------- contrast
def _srgb(c):
    c /= 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def luminance(hex_colour):
    h = hex_colour.lstrip("#")
    r, g, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
    return 0.2126 * _srgb(r) + 0.7152 * _srgb(g) + 0.0722 * _srgb(b)


def contrast(fg, bg):
    a, b = luminance(fg), luminance(bg)
    return (max(a, b) + 0.05) / (min(a, b) + 0.05)


# (label, fg, bg, selector, kind) -- kind sets the AA threshold.
# Gradients are checked at BOTH ends: a label that is legible over the dark end
# of `linear-gradient(--panel-from, --panel-to)` can still be illegible over
# the light end, and the user sees both.
PAIRS = [
    ("Primary button label", "#ffffff", "--accent-primary", ".btn.primary (12.5px upper, 600)", "normal"),
    ("Primary button label (gradient end)", "#ffffff", "--accent-hover", ".btn.primary", "normal"),
    ("Secondary button label", "--accent-primary", "#ffffff", ".btn.secondary", "normal"),
    ("Tertiary button label", "--accent-primary", "--tint-accent", ".btn.tertiary", "normal"),
    ("Active tab label", "--accent-primary", "#ffffff", ".tab.active", "normal"),
    ("Language switcher", "--accent-primary", "#ffffff", ".auth-language", "normal"),
    ("Body text on app background", "--text-body", "--bg-primary", "body", "normal"),
    ("Headings on app background", "--text-heading", "--bg-primary", "h1-h3", "normal"),
    ("Body text on white cards", "--text-body", "#ffffff", ".modal, .auth-card", "normal"),
    ("Metric value", "--text-heading", "#ffffff", ".metric-card strong", "normal"),
    ("Metric icon glyph", "--accent-primary", "--tint-accent-strong", ".metric-icon.blue", "ui"),
    ("Sidebar text", "--sidebar-text", "--sidebar-from", ".sidebar", "normal"),
    ("Sidebar link", "--sidebar-link", "--sidebar-from", ".sidebar a", "normal"),
    ("Sidebar link (gradient end)", "--sidebar-link", "--sidebar-to", ".sidebar a", "normal"),
    ("Welcome panel body", "--panel-text", "--panel-from", ".welcome-panel", "normal"),
    ("Welcome panel body (gradient end)", "--panel-text", "--panel-to", ".welcome-panel", "normal"),
    ("Welcome panel heading", "#ffffff", "--panel-to", ".welcome-panel h2 (32px)", "large"),
    ("Auth panel body", "--auth-panel-text", "--auth-panel-from", ".auth-panel", "normal"),
    ("Auth panel body (gradient end)", "--auth-panel-text", "--auth-panel-to", ".auth-panel", "normal"),
    ("Auth headline emphasis", "--auth-panel-highlight", "--auth-panel-to", ".auth-copy h1 em", "large"),
    ("Auth brand mark", "--accent-secondary", "--auth-panel-from", ".auth-brand span", "ui"),
    ("Input border", "--border-color", "#ffffff", ".form-grid input", "ui"),
]

THRESHOLD = {"normal": 4.5, "large": 3.0, "ui": 3.0}


def audit(palette):
    rows = []
    for label, fg, bg, selector, kind in PAIRS:
        f = palette.get(fg, fg) if fg.startswith("--") else fg
        b = palette.get(bg, bg) if bg.startswith("--") else bg
        if not (f.startswith("#") and b.startswith("#")):
            continue
        r = contrast(f, b)
        need = THRESHOLD[kind]
        rows.append({"label": label, "fg": f, "bg": b, "selector": selector,
                     "kind": kind, "ratio": r, "need": need, "ok": r >= need})
    return rows


def suggest(palette):
    """Darker variants of the accent that would carry white text at AA."""
    accent = palette.get("--accent-primary")
    if not accent or contrast("#ffffff", accent) >= 4.5:
        return []
    h = accent.lstrip("#")
    r, g, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
    out = []
    for pct in range(5, 65, 5):
        f = 1 - pct / 100.0
        cand = "#{:02x}{:02x}{:02x}".format(int(r * f), int(g * f), int(b * f))
        if contrast("#ffffff", cand) >= 4.5:
            out.append((cand, contrast("#ffffff", cand), pct))
            if len(out) >= 3:
                break
    return out


# -------------------------------------------------------------- review sheet
def render_html(palette, rows, draft):
    css_vars = "\n".join("      {}: {};".format(k, v) for k, v in sorted(palette.items()))
    swatches = "\n".join(
        '<figure class="sw"><div class="chip" style="background:{v}"></div>'
        '<figcaption><b>{k}</b><code>{v}</code></figcaption></figure>'.format(k=k, v=v)
        for k, v in sorted(palette.items()))

    fails = [r for r in rows if not r["ok"]]
    rowhtml = "\n".join(
        '<tr class="{cls}"><td>{label}</td><td><span class="dot" style="background:{fg}"></span>'
        '<code>{fg}</code></td><td><span class="dot" style="background:{bg}"></span><code>{bg}</code>'
        '</td><td class="num">{ratio:.2f}:1</td><td class="num">{need:.1f}:1</td>'
        '<td>{verdict}</td><td><code class="sel">{selector}</code></td></tr>'.format(
            cls="" if r["ok"] else "bad", label=r["label"], fg=r["fg"], bg=r["bg"],
            ratio=r["ratio"], need=r["need"],
            verdict="PASS" if r["ok"] else "FAILS AA", selector=r["selector"])
        for r in rows)

    banner = ""
    if draft:
        banner = ('<div class="banner draft"><b>DRAFT — NOT APPROVED.</b> These colours were '
                  'inferred from a read-only survey of the Operação TRM web app. They have not been '
                  'confirmed by Transmega. This sheet exists to get that confirmation.</div>')
    if fails:
        banner += ('<div class="banner warn"><b>{n} of {t} colour pairs fall below WCAG 2.1 AA.</b> '
                   'This project has no accessibility requirement, so this is advice rather than a '
                   'defect — but it affects the most-used control in the console, so it should be '
                   'decided deliberately. See the table at the bottom.</div>').format(
                       n=len(fails), t=len(rows))

    return TEMPLATE.format(css_vars=css_vars, swatches=swatches, rows=rowhtml, banner=banner)


TEMPLATE = """<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Transmega — console identity review</title>
<style>
  :root {{
{css_vars}
    --page:#eef0f3;
  }}
  * {{ box-sizing:border-box; }}
  body {{ margin:0; font:15px/1.5 system-ui,-apple-system,Segoe UI,Roboto,sans-serif;
         background:var(--page); color:var(--text-heading); }}
  .wrap {{ max-width:1080px; margin:0 auto; padding:32px 20px 72px; }}
  h1 {{ font-size:1.75rem; margin:0 0 4px; }}
  .sub {{ color:#5f6368; margin:0 0 24px; }}
  .banner {{ padding:14px 16px; border-radius:10px; margin:0 0 14px; font-size:.92rem; }}
  .banner.draft {{ background:#fff8e1; border:1px solid #f0c36d; color:#6b4e00; }}
  .banner.warn {{ background:#fdecea; border:1px solid #e5928a; color:#7b241c; }}
  h2 {{ font-size:1.05rem; margin:34px 0 12px; text-transform:uppercase;
        letter-spacing:.08em; color:#5f6368; }}
  .swatches {{ display:grid; grid-template-columns:repeat(auto-fill,minmax(190px,1fr)); gap:12px; }}
  .sw {{ margin:0; background:#fff; border:1px solid #dfe3e8; border-radius:10px; overflow:hidden; }}
  .chip {{ height:62px; }}
  .sw figcaption {{ padding:8px 10px; font-size:.75rem; }}
  .sw b {{ display:block; font-weight:600; }}
  .sw code {{ color:#5f6368; }}

  /* ---- console mock: uses the palette variables exactly as main.scss does ---- */
  .mock {{ border:1px solid #d5d9df; border-radius:14px; overflow:hidden;
           box-shadow:0 10px 30px rgba(0,0,0,.09); background:var(--bg-primary); }}
  .mockbar {{ background:#e9ecf1; padding:7px 12px; font-size:.72rem; color:#5f6368;
              border-bottom:1px solid #d5d9df; }}
  .app {{ display:grid; grid-template-columns:212px 1fr; min-height:430px; }}
  .side {{ background:linear-gradient(180deg,var(--sidebar-from),var(--sidebar-to));
           color:var(--sidebar-text); padding:18px 14px; }}
  .brand {{ display:flex; align-items:center; gap:9px; font-weight:700; margin-bottom:22px; }}
  .brand i {{ width:30px; height:30px; border-radius:9px; display:block;
              background:linear-gradient(135deg,var(--accent-primary),var(--brand-mark-to)); }}
  .side a {{ display:block; color:var(--sidebar-link); text-decoration:none; padding:8px 10px;
             border-radius:7px; font-size:.86rem; }}
  .side a.on {{ background:rgba(255,255,255,.1); color:var(--sidebar-text); font-weight:600; }}
  .main {{ padding:22px; }}
  .btn {{ min-height:40px; padding:0 17px; border-radius:999px; border:1px solid transparent;
          display:inline-flex; align-items:center; font-size:.78rem; font-weight:600;
          letter-spacing:.07em; text-transform:uppercase; cursor:default; }}
  .btn.primary {{ color:#fff; background:linear-gradient(135deg,var(--accent-primary),var(--accent-hover)); }}
  .btn.secondary {{ color:var(--accent-primary); background:#fff; border-color:var(--border-color); }}
  .btn.tertiary {{ color:var(--accent-primary); background:var(--tint-accent); min-height:32px; }}
  .cards {{ display:grid; grid-template-columns:repeat(3,1fr); gap:14px; margin:18px 0; }}
  .card {{ background:#fff; border:1px solid var(--border-color); border-radius:14px; padding:16px;
           display:flex; gap:12px; align-items:center; }}
  .ico {{ width:44px; height:44px; border-radius:12px; display:grid; place-items:center;
          font-weight:700; color:var(--accent-primary); background:var(--tint-accent-strong); }}
  .card small {{ display:block; color:var(--text-body); font-size:.74rem; }}
  .card b {{ font-size:1.45rem; }}
  .panel {{ margin-top:6px; padding:26px; border-radius:16px; color:var(--panel-text);
            background:linear-gradient(130deg,var(--panel-from),var(--panel-to)); }}
  .panel h3 {{ color:#fff; margin:6px 0; font-size:1.4rem; }}
  .panel p {{ margin:0; font-size:.9rem; }}
  .tabs {{ display:flex; gap:4px; border-bottom:1px solid var(--border-color); margin:20px 0 14px; }}
  .tab {{ padding:8px 14px; font-size:.86rem; color:var(--text-body);
          border-bottom:2px solid transparent; }}
  .tab.active {{ color:var(--accent-primary); border-bottom-color:var(--accent-primary);
                 font-weight:600; }}

  .login {{ display:grid; grid-template-columns:46% 1fr; min-height:400px; }}
  .lpanel {{ padding:34px; color:var(--auth-panel-text);
             background:linear-gradient(150deg,var(--auth-panel-from),var(--auth-panel-to)); }}
  .lpanel h1 {{ color:#fff; font-size:2.1rem; line-height:1.08; margin:14px 0; }}
  .lpanel h1 em {{ color:var(--auth-panel-highlight); font-style:normal; }}
  .lform {{ background:#fff; display:grid; place-items:center; padding:30px; }}
  .lcard {{ width:100%; max-width:330px; }}
  .lcard h2 {{ margin:10px 0 18px; font-size:1.4rem; }}
  .lcard label {{ display:block; font-size:.8rem; margin:12px 0 5px; }}
  .lcard input {{ width:100%; min-height:42px; padding:0 11px; border-radius:7px;
                  border:1px solid var(--border-color); background:var(--bg-primary); }}
  .lcard .btn {{ width:100%; justify-content:center; margin-top:18px; }}
  .aicon {{ width:46px; height:46px; border-radius:13px; display:grid; place-items:center;
            color:var(--accent-primary); background:var(--tint-accent); font-weight:700; }}

  table {{ width:100%; border-collapse:collapse; background:#fff; border-radius:10px;
           overflow:hidden; font-size:.84rem; }}
  th,td {{ text-align:left; padding:9px 11px; border-bottom:1px solid #e8ebef; }}
  th {{ background:#f4f6f8; font-size:.74rem; text-transform:uppercase; letter-spacing:.06em;
        color:#5f6368; }}
  tr.bad td {{ background:#fdf3f2; }}
  tr.bad td:nth-child(6) {{ color:#b3261e; font-weight:700; }}
  .num {{ text-align:right; font-variant-numeric:tabular-nums; }}
  .dot {{ display:inline-block; width:11px; height:11px; border-radius:3px; margin-right:5px;
          border:1px solid rgba(0,0,0,.15); vertical-align:-1px; }}
  code {{ font-family:ui-monospace,SFMono-Regular,Menlo,monospace; font-size:.9em; }}
  code.sel {{ color:#5f6368; }}
  .sign {{ margin-top:34px; background:#fff; border:1px solid #dfe3e8; border-radius:12px;
           padding:20px 22px; }}
  .sign h2 {{ margin-top:0; }}
  .sign li {{ margin:7px 0; }}
</style></head><body><div class="wrap">

<h1>Transmega — console identity review</h1>
<p class="sub">How the hermes backoffice looks with the palette currently in the code.
Everything below is rendered from those exact colour values.</p>

{banner}

<h2>1. Sign-in screen</h2>
<div class="mock"><div class="mockbar">hermes — sign in</div>
<div class="login">
  <div class="lpanel">
    <div style="display:flex;align-items:center;gap:9px;color:#fff;font-weight:700">
      <i style="width:34px;height:34px;border-radius:10px;display:block;
                background:linear-gradient(135deg,var(--accent-primary),var(--brand-mark-to))"></i>
      hermes</div>
    <h1>Your fleet,<br><em>under control</em></h1>
    <p>Operations, vehicles and people in one place.</p>
  </div>
  <div class="lform"><div class="lcard">
    <div class="aicon">&#9679;</div>
    <h2>Sign in</h2>
    <label>E-mail</label><input value="operacoes@transmega.com.br" readonly>
    <label>Password</label><input type="password" value="........" readonly>
    <span class="btn primary">Sign in</span>
  </div></div>
</div></div>

<h2>2. Dashboard</h2>
<div class="mock"><div class="mockbar">hermes — dashboard</div>
<div class="app">
  <nav class="side">
    <div class="brand"><i></i> hermes</div>
    <a class="on">Dashboard</a><a>Vehicles</a><a>Drivers</a><a>Tenants</a>
    <a>Business plans</a><a>Settings</a>
  </nav>
  <div class="main">
    <div style="display:flex;justify-content:space-between;align-items:center">
      <b style="font-size:1.2rem">Good morning, Transmega</b>
      <span class="btn primary">New vehicle</span>
    </div>
    <div class="tabs"><span class="tab active">Overview</span>
      <span class="tab">Activity</span><span class="tab">Reports</span></div>
    <div class="cards">
      <div class="card"><div class="ico">V</div><div><small>Vehicles</small><b>128</b></div></div>
      <div class="card"><div class="ico">D</div><div><small>Drivers</small><b>96</b></div></div>
      <div class="card"><div class="ico">R</div><div><small>Routes today</small><b>41</b></div></div>
    </div>
    <div class="panel">
      <h3>Fleet is operating normally</h3>
      <p>All vehicles reporting. Last position update 2 minutes ago.</p>
    </div>
    <div style="margin-top:16px;display:flex;gap:8px">
      <span class="btn secondary">Export</span>
      <span class="btn tertiary">Filter</span>
    </div>
  </div>
</div></div>

<h2>3. The palette</h2>
<div class="swatches">{swatches}</div>

<h2>4. Legibility check (WCAG 2.1 AA)</h2>
<table>
<thead><tr><th>Pair</th><th>Foreground</th><th>Background</th><th class="num">Ratio</th>
<th class="num">Needs</th><th>Verdict</th><th>Where</th></tr></thead>
<tbody>{rows}</tbody></table>

<div class="sign">
<h2>What we need from Transmega</h2>
<ol>
<li><b>Are these your colours?</b> If not, please send the official brand values
    (hex codes, or a brand manual / logo files we can take them from).</li>
<li><b>Is the sign-in screen acceptable</b> as the first thing your team sees each day?</li>
<li><b>Who approves this?</b> We need a name and a date to record against the release.</li>
</ol>
<p style="margin-bottom:0"><b>Approved by:</b> ______________________  &nbsp;
<b>Role:</b> ______________________ &nbsp; <b>Date:</b> ____________</p>
</div>

</div></body></html>
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--html", help="also write the review sheet to this path")
    ap.add_argument("--strict", action="store_true",
                    help="exit non-zero if any pair fails AA (off by default: "
                         "this project has no accessibility requirement)")
    a = ap.parse_args()

    palette = load_palette()
    draft = is_draft()
    rows = audit(palette)
    fails = [r for r in rows if not r["ok"]]

    print("Transmega palette — {} colours, parsed from backoffice/src/theme.ts".format(len(palette)))
    print("Status in source: {}\n".format("DRAFT (not signed off)" if draft else "no DRAFT marker"))

    width = max(len(r["label"]) for r in rows)
    for r in rows:
        print("  {:<{w}}  {} on {}  {:>6.2f}:1  needs {:.1f}  {}".format(
            r["label"], r["fg"], r["bg"], r["ratio"], r["need"],
            "ok" if r["ok"] else "** FAILS AA **", w=width))

    print("\n{} of {} pairs meet WCAG 2.1 AA.".format(len(rows) - len(fails), len(rows)))

    if fails:
        print("\nThis project has NO accessibility requirement, so the above is a quality")
        print("finding, not a requirement violation. It is reported because it affects the")
        print("console's most-used control and should be decided knowingly.\n")
        for cand, ratio_, pct in suggest(palette):
            print("  A {}% darker accent ({}) would carry white text at {:.2f}:1.".format(
                pct, cand, ratio_))

    if a.html:
        target = a.html if os.path.isabs(a.html) else os.path.join(REPO, a.html)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        with open(target, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(render_html(palette, rows, draft))
        print("\nReview sheet written to {}".format(os.path.relpath(target, REPO)))

    return 1 if (fails and a.strict) else 0


if __name__ == "__main__":
    sys.exit(main())
