# Detector comparison and measured synthesis

Date: 2026-09-15. Original Cirdean revision:
`cd197ecf39316259e7b3314c9d501ab4fe70b161`.

## What the references contribute

| Aspect | Hbot | Camscan | Revised Cirdean |
| --- | --- | --- | --- |
| Candidate generation | Two contour preprocessing paths | Hough lines, intersections, four-cycles | Contour first, bounded Hough fallback |
| Selection | Largest valid four-corner contour | Boundary overlap | Geometry plus balanced four-side support |
| Broken contours | Requires usable polygon approximation | Can reconstruct from disconnected lines | Retains both sources of hypotheses |
| Competing objects | Area can favor a distractor | Strong lines can form false rectangles | Deduplicates hypotheses; rejects near ties |
| Detector agreement | Not applicable | Not applicable | Adds evidence without moving measured corners |
| Small input | Fixed pixel preprocessing | Fixed pixel thresholds | Hough votes scale with preview dimensions |

Sources inspected: [Hbot scanner at 0575c0b](https://github.com/abhishekdhital/hbot/blob/0575c0bf6aed77884c43bc9191574dc8607a05e4/hbot/scanner.py)
and [Camscan scanner at 8c06d74](https://github.com/suhren/camscan/blob/8c06d742dc77ad2c6768fdc873a0755764f1bb16/camscan/scanner.py).
The source comparison describes mechanisms, not measured upstream speed.
No upstream Python application was executed in this evaluation.

## Implemented algorithm

1. Generate contour hypotheses from ordinary and adaptive-threshold edges.
2. Score each boundary using `0.5 * mean(four side supports) + 0.5 * weakest side`.
3. Rank and deduplicate hypotheses within 2% of the longest source dimension,
   using cyclic corner alignment. Preserve the best measured quad.
4. Keep candidates with edge support >= 0.55 and confidence >= 0.65.
5. Accept the contour fast path only at confidence >= 0.88 and a margin >= 0.04
   over the next distinct eligible candidate.
6. Otherwise evaluate bounded Hough candidates, with vote thresholds scaled down
   for input smaller than the configured preview.
7. Add agreement evidence to corresponding contour/Hough hypotheses. Retain
   actual measured corners rather than averaging boundaries without rescoring.
8. Rank again; return `None` when the two strongest distinct hypotheses are
   separated by less than 0.04. This includes unresolved ambiguity.

The scores and thresholds are heuristics. Two detectors share edge evidence;
agreement is not independent probabilistic confirmation. A clear rectangle can
still be a screen, book cover, or other non-document object.

## Synthetic comparison and ablation

All runs use the same seven deterministic fixtures and the locked Rust dependency
set. Four positive cases require mean cyclic corner error <= 3% of the actual
frame diagonal. Two negative cases require no detection. Nested rectangles have
ambiguous intent and are excluded from accuracy, not counted as a success.
The fixture named `broken_outline` applies local border occlusions to a filled
polygon; it is not proof of recovery from a completely disconnected contour.

| Variant | Contour correct | Hough correct | Hybrid correct |
| --- | ---: | ---: | ---: |
| Original Cirdean | 5/6 | 2/6 | 3/6 |
| Original + Canny crash fix only | 5/6 | 4/6 | 5/6 |
| Revised algorithm | 5/6 | 5/6 | 6/6 |

Original Hough and hybrid panic on four of seven fixtures, including the
unscored nested case. A zero low threshold in imageproc 0.27 Canny admits
zero-gradient pixels during hysteresis, reaching an out-of-bounds neighbor.
Changing it to 1.0 removes those panics. The isolated ablation changes only that
threshold, so its improvement must not be credited to candidate ranking.

Beyond the crash fix, resolution-scaled Hough votes recover the 160x120 document:
mean corner error 0.925 px. Contour alone still misses this case. The four positive
hybrid errors are 2.364, 2.179, 1.240, and 0.925 px. Both negative fixtures remain
negative in all variants; two negatives do not establish a population false-positive rate.

The stricter edge score has costs. The low-contrast hybrid case takes about 38 ms
instead of 24.5 ms in the crash-fixed baseline because it now invokes Hough.
On the clean skewed case, preserving a measured boundary gives 2.364 px error
versus 2.060 px for the earlier averaged quad. On the occluded case, it improves
1.849 px to 1.240 px. There is no claim that corner averaging is always worse.

Latency is detector-only, local Windows release-build timing: one warm-up and
median of three runs, with image generation excluded. These are small diagnostic
samples, not an FPS guarantee or a statistically powered performance study.

Raw results: [original](evaluation/original.csv),
[Canny-only ablation](evaluation/canny-only.csv), [revised](evaluation/revised.csv).

## Real-image check

The five images and corner annotations come from Camscan commit
`8c06d742dc77ad2c6768fdc873a0755764f1bb16`, under `tests/`.
The downloader pins URLs, verifies SHA256, and retains the upstream MIT license.
Pass means every corner is within 30 source pixels after best cyclic alignment;
mean and maximum errors are both reported. This removes corner-start ambiguity
while using the upstream per-corner tolerance.

| Fixture | Revised hybrid | Mean error | Maximum error |
| --- | --- | ---: | ---: |
| IMG_1842 | Pass | 2.55 px | 3.08 px |
| IMG_1843 | Pass | 2.85 px | 4.98 px |
| IMG_1844 | No detection | — | — |
| IMG_1845 | Pass | 4.91 px | 6.56 px |
| IMG_1846 | Pass | 3.29 px | 5.77 px |

Contour alone passes 1/5; Hough and hybrid each pass 4/5. The hybrid takes about
59–63 ms per image; Hough alone about 16–17 ms. This fixture set mostly needs the
fallback, so hybrid pays contour overhead without improving Hough's recall.
Timing here is a single release run per image with decoding excluded.
[Raw real-image results](evaluation/real-images.csv).

IMG_1844 remains an unresolved miss. The current work does not establish better
accuracy than upstream Hbot or Camscan, nor generalize from five related photos.
Synthetic cases informed these changes, so they are development checks rather
than a held-out validation set. A next evaluation should add independent camera
photos, document-free scenes, severe perspective, glare, motion, and edge occlusion.

## Reproduce

From the repository root:

```powershell
cargo run --locked -p cirdean-vision --release --example compare_detectors
pwsh -NoProfile -File scripts/fetch-camscan-fixtures.ps1
cargo run --locked -p cirdean-vision --release --example evaluate_images -- target/camscan-fixtures
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

To reproduce the Canny-only ablation, check out the original revision in a separate
directory, copy the current `Cargo.lock` and synthetic example into it, and change
only `canny(&closed, 0.0, 84.0)` to `canny(&closed, 1.0, 84.0)`. Build in a separate
target directory. The original baseline uses that revision without the threshold
change. Neither baseline requires modifying the active checkout.

Unit regressions additionally cover absent fourth-side evidence, duplicate views,
cyclic corner identity, conflicting detectors, candidate ties, the preserved
contour fast path, small frames, and Canny zero-gradient flooding.
