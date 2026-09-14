use crate::{Result, save};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Foundation::{
        AsyncActionCompletedHandler, AsyncOperationCompletedHandler, IAsyncAction, IAsyncOperation,
    },
    Win32::{
        Storage::Packaging::Appx::GetCurrentPackageFamilyName,
        System::{Com::*, RemoteDesktop::ProcessIdToSessionId},
        UI::WindowsAndMessaging::*,
    },
    core::{PWSTR, RuntimeType},
};

pub fn apartment() -> Value {
    let mut kind = APTTYPE::default();
    let mut qualifier = APTTYPEQUALIFIER::default();
    match unsafe { CoGetApartmentType(&mut kind, &mut qualifier) } {
        Ok(()) => json!({"type":kind.0,"qualifier":qualifier.0,"name":format!("{kind:?}")}),
        Err(e) => json!({"hresult":format!("0x{:08X}",e.code().0 as u32)}),
    }
}
pub fn environment(out: &Path) -> Result<()> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("environment.ps1");
    let observation = std::process::Command::new("pwsh")
        .args(["-NoProfile", "-File"])
        .arg(script)
        .arg("-ProbeProcessId")
        .arg(std::process::id().to_string())
        .output()?;
    if !observation.status.success() {
        return Err(String::from_utf8_lossy(&observation.stderr)
            .into_owned()
            .into());
    }
    let host: Value = serde_json::from_slice(&observation.stdout)?;
    save(&out.join("host-environment.json"), &host)?;
    let mut session = 0;
    let session_result = unsafe { ProcessIdToSessionId(std::process::id(), &mut session) };
    let mut length = 0;
    let package_status = unsafe { GetCurrentPackageFamilyName(&mut length, PWSTR::null()) };
    let mut buffer = vec![0u16; length as usize];
    let package = if length > 0
        && unsafe { GetCurrentPackageFamilyName(&mut length, PWSTR(buffer.as_mut_ptr())) }.0 == 0
    {
        Some(String::from_utf16_lossy(
            &buffer[..length.saturating_sub(1) as usize],
        ))
    } else {
        None
    };
    save(
        &out.join("environment.json"),
        &json!({"apartment":apartment(),"pid":std::process::id(),"session_id":session_result.ok().map(|_|session),"executable":std::env::current_exe()?.display().to_string(),"package_family_name":package,"package_query_win32_status":package_status.0,"note":"15700 means APPMODEL_ERROR_NO_PACKAGE; host token/OS/policy observations are in host-environment.json."}),
    )
}
pub fn stage(out: &Path, name: &str) -> Result<()> {
    let row = json!({"stage":name,"apartment":apartment()});
    eprintln!("{row}");
    save(&out.join("last-stage.json"), &row)
}
pub fn call<T>(out: &Path, name: &str, result: windows::core::Result<T>) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(e) => {
            save(
                &out.join("failure.json"),
                &json!({"stage":name,"hresult":format!("0x{:08X}",e.code().0 as u32),"message":e.to_string(),"apartment":apartment()}),
            )?;
            Err(e.into())
        }
    }
}
fn pump_until(done: &AtomicBool) -> windows::core::Result<()> {
    let start = Instant::now();
    while !done.load(Ordering::Acquire) {
        if start.elapsed() > Duration::from_secs(30) {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x800705B4u32 as i32,
            )));
        }
        unsafe {
            // Bounded message-aware wait; completion is signaled by the WinRT callback.
            MsgWaitForMultipleObjectsEx(None, 20, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    Ok(())
}
pub trait Pump {
    type Output;
    fn pump(&self) -> windows::core::Result<Self::Output>;
}

pub struct ResearchWindow(windows::Win32::Foundation::HWND);
impl ResearchWindow {
    pub fn new() -> windows::core::Result<Self> {
        let handle = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::w!("STATIC"),
                windows::core::w!("Cirdean CM678 research: testing camera initialization"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                560,
                120,
                None,
                None,
                None,
                None,
            )
        };
        if handle.0 == 0 {
            return Err(windows::core::Error::from_win32());
        }
        Ok(Self(handle))
    }
}
impl Drop for ResearchWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}
impl Pump for IAsyncAction {
    type Output = ();
    fn pump(&self) -> windows::core::Result<()> {
        let done = Arc::new(AtomicBool::new(false));
        let signal = done.clone();
        self.SetCompleted(&AsyncActionCompletedHandler::new(move |_, _| {
            signal.store(true, Ordering::Release);
            Ok(())
        }))?;
        if let Err(e) = pump_until(&done) {
            let _ = self.Cancel();
            return Err(e);
        }
        self.GetResults()
    }
}
impl<T: RuntimeType + 'static> Pump for IAsyncOperation<T> {
    type Output = T;
    fn pump(&self) -> windows::core::Result<T> {
        let done = Arc::new(AtomicBool::new(false));
        let signal = done.clone();
        self.SetCompleted(&AsyncOperationCompletedHandler::<T>::new(move |_, _| {
            signal.store(true, Ordering::Release);
            Ok(())
        }))?;
        if let Err(e) = pump_until(&done) {
            let _ = self.Cancel();
            return Err(e);
        }
        self.GetResults()
    }
}
