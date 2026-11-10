//! Listing the paired Bluetooth devices through the classic `BluetoothAPIs`.

use buds_core::platform::BtDevice;
use windows::Win32::Devices::Bluetooth::{
    BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BluetoothFindDeviceClose,
    BluetoothFindFirstDevice, BluetoothFindNextDevice,
};
use windows::Win32::Foundation::HANDLE;
use windows::core::BOOL;

/// Paired (authenticated / remembered) and currently connected devices of every local radio.
/// Never starts a radio inquiry (`fIssueInquiry = FALSE`), so it is cheap enough to run every few
/// seconds and never disturbs audio. Blocking: call from a blocking thread.
pub fn list_paired() -> Vec<BtDevice> {
    let size = std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: std::mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: BOOL(1),
        fReturnRemembered: BOOL(1),
        fReturnUnknown: BOOL(0),
        fReturnConnected: BOOL(1),
        fIssueInquiry: BOOL(0),
        cTimeoutMultiplier: 0,
        hRadio: HANDLE::default(),
    };
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: size,
        ..Default::default()
    };
    let mut devices: Vec<BtDevice> = Vec::new();

    // SAFETY: both structs are fully initialised with correct `dwSize` values and outlive the calls;
    // the find handle is closed exactly once.
    unsafe {
        let Ok(find) = BluetoothFindFirstDevice(&params, &mut info) else {
            return devices; // no radio, or nothing paired
        };
        loop {
            push(&mut devices, &info);
            info = BLUETOOTH_DEVICE_INFO {
                dwSize: size,
                ..Default::default()
            };
            if BluetoothFindNextDevice(find, &mut info).is_err() {
                break;
            }
        }
        let _ = BluetoothFindDeviceClose(find);
    }
    devices
}

fn push(devices: &mut Vec<BtDevice>, info: &BLUETOOTH_DEVICE_INFO) {
    // SAFETY: reading the integer view of the address union; every bit pattern is valid.
    let raw = unsafe { info.Address.Anonymous.ullLong };
    let address = address_bytes(raw);
    let name_len = info
        .szName
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(info.szName.len());
    let name = String::from_utf16_lossy(&info.szName[..name_len]);
    let connected = info.fConnected.as_bool();
    // The same device can be returned for several of the search flags.
    if let Some(existing) = devices.iter_mut().find(|d| d.address == address) {
        existing.connected |= connected;
        if existing.name.is_none() && !name.is_empty() {
            existing.name = Some(name);
        }
        return;
    }
    devices.push(BtDevice {
        address,
        name: (!name.is_empty()).then_some(name),
        connected,
        // Vendor service records aren't visible cheaply on Windows; the name and the list of
        // addresses we have talked to before identify the buds instead.
        has_control_service: false,
    });
}

/// `BLUETOOTH_ADDRESS.ullLong` holds the 48-bit address with the first octet in the top bytes.
pub fn address_bytes(raw: u64) -> [u8; 6] {
    [
        (raw >> 40) as u8,
        (raw >> 32) as u8,
        (raw >> 24) as u8,
        (raw >> 16) as u8,
        (raw >> 8) as u8,
        raw as u8,
    ]
}

/// The inverse of [`address_bytes`], for socket addresses.
pub fn address_u64(address: [u8; 6]) -> u64 {
    address
        .iter()
        .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_conversion_round_trips() {
        let raw = 0x0000_1234_5678_9ABC_u64;
        assert_eq!(address_bytes(raw), [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);
        assert_eq!(address_u64([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]), raw);
    }
}
