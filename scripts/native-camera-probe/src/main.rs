use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use windows::{
    Devices::Enumeration::{DeviceClass, DeviceInformation},
    Media::{
        Capture::{
            MediaCapture, MediaCaptureInitializationSettings, MediaStreamType, StreamingCaptureMode,
        },
        MediaProperties::{
            IMediaEncodingProperties, ImageEncodingProperties, VideoEncodingProperties,
        },
    },
    Storage::Streams::DataReader,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
    core::Interface,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
mod native;

struct CaptureGuard(MediaCapture);
impl Drop for CaptureGuard {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}

fn describe(p: &IMediaEncodingProperties) -> Value {
    let mut value = json!({"type": p.Type().ok().map(|s| s.to_string()),
        "subtype": p.Subtype().ok().map(|s| s.to_string())});
    if let Ok(v) = p.cast::<VideoEncodingProperties>() {
        value["width"] = json!(v.Width().ok());
        value["height"] = json!(v.Height().ok());
        if let Ok(fps) = v.FrameRate() {
            value["fps_numerator"] = json!(fps.Numerator().ok());
            value["fps_denominator"] = json!(fps.Denominator().ok());
        }
    } else if let Ok(v) = p.cast::<ImageEncodingProperties>() {
        value["width"] = json!(v.Width().ok());
        value["height"] = json!(v.Height().ok());
    }
    value
}

fn save(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn run(out: &Path) -> Result<()> {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    fs::create_dir_all(out)?;
    let mut selected = None;
    let mut inventory = vec![];
    if let Some(inventory_path) = std::env::args().nth(3) {
        let records: Vec<Value> = serde_json::from_slice(&fs::read(inventory_path)?)?;
        for device in records {
            if let Some(id) = device["id"]
                .as_str()
                .filter(|id| id.to_lowercase().contains("vid_0c45&pid_6369&mi_00"))
            {
                if selected.is_some() {
                    return Err("Multiple matching device IDs".into());
                }
                selected = Some(windows::core::HSTRING::from(id));
                inventory.push(device);
            }
        }
    } else {
        eprintln!("Enumerating WinRT camera devices");
        let devices =
            DeviceInformation::FindAllAsyncDeviceClass(DeviceClass::VideoCapture)?.get()?;
        for device in devices {
            let id = device.Id()?;
            inventory.push(json!({"name": device.Name()?.to_string(), "id": id.to_string()}));
            if id
                .to_string()
                .to_lowercase()
                .contains("vid_0c45&pid_6369&mi_00")
            {
                if selected.is_some() {
                    return Err("Multiple matching devices; ambiguous selection".into());
                }
                selected = Some(id);
            }
        }
    }
    save(&out.join("devices.json"), &json!(inventory))?;
    let selected = selected.ok_or("CM678 device not found")?;
    let settings = MediaCaptureInitializationSettings::new()?;
    settings.SetVideoDeviceId(&selected)?;
    settings.SetStreamingCaptureMode(StreamingCaptureMode::Video)?;
    let camera = CaptureGuard(MediaCapture::new()?);
    eprintln!("Initializing MediaCapture on CM678 (video only)");
    camera.0.InitializeWithSettingsAsync(&settings)?.get()?;
    let controller = camera.0.VideoDeviceController()?;
    let mut modes = json!({});
    let mut chosen = None;
    for (name, kind) in [
        ("preview", MediaStreamType::VideoPreview),
        ("video", MediaStreamType::VideoRecord),
        ("photo", MediaStreamType::Photo),
    ] {
        let properties = controller.GetAvailableMediaStreamProperties(kind)?;
        let mut rows = vec![];
        for p in properties {
            let row = describe(&p);
            if name == "photo" && row["width"] == 1920 && row["height"] == 1080 && chosen.is_none()
            {
                chosen = Some(p.clone());
            }
            rows.push(row);
        }
        modes[name] = json!({"available": rows, "initial": describe(&controller.GetMediaStreamProperties(kind)?)});
    }
    save(&out.join("media-types.json"), &modes)?;
    if let Some(p) = chosen {
        controller
            .SetMediaStreamPropertiesAsync(MediaStreamType::Photo, &p)?
            .get()?;
    } else {
        return Err("No advertised 1920x1080 Photo property; refusing implicit resizing".into());
    }
    let exposure = controller.ExposureControl()?;
    let wb = controller.WhiteBalanceControl()?;
    let focus = controller.FocusControl()?;
    save(
        &out.join("controls.json"),
        &json!({
        "exposure": {"supported": exposure.Supported().ok(), "auto": exposure.Auto().ok(),
            "readback_100ns": exposure.Value().ok().map(|v| v.Duration)},
        "white_balance": {"supported": wb.Supported().ok(), "preset": wb.Preset().ok().map(|v| v.0), "readback_kelvin": wb.Value().ok()},
        "focus_supported": focus.Supported().ok(),
        "photo_selected": describe(&controller.GetMediaStreamProperties(MediaStreamType::Photo)?),
        "note": "No exposure/WB/focus writes; control readback is not independent sensor measurement. Photo properties are API-advertised, not proof of a dedicated USB still pin."}),
    )?;
    eprintln!("Photo capabilities saved; preparing low-lag JPEG photo capture");
    let format = ImageEncodingProperties::CreateJpeg()?;
    // No encoded width/height requested: use the explicitly selected source mode.
    let photos = camera.0.PrepareLowLagPhotoCaptureAsync(&format)?.get()?;
    let capture_result = (|| -> Result<()> {
        let mut records = vec![];
        // Warm up through actual photo acquisitions, then retain ten samples.
        for index in 0..13 {
            let start = Instant::now();
            let photo = photos.CaptureAsync()?.get()?;
            let frame = photo.Frame()?;
            let size = u32::try_from(frame.Size()?)?;
            let reader = DataReader::CreateDataReader(&frame.GetInputStreamAt(0)?)?;
            if reader.LoadAsync(size)?.get()? != size {
                return Err("Incomplete photo read".into());
            }
            let mut bytes = vec![0; size as usize];
            reader.ReadBytes(&mut bytes)?;
            reader.Close()?;
            if index >= 3 {
                let filename = format!("photo-{:02}.jpg", index - 3);
                fs::write(out.join(&filename), &bytes)?;
                let controls = frame.ControlValues().ok();
                records.push(json!({"file": filename, "bytes": size,
                    "capture_and_read_ms": start.elapsed().as_secs_f64()*1000.0,
                    "frame_metadata": {
                        "exposure_100ns": controls.as_ref().and_then(|c| c.Exposure().ok()).and_then(|v| v.Value().ok()).map(|v| v.Duration),
                        "iso_speed": controls.as_ref().and_then(|c| c.IsoSpeed().ok()).and_then(|v| v.Value().ok()),
                        "white_balance_kelvin": controls.as_ref().and_then(|c| c.WhiteBalance().ok()).and_then(|v| v.Value().ok()),
                    }, "note": "Null metadata means unavailable. API-reported applied values are not independently verified."}));
                save(&out.join("photos.json"), &json!(records))?;
                eprintln!("Saved {filename}");
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        Ok(())
    })();
    let finish_result = photos.FinishAsync()?.get();
    capture_result?;
    finish_result?;
    Ok(())
}

fn main() -> Result<()> {
    let out = std::env::args()
        .nth(1)
        .ok_or("Usage: native-camera-probe OUTDIR")?;
    // Bound driver stalls in this research executable, including async waits.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(120));
        eprintln!("Native probe exceeded 120 seconds; terminating to release camera handles");
        std::process::exit(124);
    });
    if std::env::args().nth(2).as_deref() == Some("native") {
        native::run(Path::new(&out))
    } else {
        run(Path::new(&out))
    }
}
