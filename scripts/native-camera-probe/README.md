# Native camera research probe

Windows-only, standalone Rust executable using the locally available windows-rs
0.54 bindings. It does not add a production camera backend to Cirdean.

## Commands

From the Cirdean root:

```powershell
cargo run --manifest-path scripts/native-camera-probe/Cargo.toml -- output/my-native-probe native
cargo run --manifest-path scripts/native-camera-probe/Cargo.toml -- output/my-sta-probe native-sta
cargo run --manifest-path scripts/native-camera-probe/Cargo.toml -- output/my-photo-probe photo output/my-native-probe/mf-devices.json
```

The native command selects the CM678 by its hardware ID, enumerates Source Reader
native media types, disables Source Reader converters, requests advertised MJPEG
1080p30, and attempts ten encoded sample dumps with timestamps and optional
metadata. Encoded MJPEG is not sensor RAW. Attributes describe the Windows source,
not an independently observed USB transfer. Missing metadata remains null.

`native` initializes MTA; `native-sta` runs the same enumeration and activation
sequence on the main STA. Both log `CoGetApartmentType` immediately before the
two calls. `environment.json` records the process apartment, package identity,
session and executable. `environment.ps1` reads that exact process's token plus
OS and camera policy values into `host-environment.json`; it makes no changes.

The photo command uses a freshly enumerated symbolic link supplied in the JSON
inventory. It enumerates Preview/Video/Photo properties, selects an advertised
1080p Photo property, and attempts low-lag JPEG photo captures with per-photo
metadata. That is not proof of equivalence to Windows Camera or of a dedicated
hardware still pin. No encoded resize or exposure/WB/focus settings are requested.

Photo initialization now starts on the main STA with a small research window.
WinRT completion callbacks signal an atomic flag while a bounded
`MsgWaitForMultipleObjectsEx`/PeekMessage/DispatchMessage loop pumps messages.
`GetResults` runs only after completion; there are no synchronous WinRT `.get()`
calls. A 30-second async timeout requests cancellation. Initialization failures
record their exact stage and HRESULT in `failure.json`.

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

The follow-up MTA, main-STA, and windowed/message-pumping MediaCapture trials all
returned `0x80070005` at source activation or Initialize completion. STA alone
did not fix this executable. This does not establish the underlying cause.
Regular Photo capture, native color attributes and per-frame metadata remain
blocked; no Method 2 use is claimed. See `output/camera-characterization/`.

Reference: [Microsoft InitializeAsync requirements](https://learn.microsoft.com/en-us/uwp/api/windows.media.capture.mediacapture.initializeasync).

The completed fallback experiment lives in `experiment-document-exposure.ps1`
and `analyze-document-burst.py` one directory up. Its report documents its own
capture path and measurement limits.
