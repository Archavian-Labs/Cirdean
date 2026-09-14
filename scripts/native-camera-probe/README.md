# Native camera research probe

Windows-only, standalone Rust executable using the locally available windows-rs
0.54 bindings. It does not add a production camera backend to Cirdean.

## Commands

From the Cirdean root:

```powershell
cargo run --manifest-path scripts/native-camera-probe/Cargo.toml -- output/my-native-probe native
cargo run --manifest-path scripts/native-camera-probe/Cargo.toml -- output/my-photo-probe photo output/my-native-probe/mf-devices.json
```

The native command selects the CM678 by its hardware ID, enumerates Source Reader
native media types, disables Source Reader converters, requests advertised MJPEG
1080p30, and attempts ten encoded sample dumps with timestamps and optional
metadata. Encoded MJPEG is not sensor RAW. Attributes describe the Windows source,
not an independently observed USB transfer. Missing metadata remains null.

The photo command uses a freshly enumerated symbolic link supplied in the JSON
inventory. It enumerates Preview/Video/Photo properties, selects an advertised
1080p Photo property, and attempts low-lag JPEG photo captures with per-photo
metadata. That is not proof of equivalence to Windows Camera or of a dedicated
hardware still pin. No encoded resize or exposure/WB/focus settings are requested.

Without an inventory path, the photo command tries WinRT device enumeration.
There is a 120-second process watchdog. Camera handles are closed on normal error
paths; process termination releases them if an API stalls.

## Validation and current limitation

`cargo check`, `cargo fmt`, and `cargo clippy -- -D warnings` pass on this host.
MF device enumeration succeeded. Native activation returned E_ACCESSDENIED
(0x80070005); MediaCapture initialization returned the same error. WinRT device
enumeration without a supplied ID timed out. Therefore native/photo capture and
metadata parsing are compiled but **not runtime validated**. Existing OpenCV MSMF
capture succeeded. No permissions, drivers, registry, or executable identity were
modified to work around the denial.

The completed fallback experiment lives in `experiment-document-exposure.ps1`
and `analyze-document-burst.py` one directory up. Its report documents its own
capture path and measurement limits.
