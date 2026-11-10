//! Audio reconnect through the Bluetooth audio driver.
//!
//! Buds that are connected to the phone only show up on this PC once the PC's audio profile
//! connects. The driver exposes a "one-shot reconnect" kernel-streaming property on the Bluetooth
//! audio endpoint's filter (the same call the Windows volume flyout uses when you click an
//! earbud entry). We find the endpoints that belong to the buds and send it.
//!
//! The endpoint -> Bluetooth filter hop: an audio endpoint's connector is wired to a device topology
//! (`IConnector::GetDeviceIdConnectedTo`); for Bluetooth audio that id names the Bluetooth bus device
//! and contains the buds' address as twelve lowercase hex digits.

use windows::Win32::Media::Audio::{
    DEVICE_STATE, DEVICE_STATEMASK_ALL, IDeviceTopology, IMMDeviceEnumerator, MMDeviceEnumerator,
    eCapture, eRender,
};
use windows::Win32::Media::KernelStreaming::{
    IKsControl, KSIDENTIFIER, KSIDENTIFIER_0, KSIDENTIFIER_0_0, KSPROPERTY_TYPE_GET,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize,
};
use windows::core::{GUID, PCWSTR, PWSTR};

/// `KSPROPSETID_BtAudio`; property 0 is `KSPROPERTY_ONESHOT_RECONNECT`.
const KSPROPSETID_BT_AUDIO: GUID = GUID::from_u128(0x7fa06c40_b8f6_4c7e_8556_e8c33a12e54d);
const KSPROPERTY_ONESHOT_RECONNECT: u32 = 0;

/// One audio endpoint that belongs to the buds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// "render" (headphones) or "capture" (microphone).
    pub flow: &'static str,
    pub id: String,
    /// Hex of the `DEVICE_STATE_*` mask: 1 active, 2 disabled, 4 not present, 8 unplugged.
    pub state: u32,
    /// The Bluetooth filter this endpoint is wired to.
    pub filter: String,
}

/// Runs `work` on this thread with COM initialised.
fn with_com<T>(work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    // SAFETY: balanced with CoUninitialize below when the call succeeded.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let initialised = hr.is_ok(); // S_OK or S_FALSE (already initialised: still counted)
    let result = work();
    if initialised {
        // SAFETY: matches the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    result
}

fn take(text: PWSTR) -> String {
    // SAFETY: COM returned a NUL-terminated wide string that we own and free.
    let s = unsafe { text.to_string() }.unwrap_or_default();
    // SAFETY: allocated by the COM task allocator.
    unsafe { CoTaskMemFree(Some(text.0 as *const _)) };
    s
}

fn hex_address(address: [u8; 6]) -> String {
    address.iter().map(|b| format!("{b:02x}")).collect()
}

/// The audio endpoints (any state) wired to the Bluetooth device at `address`.
pub fn endpoints(address: [u8; 6]) -> Result<Vec<Endpoint>, String> {
    with_com(|| list(address))
}

fn list(address: [u8; 6]) -> Result<Vec<Endpoint>, String> {
    let needle = hex_address(address);
    // SAFETY: COM calls on a thread where COM is initialised; every returned interface is released on drop.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("audio enumerator: {e}"))?;
        let mut found = Vec::new();
        for (flow, label) in [(eRender, "render"), (eCapture, "capture")] {
            let Ok(collection) =
                enumerator.EnumAudioEndpoints(flow, DEVICE_STATE(DEVICE_STATEMASK_ALL))
            else {
                continue;
            };
            let count = collection.GetCount().unwrap_or(0);
            for index in 0..count {
                let Ok(device) = collection.Item(index) else {
                    continue;
                };
                let Ok(id) = device.GetId().map(take) else {
                    continue;
                };
                let state = device.GetState().map_or(0, |s| s.0);
                let Ok(topology) = device.Activate::<IDeviceTopology>(CLSCTX_ALL, None) else {
                    continue;
                };
                let Ok(connector) = topology.GetConnector(0) else {
                    continue;
                };
                let Ok(filter) = connector.GetDeviceIdConnectedTo().map(take) else {
                    continue;
                };
                if filter.to_lowercase().contains(&needle) {
                    found.push(Endpoint {
                        flow: label,
                        id,
                        state,
                        filter,
                    });
                }
            }
        }
        Ok(found)
    }
}

/// Sends the one-shot reconnect to the buds' Bluetooth audio endpoints. Returns how many accepted it.
pub fn reconnect(address: [u8; 6]) -> Result<usize, String> {
    with_com(|| {
        let found = list(address)?;
        if found.is_empty() {
            return Err("no audio endpoint for the buds (not paired for audio?)".into());
        }
        let mut accepted = 0;
        let mut last_error = String::new();
        let mut tried: Vec<String> = Vec::new();
        for endpoint in &found {
            if tried.contains(&endpoint.filter) {
                continue; // the render and capture endpoints share one filter
            }
            tried.push(endpoint.filter.clone());
            match send_reconnect(&endpoint.filter) {
                Ok(()) => accepted += 1,
                Err(e) => last_error = e,
            }
        }
        if accepted > 0 {
            Ok(accepted)
        } else {
            Err(last_error)
        }
    })
}

fn send_reconnect(filter: &str) -> Result<(), String> {
    let wide: Vec<u16> = filter.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: COM is initialised by the caller; `wide` is NUL-terminated and outlives the calls.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("audio enumerator: {e}"))?;
        let device = enumerator
            .GetDevice(PCWSTR(wide.as_ptr()))
            .map_err(|e| format!("bluetooth audio filter: {e}"))?;
        let control: IKsControl = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("kernel streaming control: {e}"))?;
        let property = KSIDENTIFIER {
            Anonymous: KSIDENTIFIER_0 {
                Anonymous: KSIDENTIFIER_0_0 {
                    Set: KSPROPSETID_BT_AUDIO,
                    Id: KSPROPERTY_ONESHOT_RECONNECT,
                    Flags: KSPROPERTY_TYPE_GET,
                },
            },
        };
        let mut returned = 0u32;
        control
            .KsProperty(
                &property,
                std::mem::size_of::<KSIDENTIFIER>() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
            )
            .map_err(|e| format!("reconnect request refused: {e}"))
    }
}

/// Readable lines for `budsctl audio`.
pub fn report(address: [u8; 6]) -> Vec<String> {
    match endpoints(address) {
        Err(e) => vec![e],
        Ok(list) if list.is_empty() => {
            vec!["no audio endpoints for these buds (the PC has no audio profile for them)".into()]
        }
        Ok(list) => list
            .iter()
            .map(|e| {
                let state = match e.state {
                    1 => "active",
                    2 => "disabled",
                    4 => "not present",
                    8 => "unplugged",
                    _ => "other",
                };
                format!("{} endpoint, {state}: {}", e.flow, e.filter)
            })
            .collect(),
    }
}
