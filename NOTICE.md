# Cirdean notices

Cirdean is an independent Rust implementation informed by publicly available open-source document scanning projects.

## Upstream research

- **Hbot** (`abhishekdhital/hbot`), MIT licensed. Studied for multi-path contour detection, stable-frame auto-capture, page-turn re-arming, illumination normalization, and gutter-aware book scanning. Upstream license: `Copyright (c) 2026 KibuSpace`.
- **Camscan** (`suhren/camscan`), MIT licensed. Studied for camera abstraction, Hough-line detection, line intersections, graph-based quadrilateral construction, candidate scoring, and detector fixture tests. Upstream license: `Copyright (c) 2023 Adam Suhren Gustafsson`.

Cirdean currently re-implements these algorithmic ideas independently rather than copying upstream source files verbatim. If future work ports or adapts a substantial portion of upstream source, the applicable upstream copyright and MIT permission notice must be preserved with that derived material.

See `docs/upstream.md` and `docs/upstream-algorithm-study.md` for the engineering mapping used by Cirdean.
