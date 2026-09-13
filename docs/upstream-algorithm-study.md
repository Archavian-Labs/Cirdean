# Upstream algorithm study: Hbot and Camscan

This document records the engineering study used to evolve Cirdean beyond a direct port of either upstream project. Cirdean is an independent Rust implementation. The goal is to preserve useful algorithmic ideas, identify failure modes, and make the resulting pipeline measurable and testable.

## Hbot

Studied upstream: `abhishekdhital/hbot`.

### Document boundary detector

Hbot reduces large images so the longest side is about 700 pixels, converts to grayscale, applies a small Gaussian blur, then runs two alternative edge strategies:

1. Canny edges at roughly 50/150 followed by a small dilation.
2. Adaptive Gaussian thresholding followed by Canny, intended to recover low-contrast page boundaries.

External contours are sorted by area. The implementation examines a small number of the largest contours, rejects contours below roughly 15% of the frame, approximates each contour as a polygon at about 2% of its perimeter, and accepts the largest four-corner candidate.

**Strengths**

- Cheap enough for live preview.
- Multiple preprocessing paths make it less brittle than a single Canny pass.
- The area gate removes much small-background clutter.
- Simple, easy to reason about, and easy to benchmark.

**Weaknesses**

- "Largest valid quad wins" has little semantic scoring. A rectangular monitor, mat, notebook cover, or table feature can beat the intended page.
- A broken page edge may fail polygon approximation even when the underlying long lines are obvious.
- Fixed pixel thresholds do not automatically adapt to preview size or scene statistics.

### Auto-capture

Hbot downsamples consecutive frames to 320x240 grayscale and uses mean absolute pixel difference as a motion signal. Once a detected document stays below the motion threshold for about 1.2 seconds, it captures and disarms. Motion caused by turning/replacing the page re-arms capture.

The important idea is not the exact threshold. It is the state machine: **stable -> capture once -> wait for page change -> re-arm**.

Cirdean keeps this behavior but separates page-change evidence from corner movement. A replacement page can occupy almost the exact same geometry, so relying only on quad displacement would produce duplicate or missed captures. Cirdean also shortens the initial default dwell time and delegates capture quality to a multi-signal gate.

### Book gutter and enhancement

Hbot searches the middle 35-65% of a scanned spread for the darkest smoothed vertical band and treats it as the binding gutter. Its enhancement pipeline also contains useful illumination-normalization, adaptive B&W, denoise, local-contrast and sharpening ideas.

Cirdean treats the gutter location as an estimate with confidence rather than an unconditional split. Enhancement is kept outside the detector so it cannot contaminate boundary confidence.

## Camscan

Studied upstream: `suhren/camscan`.

### Hough / graph document detector

Camscan downsamples to about 500 pixels in height, grayscales, strongly blurs and morphologically closes the image, then applies Canny. It runs Hough line detection with increasingly strict vote thresholds until the number of major lines is manageable.

The detector then:

1. Rejects nearly parallel line pairs and computes valid line intersections inside the image.
2. Builds a graph whose nodes are intersections.
3. Connects nodes that share one of the Hough lines and are sufficiently far apart.
4. Finds unique graph cycles of length four.
5. Treats those cycles as quadrilateral candidates.
6. Rejects candidates below roughly 20% of the image area.
7. Scores candidate boundaries using overlap with the edge image.
8. Perspective-warps the best result.

Its test suite includes graph-cycle tests, corner-order tests, and real-image detector fixtures. The real-image tests allow up to about 30 pixels of corner error.

**Strengths**

- Can reconstruct a page from long boundary lines even when a closed contour is fragmented.
- Candidate scoring is a better basis than area alone.
- Explicit geometry primitives are independently testable.

**Weaknesses**

- Hough output can explode in line-heavy scenes such as desks, keyboards, shelves, grids, or printed engineering drawings.
- Intersection graphs have combinatorial cost if line count is not tightly bounded.
- Fixed minimum intersection angle and pixel-distance thresholds are resolution-sensitive.
- Running this full path on every preview frame wastes CPU when the contour detector already has a high-confidence answer.

## Cirdean synthesis

Cirdean uses a staged detector rather than blindly running both upstream algorithms:

```text
preview frame
    |
    +--> multi-path contour detector
    |         |
    |     strong confidence --------------------------+
    |         |                                       |
    |     ambiguous                                   |
    |         v                                       |
    +--> Hough lines -> intersections -> bounded      |
         4-cycle search -> candidate scoring          |
              |                                       |
              +--> agreement/fusion                   |
                          |                            |
                          +----------------------------+
                                       |
                              temporal evidence
                                       |
                              capture quality gate
                                       |
                         stable -> capture exactly once
                                       |
                              wait for page change
```

### Changes over Hbot

- Candidate ranking combines edge support and geometric plausibility instead of using area alone.
- Cross-detector evidence can strengthen an ambiguous contour rather than replacing it.
- Auto-capture uses an explicit state machine and a quality gate.
- Page-change evidence is separate from quad displacement.
- Thresholds that represent geometry should be normalized to frame dimensions where practical.

### Changes over Camscan

- Hough is a fallback, not the default per-frame cost.
- Line count and cycle enumeration are hard-bounded.
- Candidate geometry is scored before expensive downstream work.
- Hough/contour disagreement is useful evidence, not something silently discarded.
- Confidence is explicit in the public domain model.

## Confidence semantics

Cirdean distinguishes mandatory evidence from optional evidence.

Mandatory evidence:

- edge support,
- geometric plausibility.

Optional evidence:

- temporal consistency,
- contour/Hough agreement.

Optional evidence weights are added only when the evidence exists, and confidence is renormalized over active weights. This matters for staged execution: a good contour detection must be capable of reaching high confidence without forcing the Hough fallback to run merely to populate an agreement field.

## Capture-state improvements

The core capture controller currently models:

1. `Searching`
2. `Stabilizing`
3. `AwaitingPageChange`

A page is captured only after stable dwell time and quality-gate acceptance. After capture, the same document cannot trigger another capture until there is sufficient scene-change evidence or meaningful boundary displacement. In the vision layer, scene change should be measured primarily inside/around the document rather than across the whole camera frame so unrelated background motion does not re-arm the scanner.

## Validation strategy

Cirdean should maintain three classes of tests:

- deterministic geometry/unit tests,
- synthetic image tests for controlled distortions and clutter,
- real camera fixtures with expected corners and tolerances.

A detector change is not considered an improvement merely because it looks better on one sample photo. It should improve or preserve fixture accuracy, false-positive rate, and preview latency.

## Upstream licenses

Both studied projects are MIT licensed. Hbot currently carries `Copyright (c) 2026 KibuSpace`; Camscan carries `Copyright (c) 2023 Adam Suhren Gustafsson`. Cirdean currently re-implements the studied ideas independently rather than copying source files verbatim. If substantial upstream code is later ported or adapted, the applicable upstream copyright and permission notice must accompany that derived code.
