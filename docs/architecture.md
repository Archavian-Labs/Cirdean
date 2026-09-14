# Cirdean architecture

## Principle

Cirdean separates *seeing a document* from *capturing a document*. Preview frames should be cheap to analyze, while the selected source frame should retain the highest useful camera resolution.

The detection path is staged. Expensive Hough geometry is a fallback for ambiguous contour results, not a tax paid on every video frame.

## Pipeline

```text
Camera backend
    |
    +--> preview/downscale ------------------------------------+
    |                                                          |
    |      multi-path contour detector                         |
    |              |                                           |
    |        high confidence? -- yes --------------------------+
    |              | no / ambiguous                            |
    |              v                                           |
    |        Hough fallback detector                            |
    |              |                                           |
    |      bounded candidates + scoring                        |
    |              |                                           |
    |         detector fusion                                   |
    |              |                                           |
    |       temporal corner tracking                            |
    |              |                                           |
    |       capture quality + dwell                             |
    |              |                                           |
    |       auto-capture state machine                          |
    |              |                                           |
    +--> native frame <----- capture trigger ------------------+
                             |
                      perspective warp
                             |
                   enhancement / book split
                             |
                      session / export
```

## Detection tiers

### Tier 1: contour detector

The first detector favors latency and low CPU cost. Following the strongest part of Hbot's approach, Cirdean uses multiple preprocessing views rather than trusting one edge map. Candidate quads are scored by edge support and geometry instead of accepting the largest rectangle blindly.

### Tier 2: Hough fallback

If Tier 1 is ambiguous, use a Hough-line detector informed by Camscan: major line extraction, valid line intersections, quadrilateral construction from bounded four-cycles, and candidate scoring. Line count and cycle count must have explicit limits because line-heavy scenes otherwise create combinatorial junk.

### Fusion

When both detectors produce plausible candidates, compare corresponding corners. If they agree, confidence-weighted fusion produces the final quad and adds agreement as evidence. If they disagree, the stronger candidate may still survive but does not receive an agreement bonus.

## Confidence model

`edge_score` and `geometry_score` are mandatory. `temporal_score` and `agreement_score` are optional evidence. The confidence model renormalizes over whatever evidence exists so the fast detector is not artificially capped before fallback execution.

## Temporal and capture model

Detection confidence alone is insufficient for auto-capture. Capture quality combines:

- motion stability,
- corner stability,
- sharpness,
- detector confidence,
- exposure stability.

The auto-capture controller is a state machine:

```text
Searching
   |
   v
Stabilizing -- movement/scene change --> restart dwell
   |
   +-- quality + dwell satisfied --> Capture once
                                      |
                                      v
                              AwaitingPageChange
                                      |
                         page-turn evidence / new quad
                                      |
                                      v
                                  Stabilizing
```

Page-change evidence is intentionally separate from corner displacement. A new sheet can occupy almost exactly the same quadrilateral as the previous sheet.

## Resolution policy

Boundary detection runs on a downscaled preview. Once a capture is triggered, preview coordinates are mapped back to the native camera frame and perspective correction runs on the high-resolution source. Cirdean should never throw away sensor resolution merely because the detector does not need it.

## Crate direction

- `cirdean-core`: geometry, detection contracts, fusion, capture state, tracking and quality semantics.
- `cirdean-vision`: pure-Rust image preprocessing, contour/Hough detectors, candidate scoring and book geometry.
- `cirdean-camera`: camera discovery, streams, resolution/FPS negotiation.
- `cirdean-enhance`: illumination normalization, B&W, denoise and color enhancement.
- `cirdean-document`: page sessions, non-destructive operations, PDF/image export.
- `cirdean-ui`: reusable UI state and views.
- `cirdean-desktop`: desktop composition root.

Crates are added when their first implementation lands rather than committing empty placeholder packages.

See `upstream-algorithm-study.md` for the detailed Hbot/Camscan analysis that motivates these choices.
