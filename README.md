# Cirdean

Cirdean is a camera-native document acquisition and scanning system written in Rust.

The project aims to combine fast contour-based document detection, a more robust Hough-line fallback detector, temporal stability tracking, full-resolution capture, perspective correction, document enhancement, book scanning, and structured export without tying the core engine to a specific GUI.

## Status

Cirdean is in early research and architecture development. The current `research/detection-foundation` branch establishes the Rust workspace and the domain model for detection confidence, geometry, and capture quality.

## Design goals

- **Camera-native:** understand the document from a live stream before capture.
- **Hybrid detection:** use a fast contour detector first and a Hough-based fallback when confidence is low.
- **Confidence-driven:** detections carry geometry, edge, temporal, and overall confidence rather than a bare quadrilateral.
- **Full-resolution capture:** detect on downscaled preview frames, then process the native-resolution frame.
- **Non-destructive processing:** preserve source images and model edits as reversible operations.
- **Modular Rust core:** keep camera, vision, enhancement, document, and UI layers separable.
- **Cross-platform direction:** avoid coupling the core engine to Windows-only APIs.

## Planned architecture

```text
Camera
  -> preview frame
  -> fast contour detector
  -> Hough fallback detector
  -> detector fusion
  -> temporal corner tracking
  -> stability / quality gate
  -> full-resolution capture
  -> perspective correction
  -> enhancement / book split
  -> page session
  -> image / PDF export
```

See [`docs/architecture.md`](docs/architecture.md) for the current architecture and [`docs/upstream.md`](docs/upstream.md) for upstream inspiration and licensing notes.

## Roadmap

- **v0.1:** camera preview, hybrid boundary detection, confidence model, manual capture, perspective correction, PNG output.
- **v0.2:** stability tracking, auto-capture, multi-page sessions.
- **v0.3:** shadow removal, B&W enhancement, denoise, book mode.
- **v0.4:** page management, non-destructive edits, PDF export.
- **v0.5:** OCR and searchable documents.

## License

Cirdean is licensed under the MIT License. See [`LICENSE`](LICENSE) and [`NOTICE.md`](NOTICE.md).
