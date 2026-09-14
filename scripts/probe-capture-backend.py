"""Read-only camera control audit and sample from one OpenCV capture backend.

Camera index must be verified for the host; this probe does not write focus,
exposure, gain, or white-balance controls. Backend requests may be renegotiated.
"""

import argparse
import json
import time
from pathlib import Path

import cv2


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("backend", choices=["dshow", "msmf"])
parser.add_argument("outdir", type=Path)
parser.add_argument("--index", type=int, default=0)
args = parser.parse_args()
args.outdir.mkdir(parents=True, exist_ok=True)
backend_id = cv2.CAP_DSHOW if args.backend == "dshow" else cv2.CAP_MSMF
if not cv2.videoio_registry.hasBackend(backend_id):
    parser.error("Backend unavailable in this OpenCV build; MSMF requires a compatible non-headless Windows wheel.")
started = time.perf_counter()
capture = cv2.VideoCapture(args.index, backend_id)
try:
    if not capture.isOpened():
        raise RuntimeError("Camera open failed")
    accepted = {}
    for name, prop, value in [("fourcc", cv2.CAP_PROP_FOURCC, cv2.VideoWriter_fourcc(*"MJPG")),
                              ("width", cv2.CAP_PROP_FRAME_WIDTH, 1920),
                              ("height", cv2.CAP_PROP_FRAME_HEIGHT, 1080),
                              ("fps", cv2.CAP_PROP_FPS, 5)]:
        accepted[name] = capture.set(prop, value)
    values = {name: capture.get(prop) for name, prop in [
        ("width", cv2.CAP_PROP_FRAME_WIDTH), ("height", cv2.CAP_PROP_FRAME_HEIGHT),
        ("fps", cv2.CAP_PROP_FPS), ("fourcc", cv2.CAP_PROP_FOURCC),
        ("exposure", cv2.CAP_PROP_EXPOSURE), ("auto_exposure", cv2.CAP_PROP_AUTO_EXPOSURE),
        ("gain", cv2.CAP_PROP_GAIN), ("focus", cv2.CAP_PROP_FOCUS)]}
    times = []
    for index in range(25):
        ok, frame = capture.read()
        if not ok:
            raise RuntimeError(f"Frame {index} failed")
        times.append(time.perf_counter())
        if index >= 22 and not cv2.imwrite(str(args.outdir / f"{args.backend}-{index}.png"), frame):
            raise RuntimeError("Image write failed")
    report = {"backend": capture.getBackendName(), "index": args.index, "opencv": cv2.__version__,
              "set_returned": accepted, "get_returned": values,
              "actual_frame_shape": list(frame.shape),
              "read_intervals_ms": [(b-a)*1000 for a,b in zip(times, times[1:])],
              "elapsed_seconds": time.perf_counter()-started,
              "caveat": "Read intervals include client work; not sensor timestamps or unique FPS. Unsupported get may return zero. Not Windows Camera app parity."}
    fourcc = int(values["fourcc"])
    report["reported_fourcc_text"] = "".join(chr((fourcc >> (8*i)) & 255) for i in range(4))
    (args.outdir / f"{args.backend}.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))
finally:
    capture.release()
