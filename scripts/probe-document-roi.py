"""Offline OpenCV ROI experiment; does not control camera focus.

Run: uv run --with opencv-python-headless python scripts/probe-document-roi.py IMAGE OUTDIR
"""

import argparse
import json
from pathlib import Path
from time import perf_counter

import cv2
import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path)
    parser.add_argument("outdir", type=Path)
    args = parser.parse_args()
    source = cv2.imread(str(args.image))
    if source is None:
        parser.error(f"Cannot read {args.image}")
    started = perf_counter()
    height, width = source.shape[:2]
    gray = cv2.cvtColor(source, cv2.COLOR_BGR2GRAY)
    smooth = cv2.GaussianBlur(gray, (5, 5), 1.2)
    edges = cv2.Canny(smooth, 40, 120)
    maps = {
        "edges": cv2.morphologyEx(edges, cv2.MORPH_CLOSE, np.ones((3, 3), np.uint8)),
        "otsu": cv2.threshold(smooth, 0, 255, cv2.THRESH_BINARY | cv2.THRESH_OTSU)[1],
    }
    candidates = []
    for method, binary in maps.items():
        contours, _ = cv2.findContours(binary, cv2.RETR_LIST, cv2.CHAIN_APPROX_SIMPLE)
        for contour in contours:
            area = cv2.contourArea(contour)
            if not 0.01 <= area / (width * height) <= 0.85:
                continue
            quad = cv2.approxPolyDP(contour, 0.02 * cv2.arcLength(contour, True), True)
            if len(quad) != 4 or not cv2.isContourConvex(quad):
                continue
            x, y, w, h = cv2.boundingRect(quad)
            if min(x, y, width - x - w, height - y - h) < 3:
                continue
            (_, _), (rw, rh), _ = cv2.minAreaRect(contour)
            rectangularity = area / max(rw * rh, 1)
            if rectangularity < 0.8:
                continue
            candidates.append({"method": method, "quad": quad[:, 0].tolist(),
                               "bbox_xywh": [x, y, w, h], "area_ratio": area / (width * height),
                               "rectangularity": rectangularity, "rank_score": area * rectangularity})
    candidates.sort(key=lambda candidate: candidate["rank_score"], reverse=True)
    result = {"source": str(args.image.resolve()), "opencv": cv2.__version__,
              "dimensions": [width, height], "candidates": candidates[:10],
              "selected": candidates[0] if candidates else None,
              "detection_ms": (perf_counter() - started) * 1000,
              "limitations": "Largest interior rectangular candidate heuristic; not calibrated confidence or autofocus."}
    args.outdir.mkdir(parents=True, exist_ok=True)
    if candidates:
        selected = candidates[0]
        x, y, w, h = selected["bbox_xywh"]
        pad = round(max(w, h) * 0.1)
        left, top = max(0, x - pad), max(0, y - pad)
        right, bottom = min(width, x + w + pad), min(height, y + h + pad)
        result["padded_roi_xywh"] = [left, top, right - left, bottom - top]
        overlay = source.copy()
        cv2.polylines(overlay, [np.array(selected["quad"], np.int32)], True, (0, 255, 0), 2)
        cv2.rectangle(overlay, (left, top), (right - 1, bottom - 1), (255, 160, 0), 2)
        for name, pixels in [("overlay.png", overlay), ("roi-native.png", source[top:bottom, left:right])]:
            if not cv2.imwrite(str(args.outdir / name), pixels):
                raise RuntimeError(f"Cannot write {name}")
    (args.outdir / "result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps({key: value for key, value in result.items() if key != "candidates"}, indent=2))


if __name__ == "__main__":
    main()
