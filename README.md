# Cirdean

Cirdean is a camera-native document acquisition and scanning system written in Rust.

The project combines fast contour-based document detection, a bounded Hough-line fallback, confidence-aware detector fusion, temporal capture logic, book geometry, and eventually full-resolution correction/enhancement without tying the core engine to a specific GUI.

## Status

Cirdean is in early detection-foundation development. The `research/detection-foundation` branch now contains a working pure-Rust vision foundation rather than architecture placeholders only.

Implemented today:

- dependency-light `cirdean-core` geometry and detector contracts;
- confidence semantics with optional temporal/cross-detector evidence;
- confidence-weighted contour/Hough fusion;
- explicit auto-capture state machine with page-change re-arming;
- `cirdean-vision` using `image` + `imageproc`;
- Hbot-inspired multi-path contour preprocessing;
- candidate ranking using boundary support and document geometry instead of "largest rectangle wins";
- Camscan-inspired Hough lines, intersections, graph construction, and bounded 4-cycle search;
- staged hybrid routing so Hough runs only when the fast detector is ambiguous;
- distinct candidate ranking, ambiguity rejection, and weakest-side boundary validation;
- evidence fusion that preserves measured corners, with resolution-scaled Hough votes;
- Hbot-inspired book-gutter estimation improved with plateau centering and confidence;
- synthetic/unit tests plus CI gates for formatting, Clippy (`-D warnings`), and workspace tests.

## Why hybrid detection?

The two upstream approaches solve different failure modes:

```text
preview frame
    |
    +--> fast contour detector
    |         |
    |      confident ---------------------------+
    |         |                                 |
    |      ambiguous                            |
    |         v                                 |
    +--> bounded Hough fallback                 |
              |                                 |
        line intersections                      |
              |                                 |
        candidate 4-cycles                      |
              |                                 |
        scoring / fusion                        |
              +---------------------------------+
                        |
                 temporal evidence
                        |
                 capture quality
```

The contour path keeps preview latency low. The Hough path can reconstruct document geometry when a closed contour is fragmented. Cirdean bounds Hough line/cycle growth and treats detector agreement as optional evidence rather than forcing both algorithms to run on every frame.

## Design goals

- **Camera-native:** understand the document from a live stream before capture.
- **Staged hybrid detection:** cheap contour detection first, robust fallback only when useful.
- **Confidence-driven:** edge, geometry, temporal, and cross-detector evidence remain explicit.
- **Full-resolution capture:** detect on downscaled preview frames, then process the native-resolution frame.
- **Non-destructive processing:** preserve source images and model edits as reversible operations.
- **Modular Rust core:** keep camera, vision, enhancement, document, and UI layers separable.
- **Cross-platform direction:** avoid coupling the core engine to Windows-only APIs.

## Architecture

```text
Camera backend
  -> downscaled preview
  -> contour fast path
  -> optional Hough fallback
  -> detector fusion
  -> temporal tracking
  -> stability / quality gate
  -> full-resolution capture
  -> perspective correction
  -> enhancement / book split
  -> page session
  -> image / PDF export
```

See [`docs/architecture.md`](docs/architecture.md) for the system architecture and [`docs/upstream-algorithm-study.md`](docs/upstream-algorithm-study.md) for the detailed Hbot/Camscan algorithm study, failure modes, and Cirdean synthesis. Licensing/provenance notes are kept in [`docs/upstream.md`](docs/upstream.md) and [`NOTICE.md`](NOTICE.md).

The latest measured comparison, reproduction commands, and remaining failures are in
[`docs/detector-evaluation.md`](docs/detector-evaluation.md). The benchmark compares
Cirdean implementations; it does not establish superiority over the upstream Python applications.

## Roadmap

- **v0.1:** camera preview, hybrid boundary detection, confidence model, manual capture, perspective correction, PNG output.
- **v0.2:** temporal tracking, production auto-capture, multi-page sessions.
- **v0.3:** shadow removal, B&W enhancement, denoise, book mode/dewarping.
- **v0.4:** page management, non-destructive edits, PDF export.
- **v0.5:** OCR and searchable documents.

## Toolchain

Cirdean uses Rust edition 2024 with a current minimum Rust version of **1.89** for the selected dependency set.

## License

Cirdean is licensed under the MIT License. See [`LICENSE`](LICENSE) and [`NOTICE.md`](NOTICE.md).
