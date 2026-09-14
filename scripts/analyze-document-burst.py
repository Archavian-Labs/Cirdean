"""Analyze fixed-setup exposure bursts and export conservative RAM fusion previews.

Requires a visually verified four-corner ROI JSON from probe-document-roi.py.
All numeric luminance metrics are encoded-image values, not radiometry.
"""
import argparse
import hashlib
import json
from pathlib import Path

import cv2
import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bursts", type=Path)
    parser.add_argument("roi", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    points = np.array(json.loads(args.roi.read_text())["selected"]["quad"], dtype=np.float32)
    ordered = np.array([points[np.argmin(points.sum(1))], points[np.argmin(np.diff(points, axis=1))],
                        points[np.argmax(points.sum(1))], points[np.argmax(np.diff(points, axis=1))]])
    tl, tr, br, bl = ordered
    width = round(max(np.linalg.norm(tr-tl), np.linalg.norm(br-bl)))
    height = round(max(np.linalg.norm(bl-tl), np.linalg.norm(br-tr)))
    transform = cv2.getPerspectiveTransform(ordered, np.float32([[0,0], [width-1,0], [width-1,height-1], [0,height-1]]))
    paper_patch = (slice(round(height*.18), round(height*.38)), slice(round(width*.68), round(width*.88)))
    results = []
    usable = {}
    for path in sorted(args.bursts.glob("exposure-*.avi")):
        cap = cv2.VideoCapture(str(path))
        if not cap.isOpened():
            raise RuntimeError(f"Cannot decode {path}")
        frames, hashes, positions = [], [], []
        try:
            while True:
                ok, frame = cap.read()
                if not ok:
                    break
                position = cap.get(cv2.CAP_PROP_POS_MSEC)
                if position >= 3500:
                    frames.append(cv2.warpPerspective(frame, transform, (width, height)))
                    hashes.append(hashlib.sha256(frame.tobytes()).hexdigest())
                    positions.append(position)
        finally:
            cap.release()
        frames, hashes, positions = frames[-8:], hashes[-8:], positions[-8:]
        if len(frames) < 8:
            raise RuntimeError(f"Too few settled frames: {path}")
        gray = np.array([cv2.cvtColor(f, cv2.COLOR_BGR2GRAY) for f in frames], np.float32)
        patch = gray[:, paper_patch[0], paper_patch[1]]
        # Thresholds near the encoded endpoints flag highlight/shadow risk,
        # not a claim about sensor saturation.
        inner = np.array(frames)[:, 8:-8, 8:-8]
        clipped = float(np.mean(np.any(inner >= 250, axis=3)))
        pair_noise = float(np.mean([np.std(patch[i+1]-patch[i])/np.sqrt(2) for i in range(7)]))
        metrics = {"burst": path.stem, "frames": 8, "decoded_distinct_hashes": len(set(hashes)),
                   "container_positions_ms": positions, "document_size": [width,height],
                   "paper_mean_gray": float(patch.mean()), "paper_frame_mean_range": [float(patch.mean((1,2)).min()),float(patch.mean((1,2)).max())],
                   "document_near_white_fraction": clipped,
                   "paper_temporal_noise_proxy": pair_noise,
                   "document_p10_gray": float(np.percentile(gray[:,8:-8,8:-8],10))}
        cv2.imwrite(str(args.output / f"{path.stem}.png"), frames[0])
        results.append(metrics)
        usable[path.stem] = frames
    # Explicit research heuristic: protect highlights, then favor a brighter paper.
    eligible = [r for r in results if r["document_near_white_fraction"] < .01 and r["paper_mean_gray"] > 100]
    selected = max(eligible, key=lambda r: r["paper_mean_gray"], default=None)
    fusion = None
    if selected:
        frames = usable[selected["burst"]]
        reference = cv2.cvtColor(frames[0], cv2.COLOR_BGR2GRAY).astype(np.float32)/255
        aligned = [frames[0].astype(np.float32)]
        registration = []
        for frame in frames[1:]:
            warp = np.eye(2, 3, dtype=np.float32)
            try:
                correlation, warp = cv2.findTransformECC(reference, cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY).astype(np.float32)/255,
                    warp, cv2.MOTION_EUCLIDEAN, (cv2.TERM_CRITERIA_COUNT | cv2.TERM_CRITERIA_EPS, 80, 1e-5), None, 5)
                shift = float(np.linalg.norm(warp[:,2]))
                accepted = correlation > .95 and shift < 2 and abs(float(warp[0,1])) < .01
                registration.append({"correlation": correlation, "translation_pixels": shift, "accepted": accepted})
                if accepted:
                    aligned.append(cv2.warpAffine(frame.astype(np.float32), warp, (width,height), flags=cv2.INTER_LINEAR | cv2.WARP_INVERSE_MAP, borderMode=cv2.BORDER_REFLECT))
            except cv2.error as error:
                registration.append({"accepted": False, "error": str(error)})
        for count in (1,2,4,8):
            if len(aligned) >= count:
                averaged = np.mean(aligned[:count], axis=0).clip(0,255).astype(np.uint8)
                cv2.imwrite(str(args.output / f"fusion-{count}.png"), averaged)
                if count == 4:
                    mono = cv2.cvtColor(averaged, cv2.COLOR_BGR2GRAY).astype(np.float32)
                    black = float(np.percentile(mono[8:-8,8:-8], 1))
                    white = float(np.percentile(mono[paper_patch], 95))
                    if white > black:
                        normalized = ((mono-black)*240/(white-black)).clip(0,255).astype(np.uint8)
                        cv2.imwrite(str(args.output / "fusion-4-grayscale.png"), normalized)
        fusion = {"selected": selected["burst"], "accepted_frames":len(aligned), "registration":registration,
                  "selection_rule":"Highest paper mean among samples with <1% near-white document pixels and mean >100; provisional, not a sharpness optimum."}
        if len(aligned) == 8:
            patches = np.array([cv2.cvtColor(f,cv2.COLOR_BGR2GRAY)[paper_patch] for f in aligned])
            sigma1 = np.mean([np.std(patches[i+1]-patches[i])/np.sqrt(2) for i in range(7)])
            sigma4 = np.std(patches[:4].mean(0)-patches[4:].mean(0))/np.sqrt(2)
            fusion.update({"aligned_single_noise_proxy":float(sigma1), "four_frame_noise_proxy":float(sigma4),
                           "noise_reduction_fraction":float(1-sigma4/sigma1) if sigma1 > 0 else None})
    report = {"exposures":results,"fusion":fusion,"paper_patch_normalized_xyxy":[.68,.18,.88,.38],
              "limitations":["Fixed ROI assumes unchanged page position; verify exported crops visually.",
                "Grayscale preview is a separate global tone mapping derived from image percentiles; it is not part of the noise measurement or recovered detail.",
                "White balance remains auto; exposure sweep order is not randomized.",
                "Timestamps are container timestamps, not sensor timestamps. Different hashes do not prove independent exposures.",
                "Noise proxies use a single blank patch and limited pairs; alignment resampling also reduces noise.",
                "No calibrated sharpness chart or OCR ground truth; no claim that fusion improves true detail."]}
    (args.output/"analysis.json").write_text(json.dumps(report,indent=2),encoding="utf-8")
    print(json.dumps(report,indent=2))


if __name__ == "__main__":
    main()
