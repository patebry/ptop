#!/usr/bin/env python3
"""Render the synthetic README fixture through ptop into SVG (no Python dependencies).

Usage: python3 scripts/preview.py [path/to/ptop]
Optional PNG (development-only dependency):
uv run --with cairosvg==2.9.1 python -c 'import cairosvg; cairosvg.svg2png(url="docs/assets/ptop.svg", write_to="docs/assets/ptop.png")'
The input is ptop's capture sheet: CUP + SGR, each row starts in default style.
"""
import html
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
fixture = ROOT / 'docs/assets/demo.json'
settings = json.loads(fixture.read_text())
binary = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / 'target/release/ptop'
frame = subprocess.check_output([str(binary), '--capture', str(fixture)], text=True)
colors = ['#000000', '#cd0000', '#00cd00', '#cdcd00', '#0000ee', '#cd00cd', '#00cdcd', '#e5e5e5',
          '#7f7f7f', '#ff0000', '#00ff00', '#ffff00', '#5c5cff', '#ff00ff', '#00ffff', '#ffffff']
levels = [0, 95, 135, 175, 215, 255]
colors += ['#%02x%02x%02x' % (r, g, b) for r in levels for g in levels for b in levels]
colors += ['#%02x%02x%02x' % (v, v, v) for v in range(8, 239, 10)]
width, height = settings['cols'] * 10 + 24, settings['rows'] * 20 + 24
parts = ['<?xml version="1.0" encoding="UTF-8"?>', f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
         '<title>ptop — synthetic demonstration data</title>',
         '<rect width="100%" height="100%" fill="#000000"/>',
         '<g font-family="Menlo, monospace" font-size="16" font-style="normal">']
for row in frame.splitlines():
    fg, bg, bold = colors[7], colors[0], False
    x = y = 0
    for token in re.split(r'(\x1b\[[0-9;]*[Hm])', row):
        if token.startswith('\x1b['):
            codes = [int(n or 0) for n in token[2:-1].split(';')]
            if token.endswith('H'):
                y, x = codes[0] - 1, codes[1] - 1
                continue
            i = 0
            while i < len(codes):
                code = codes[i]
                if code == 0: fg, bg, bold = colors[7], colors[0], False
                elif code == 1: bold = True
                elif code == 22: bold = False
                elif code == 39: fg = colors[7]
                elif code == 49: bg = colors[0]
                elif 30 <= code <= 37: fg = colors[code - 30]
                elif 40 <= code <= 47: bg = colors[code - 40]
                elif code in (38, 48) and codes[i + 1] == 5:
                    if code == 38: fg = colors[codes[i + 2]]
                    else: bg = colors[codes[i + 2]]
                    i += 2
                else: raise ValueError(f'Unsupported capture style: {code}')
                i += 1
        else:
            assert '\x1b' not in token, 'Unexpected terminal escape'
            for char in token:
                px, py = 12 + x * 10, 12 + y * 20
                if bg != colors[0]: parts.append(f'<rect x="{px}" y="{py}" width="10" height="20" fill="{bg}"/>')
                if 0x2800 <= ord(char) <= 0x28ff:
                    # Render Unicode braille cells directly; portable across fonts.
                    for bit, (dx, dy) in enumerate([(3, 3), (3, 7), (3, 11), (7, 3), (7, 7), (7, 11), (3, 15), (7, 15)]):
                        if (ord(char) - 0x2800) & (1 << bit):
                            parts.append(f'<circle cx="{px + dx}" cy="{py + dy}" r="0.8" fill="{fg}"/>')
                elif char != ' ': parts.append(f'<text x="{px}" y="{py + 16}" fill="{fg}" font-weight="{700 if bold else 400}">{html.escape(char)}</text>')
                x += 1
parts.append('</g></svg>')
(ROOT / 'docs/assets/ptop.svg').write_text('\n'.join(parts) + '\n')
print('Rendered docs/assets/ptop.svg from synthetic fixture through ptop')
