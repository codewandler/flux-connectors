# Brand assets

| File | Use |
|---|---|
| `banner.svg` | README header, docs hero. 1200×300. |
| `icon.svg` | App/favicon mark on a dark tile. 64×64, legible down to 16px. |
| `mark.svg` | The glyph alone, no tile — for light surfaces where a dark square would fight the page. |
| `icon-<n>.png` | Rasterised favicons: 16, 32, 48, 64, 128, 180 (Apple touch), 256, 512. |
| `social-preview.png` | GitHub *Settings → Social preview*, 1280×320. |
| `banner@2x.png` | 2400×600, for anywhere SVG is not accepted. |

## The mark

Three nodes converging into one. That is the product argument as a shape: **many vendor APIs, one
compiled language** — not decoration, and the reason the output node is solid while the inputs are
outlines. One artifact comes out, not three.

It is stroke-only and widely spaced so it survives at 16px, where a more detailed mark would mush.

## Palette

| Token | Hex | Role |
|---|---|---|
| Flow start | `#7C5CFF` | violet — the provider end of the gradient |
| Flow end | `#22D3EE` | cyan — the compiled end, and the solid output node |
| Tile | `#0F1629` | deep navy; reads on a light *or* dark page, so no light/dark variants are needed |
| Body text | `#94A3B8` | slate |

The gradient runs violet → cyan in the direction of the flow, so the mark reads left-to-right as a
transformation.

## Rules

- **SVG is the source.** The PNGs are generated:
  ```bash
  cd assets/brand
  for s in 16 32 48 64 128 180 256 512; do rsvg-convert -w $s -h $s icon.svg -o icon-$s.png; done
  rsvg-convert -w 1280 -h 320 banner.svg -o social-preview.png
  rsvg-convert -w 2400 -h 600 banner.svg -o banner@2x.png
  ```
  Edit the SVG and re-run; never touch a PNG by hand.
- **No `<style>` or `<script>` in the SVGs.** GitHub's sanitizer strips both, so all colour is inline
  attributes and gradient `<defs>`. Keep it that way or the marks render black.
- The dark tile is deliberate: one asset that works on both themes beats two that drift.

## Not covered

Vendor logos (Zendesk, Freshdesk, babelforce) are **not** here and are not ours to ship — that is the
trademark question parked in [C-40](../../docs/stories/C-40-provider-icons.md). These assets are for
flux-connectors itself only.
