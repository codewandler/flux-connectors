#!/usr/bin/env python3
"""Render a Flux-Lang snippet to a syntax-highlighted SVG.

GitHub's markdown has no tab syntax and strips <style> and <script>, so a highlighted
code block in a README has to be an image. SVG rather than PNG because it stays text —
diffable in review, crisp at any zoom, and a fraction of the size.

Colours are inline `fill` attributes on <tspan>, which survive GitHub's SVG sanitizer;
a <style> block would not.

    python3 scripts/flux-to-svg.py <input.flux> <out-light.svg> <out-dark.svg>

The output is a GENERATED ARTIFACT and nothing yet checks it for drift. The snippet in
`assets/readme-snippet.flux` is extracted verbatim from `connectors/zendesk.flux`, so when
that operation changes, re-extract it and re-run this script. A drift check belongs with the
provider-docs epic, which will generate far more of this.
"""

import re
import sys
from html import escape
from pathlib import Path

# Flux-Lang lexical classes. Ordered: the first pattern that matches at a position wins.
KEYWORDS = {
    "op", "flow", "return", "when", "unless", "retry", "backoff", "delay", "fallback",
    "timeout", "parallel", "branch", "match", "case", "default", "each", "repeat",
    "assert", "expose", "description", "risk", "idempotency", "effects", "limits",
    "view", "do", "with_tools", "budget", "scope", "saga", "once", "checkpoint",
}
TYPES = {"Number", "String", "Bool", "Any", "List", "Ctx", "Claim", "Evidence"}
LITERALS = {"true", "false", "null"}

THEMES = {
    # GitHub's own light/dark code colours, so the image sits naturally in the page.
    "light": {
        "bg": "#ffffff", "fg": "#1f2328", "comment": "#59636e", "keyword": "#cf222e",
        "string": "#0a3069", "var": "#8250df", "type": "#953800", "fn": "#6639ba",
        "punct": "#1f2328", "literal": "#0550ae", "border": "#d1d9e0",
    },
    "dark": {
        "bg": "#0d1117", "fg": "#e6edf3", "comment": "#9198a1", "keyword": "#ff7b72",
        "string": "#a5d6ff", "var": "#d2a8ff", "type": "#ffa657", "fn": "#d2a8ff",
        "punct": "#e6edf3", "literal": "#79c0ff", "border": "#3d444d",
    },
}

TOKEN_RE = re.compile(
    r"""
    (?P<comment>\#[^\n]*)
  | (?P<string>"(?:[^"\\]|\\.)*")
  | (?P<var>\$[A-Za-z_][A-Za-z0-9_]*)
  | (?P<fn>[A-Za-z_][A-Za-z0-9_.\-]*(?=\())
  | (?P<word>[A-Za-z_][A-Za-z0-9_\-]*)
  | (?P<num>\d+(?:\.\d+)?)
  | (?P<space>[ \t]+)
  | (?P<other>.)
    """,
    re.VERBOSE,
)

FONT = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace"
FONT_SIZE = 13.0
CHAR_W = FONT_SIZE * 0.6009  # advance width of the monospace stack at this size
LINE_H = 20.0
PAD_X, PAD_Y = 16.0, 14.0


def classify(kind: str, text: str) -> str:
    if kind == "word":
        if text in KEYWORDS:
            return "keyword"
        if text in TYPES:
            return "type"
        if text in LITERALS:
            return "literal"
        return "fg"
    if kind == "num":
        return "literal"
    if kind == "other":
        return "punct"
    if kind == "space":
        return "fg"
    return kind


def render(source: str, theme: dict) -> str:
    lines = source.rstrip("\n").split("\n")
    width = PAD_X * 2 + max((len(line) for line in lines), default=0) * CHAR_W
    height = PAD_Y * 2 + len(lines) * LINE_H

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0f}" height="{height:.0f}" '
        f'viewBox="0 0 {width:.0f} {height:.0f}" role="img" aria-label="Generated Flux-Lang source">',
        f'<rect width="{width:.0f}" height="{height:.0f}" rx="6" fill="{theme["bg"]}" '
        f'stroke="{theme["border"]}"/>',
        f'<text font-family="{escape(FONT)}" font-size="{FONT_SIZE}" xml:space="preserve">',
    ]

    for row, line in enumerate(lines):
        y = PAD_Y + LINE_H * row + FONT_SIZE
        out.append(f'<tspan x="{PAD_X:.0f}" y="{y:.1f}">')
        for m in TOKEN_RE.finditer(line):
            kind = m.lastgroup
            text = m.group()
            colour = theme[classify(kind, text)] if classify(kind, text) != "fg" else theme["fg"]
            out.append(f'<tspan fill="{colour}">{escape(text)}</tspan>')
        out.append("</tspan>")

    out.append("</text></svg>")
    return "".join(out) + "\n"


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__, file=sys.stderr)
        return 2
    source = Path(sys.argv[1]).read_text()
    for path, name in ((sys.argv[2], "light"), (sys.argv[3], "dark")):
        Path(path).write_text(render(source, THEMES[name]))
        print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
