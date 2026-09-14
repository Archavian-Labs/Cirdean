"""Compare native-size, stream-copied CM678 captures; no image enhancement/warping."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

import cv2
import numpy as np
from PIL import Image


def command(args):
    subprocess.run(args, check=True, capture_output=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    records = []
    blank = (slice(215, 245), slice(690, 730))
    text = (slice(280, 350), slice(520, 750))
    for take in sorted(args.directory.glob("*/*.avi")):
        folder = take.parent / take.stem
        folder.mkdir(exist_ok=True)
        base = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(take), "-ss", "5.4", "-map", "0:v:0", "-frames:v", "8", "-c:v", "copy", "-y"]
        samples = []
        encoded = []
        if take.parent.name == "mjpeg":
            command(base + [str(folder / "frame-%02d.jpg")])
            for path in sorted(folder.glob("frame-*.jpg")):
                with Image.open(path) as image:
                    assert image.size == (1920, 1080)
                    samples.append(np.asarray(image.convert("YCbCr"))[:, :, 0].astype(np.float32))
                    encoded.append({"file": str(path.relative_to(args.directory)), "bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "dqt": image.quantization})
            preview = cv2.imread(str(folder / "frame-01.jpg"))
        else:
            raw = folder / "frames.yuy2"
            # AVI contains empty timing packets. Stream-copy -frames:v counts
            # those on this rawvideo path; extract all payloads, then retain tail.
            command(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(take), "-map", "0:v:0", "-c:v", "copy", "-f", "rawvideo", "-y", str(raw)])
            data = raw.read_bytes()
            frame_size = 1920 * 1080 * 2
            assert len(data) % frame_size == 0
            data = data[-8 * frame_size:]
            raw.write_bytes(data)
            for index in range(len(data) // frame_size):
                chunk = data[index * frame_size:(index + 1) * frame_size]
                packed = np.frombuffer(chunk, np.uint8).reshape(1080, 1920, 2)
                # Explicit analysis normalization for reported limited-range YUY2.
                # This changes numeric units only; no spatial resampling is applied.
                samples.append((packed[:, :, 0].astype(np.float32) - 16) * (255 / 219))
                encoded.append({"file": str(raw.relative_to(args.directory)), "offset": index * frame_size, "bytes": frame_size, "sha256": hashlib.sha256(chunk).hexdigest()})
                if index == 0:
                    preview = cv2.cvtColor(packed, cv2.COLOR_YUV2BGR_YUY2)
        if len(samples) < 4:
            raise RuntimeError(f"Too few settled frames: {take}")
        stack = np.stack(samples)
        patch = stack[:, blank[0], blank[1]]
        text_patch = stack[:, text[0], text[1]]
        # Full-frame difference coordinates preserve the JPEG block grid origin.
        dx = np.abs(np.diff(stack, axis=2))[:, blank[0], blank[1]]
        columns = np.arange(690, 730) + 1
        block = float(dx[:, :, columns % 8 == 0].mean())
        nonblock = float(dx[:, :, columns % 8 != 0].mean())
        gradients = [np.hypot(cv2.Sobel(p, cv2.CV_32F, 1, 0, ksize=3), cv2.Sobel(p, cv2.CV_32F, 0, 1, ksize=3)) for p in text_patch]
        row = {"take": str(take.relative_to(args.directory)), "format": take.parent.name,
               "frames": len(samples), "source_sha256": hashlib.sha256(take.read_bytes()).hexdigest(),
               "blank_mean": float(patch.mean()), "blank_temporal_noise": float(patch.std(axis=0, ddof=1).mean()),
               "blank_frame_means": patch.mean(axis=(1, 2)).tolist(),
               "text_p90_p10": float(np.mean(np.percentile(text_patch, 90, axis=(1,2)) - np.percentile(text_patch, 10, axis=(1,2)))),
               "text_gradient_p90": float(np.percentile(gradients, 90)),
               "text_laplacian_energy": float(np.mean([np.mean(cv2.Laplacian(p, cv2.CV_32F)**2) for p in text_patch])),
               "blank_x_boundary8_difference": block, "blank_x_other_difference": nonblock,
               "blank_x_block_ratio": block/nonblock,
               "samples": encoded}
        records.append(row)
        cv2.imwrite(str(folder / "preview.png"), preview)
        cv2.imwrite(str(folder / "paper-crop.png"), preview[84:461, 437:847])
    result = {"method": "Same native pixel coordinates, eight settled frames per take; no alignment, resizing, sharpening, denoising or warp. MJPEG Y is Pillow decoded; YUY2 Y is directly extracted and normalized from reported 16..235 to 0..255. Neither is linear radiometry. Noise includes residual motion/flicker. Gradient/Laplacian metrics include noise and cannot establish resolved detail. Block ratio is scene-sensitive, horizontal boundaries only.",
              "regions_xywh": {"blank": [690, 215, 40, 30], "text": [520, 280, 230, 70]},
              "unmeasured": ["calibrated SFR/MTF", "known-glyph stroke width", "edge overshoot/ringing", "OCR accuracy"], "takes": records}
    (args.directory / "metrics.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps([{k:v for k,v in r.items() if k not in ("samples", "blank_frame_means")} for r in records], indent=2))


if __name__ == "__main__":
    main()
