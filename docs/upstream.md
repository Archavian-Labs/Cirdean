# Upstream research mapping

Cirdean is not intended to be a line-for-line rewrite of Hbot or Camscan. They are research references whose strongest ideas can be reimplemented behind Cirdean's own Rust architecture.

## Hbot

Useful concepts to study or independently reimplement:

- stability/motion-based hands-free capture,
- re-arming capture after a page turn,
- contour-based document detection as a fast path,
- perspective correction,
- gutter-aware two-page splitting,
- illumination/shadow normalization,
- document-oriented color and B&W enhancement.

## Camscan

Useful concepts to study or independently reimplement:

- camera device abstraction and configurable capture parameters,
- Hough-line document boundary detection,
- line-intersection graph construction,
- four-sided candidate generation,
- candidate scoring before perspective extraction.

## Cirdean changes

Cirdean combines those ideas differently:

1. fast contour detection runs first;
2. Hough detection is a fallback or second opinion rather than mandatory work on every frame;
3. detector agreement contributes to confidence;
4. temporal corner stability contributes to capture decisions;
5. detection runs on downscaled preview frames while the final warp uses a native-resolution frame;
6. image processing and UI are kept outside the core domain contracts.

## Licensing

Both referenced projects are MIT licensed. Conceptual inspiration does not require copying implementation text. When substantial code is actually ported or adapted, preserve the corresponding upstream copyright and MIT permission notice alongside the derived work.
