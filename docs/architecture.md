# Cirdean architecture

## Principle

Cirdean separates *seeing a document* from *capturing a document*. Preview frames should be cheap to analyze, while the selected source frame should retain the highest useful camera resolution.

## Pipeline

```text
Camera backend
    |
    +--> preview/downscale -------------------------------+
    |                                                     |
    |      fast contour detector                          |
    |              |                                      |
    |        low confidence?                              |
    |              +--> Hough detector                    |
    |                        |                             |
    |                 detector fusion                     |
    |                        |                             |
    |                 corner tracking                     |
    |                        |                             |
    |                capture quality gate                 |
    |                        |                             |
    +--> native frame <----- capture trigger -------------+
                             |
                      perspective warp
                             |
                       page processing
                             |
                      session / export
```

## Detection tiers

### Tier 1: contour detector

The first detector should favor latency and low CPU cost: grayscale, blur, edge/threshold maps, contour extraction, quadrilateral approximation, then candidate scoring.

### Tier 2: Hough fallback

If Tier 1 has ambiguous confidence, use a Hough-line detector inspired by Camscan: major line extraction, valid line intersections, quadrilateral candidate construction, and scoring.

### Fusion

When both detectors produce candidates, compare corner agreement and geometry. A fused result should expose its evidence instead of returning only four points.

## Temporal model

Document boundaries should be tracked across frames. Detection confidence alone is insufficient for auto-capture. Capture quality combines:

- motion stability,
- corner stability,
- sharpness,
- detector confidence,
- exposure stability.

The initial `cirdean-core` types intentionally make those values explicit so later calibration does not require redesigning the API.

## Crate direction

The bootstrap contains `cirdean-core` and a minimal `cirdean-desktop` executable. Planned boundaries are:

- `cirdean-core`: geometry, detection contracts, fusion, tracking, stability.
- `cirdean-camera`: camera discovery, streams, resolution/FPS negotiation.
- `cirdean-enhance`: illumination normalization, B&W, denoise, color enhancement.
- `cirdean-document`: page sessions, non-destructive operations, PDF/image export.
- `cirdean-ui`: reusable UI state and views.
- `cirdean-desktop`: desktop composition root.

Crates are added when their first implementation lands rather than committing empty placeholder packages.
