use crate::{Result, save};
use serde_json::{Value, json};
use std::{fs, path::Path, time::Instant};
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::{
            Com::CoTaskMemFree,
            WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
        },
    },
    core::{GUID, Interface},
};

fn attributes() -> windows::core::Result<IMFAttributes> {
    let mut attributes = None;
    unsafe {
        MFCreateAttributes(&mut attributes, 8)?;
    }
    Ok(attributes.expect("MFCreateAttributes succeeded without object"))
}

fn string(a: &IMFAttributes, key: &GUID) -> windows::core::Result<String> {
    unsafe {
        let mut text = vec![0u16; a.GetStringLength(key)? as usize + 1];
        let mut length = 0;
        a.GetString(key, &mut text, Some(&mut length))?;
        Ok(String::from_utf16_lossy(&text[..length as usize]))
    }
}

fn describe(a: &IMFMediaType) -> Value {
    unsafe {
        let size = a.GetUINT64(&MF_MT_FRAME_SIZE).ok();
        let rate = a.GetUINT64(&MF_MT_FRAME_RATE).ok();
        let subtype = a.GetGUID(&MF_MT_SUBTYPE).ok();
        json!({"subtype": subtype.map(|v| format!("{v:?}")),
            "subtype_name": subtype.map(|v| if v == MFVideoFormat_MJPG {"MJPG"} else if v == MFVideoFormat_YUY2 {"YUY2"} else {"other"}),
            "width": size.map(|v| v >> 32), "height": size.map(|v| v & 0xffff_ffff),
            "fps_numerator": rate.map(|v| v >> 32), "fps_denominator": rate.map(|v| v & 0xffff_ffff),
            "sample_size": a.GetUINT32(&MF_MT_SAMPLE_SIZE).ok(),
            "stride": a.GetUINT32(&MF_MT_DEFAULT_STRIDE).ok().map(|v| v as i32),
            "nominal_range": a.GetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE).ok(),
            "yuv_matrix": a.GetUINT32(&MF_MT_YUV_MATRIX).ok(),
            "primaries": a.GetUINT32(&MF_MT_VIDEO_PRIMARIES).ok(),
            "transfer_function": a.GetUINT32(&MF_MT_TRANSFER_FUNCTION).ok(),
            "chroma_siting": a.GetUINT32(&MF_MT_VIDEO_CHROMA_SITING).ok()})
    }
}

struct SourceGuard(IMFMediaSource);
impl Drop for SourceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = self.0.Shutdown();
        }
    }
}
struct MfGuard;
impl Drop for MfGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

pub fn run(out: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
    }
    let _mf = MfGuard;
    let filter = attributes()?;
    unsafe {
        filter.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )?;
    }
    let mut array = std::ptr::null_mut();
    let mut count = 0;
    eprintln!("Enumerating MF device sources");
    unsafe {
        MFEnumDeviceSources(&filter, &mut array, &mut count)?;
    }
    // Transfer COM references out, then free the COM-allocated pointer array.
    let devices: Vec<_> = if count == 0 {
        vec![]
    } else {
        unsafe {
            std::slice::from_raw_parts_mut(array, count as usize)
                .iter_mut()
                .filter_map(Option::take)
                .collect()
        }
    };
    unsafe {
        CoTaskMemFree(Some(array.cast()));
    }
    let mut inventory = vec![];
    let mut selected = None;
    for device in devices {
        let attrs = device.cast::<IMFAttributes>()?;
        let id = string(
            &attrs,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
        )?;
        inventory.push(
            json!({"id": id, "name": string(&attrs, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME).ok()}),
        );
        if id.to_lowercase().contains("vid_0c45&pid_6369&mi_00") {
            if selected.is_some() {
                return Err("Multiple matching cameras".into());
            }
            selected = Some(device);
        }
    }
    save(&out.join("mf-devices.json"), &json!(inventory))?;
    let device = selected.ok_or("CM678 missing from MF enumeration")?;
    eprintln!("Activating selected MF camera, converters disabled");
    let source = SourceGuard(unsafe {
        device
            .ActivateObject::<IMFMediaSource>()
            .map_err(|e| format!("ActivateObject: {e}"))?
    });
    eprintln!("MF camera activated; creating Source Reader");
    let config = attributes()?;
    unsafe {
        config.SetUINT32(&MF_READWRITE_DISABLE_CONVERTERS, 1)?;
    }
    let reader = unsafe {
        MFCreateSourceReaderFromMediaSource(&source.0, &config)
            .map_err(|e| format!("CreateSourceReader: {e}"))?
    };
    let streams = unsafe {
        source
            .0
            .CreatePresentationDescriptor()?
            .GetStreamDescriptorCount()?
    };
    let mut modes = vec![];
    let mut chosen = None;
    for stream in 0..streams {
        for index in 0..512 {
            let mode = match unsafe { reader.GetNativeMediaType(stream, index) } {
                Ok(mode) => mode,
                Err(error) if error.code() == MF_E_NO_MORE_TYPES => break,
                Err(error) => return Err(error.into()),
            };
            let row = describe(&mode);
            if row["subtype_name"] == "MJPG"
                && row["width"] == 1920
                && row["height"] == 1080
                && row["fps_numerator"]
                    .as_u64()
                    .zip(row["fps_denominator"].as_u64())
                    .is_some_and(|(n, d)| d > 0 && n == 30 * d)
            {
                chosen = Some((stream, mode));
            }
            modes.push(json!({"stream": stream, "index": index, "type": row}));
        }
    }
    save(&out.join("mf-native-types.json"), &json!(modes))?;
    let (stream, mode) = chosen.ok_or("Native MJPEG 1080p30 unavailable")?;
    unsafe {
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(stream, true)?;
        reader.SetCurrentMediaType(stream, None, &mode)?;
    }
    save(
        &out.join("mf-current-type.json"),
        &describe(&unsafe { reader.GetCurrentMediaType(stream)? }),
    )?;
    eprintln!("Reading native MJPEG samples");
    let started = Instant::now();
    let mut records = vec![];
    let mut samples = 0;
    for _ in 0..300 {
        let mut sample = None;
        let mut flags = 0;
        let mut timestamp = 0;
        unsafe {
            reader.ReadSample(
                stream,
                0,
                None,
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            )?;
        }
        if flags != 0 {
            eprintln!("Source Reader flags: {flags:#x}");
            if flags & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32)
                != 0
            {
                return Err("Source Reader stopped".into());
            }
        }
        let Some(sample) = sample else {
            continue;
        };
        if started.elapsed().as_secs_f64() < 3.0 {
            continue;
        }
        let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
        let mut ptr = std::ptr::null_mut();
        let mut length = 0;
        unsafe {
            buffer.Lock(&mut ptr, None, Some(&mut length))?;
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr, length as usize).to_vec() };
        unsafe {
            buffer.Unlock()?;
        }
        let filename = format!("native-{samples:02}.jpg");
        fs::write(out.join(&filename), &bytes)?;
        let metadata = unsafe {
            sample
                .GetUnknown::<IMFAttributes>(&MFSampleExtension_CaptureMetadata)
                .ok()
        };
        records.push(json!({"file": filename, "bytes": length, "timestamp_100ns": timestamp, "flags": flags,
            "metadata_present": metadata.is_some(),
            "applied_exposure_100ns": metadata.as_ref().and_then(|m| unsafe { m.GetUINT64(&MF_CAPTURE_METADATA_EXPOSURE_TIME).ok() }),
            "sensor_framerate_packed": metadata.as_ref().and_then(|m| unsafe { m.GetUINT64(&MF_CAPTURE_METADATA_SENSORFRAMERATE).ok() })}));
        samples += 1;
        if samples == 10 {
            break;
        }
    }
    save(&out.join("native-samples.json"), &json!(records))?;
    if samples != 10 {
        return Err("Insufficient post-warmup samples".into());
    }
    eprintln!("Saved {samples} native JPEG samples and metadata");
    Ok(())
}
