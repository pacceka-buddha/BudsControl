//! Service discovery through Winsock (`NS_BTH`): which services the buds advertise and on which
//! RFCOMM channel. Used by `budsctl sdp` and to show the control channel in the Developer tab.

use buds_core::platform::format_address;

use super::rfcomm::{HEYMELODY, HEYMELODY_ALT, SPP, init_winsock};
use windows::Win32::Devices::Bluetooth::SOCKADDR_BTH;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Networking::WinSock::{
    LUP_FLUSHCACHE, LUP_RETURN_ADDR, LUP_RETURN_NAME, LUP_RETURN_TYPE, SOCKADDR, WSAGetLastError,
    WSALookupServiceBeginW, WSALookupServiceEnd, WSALookupServiceNextW, WSAQUERYSETW,
};
use windows::core::{GUID, PWSTR};

const NS_BTH: u32 = 16;
/// "No more results" and "buffer too small".
const WSA_E_NO_MORE: i32 = 10110;
const WSAEFAULT: i32 = 10014;

/// Every service that runs over RFCOMM (the query returns the RFCOMM channel of each).
pub const RFCOMM_PROTOCOL: GUID = GUID::from_u128(0x00000003_0000_1000_8000_00805F9B34FB);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    /// The RFCOMM channel, if the result carries one.
    pub channel: Option<u8>,
}

/// Looks up the services of the device at `address` that belong to `class` (a service class UUID or a
/// protocol UUID). `Err` carries the Winsock error code (10108 = nothing found).
pub fn lookup(address: [u8; 6], class: GUID) -> Result<Vec<Service>, i32> {
    init_winsock();
    let mut context: Vec<u16> = format!("({})", format_address(&address))
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut class = class;
    let query = WSAQUERYSETW {
        dwSize: std::mem::size_of::<WSAQUERYSETW>() as u32,
        dwNameSpace: NS_BTH,
        lpServiceClassId: &mut class,
        lpszContext: PWSTR(context.as_mut_ptr()),
        ..Default::default()
    };
    let flags = LUP_FLUSHCACHE | LUP_RETURN_NAME | LUP_RETURN_ADDR | LUP_RETURN_TYPE;

    let mut handle = HANDLE::default();
    // SAFETY: `query` points at live data (`class`, `context`) for the duration of the call.
    if unsafe { WSALookupServiceBeginW(&query, flags, &mut handle) } != 0 {
        return Err(last_error());
    }

    let mut found: Vec<Service> = Vec::new();
    // The result is a WSAQUERYSETW followed by the data it points to; 8-byte aligned storage.
    let mut buffer: Vec<u64> = vec![0; 4096 / 8];
    let outcome = loop {
        let mut length = (buffer.len() * 8) as u32;
        // SAFETY: `buffer` is writable for `length` bytes and suitably aligned.
        let rc = unsafe {
            WSALookupServiceNextW(
                handle,
                flags,
                &mut length,
                Some(buffer.as_mut_ptr().cast::<WSAQUERYSETW>()),
            )
        };
        if rc == 0 {
            // SAFETY: the call succeeded, so the buffer holds a valid result set.
            let service = unsafe { parse(buffer.as_ptr().cast::<WSAQUERYSETW>()) };
            if !found.contains(&service) {
                found.push(service);
            }
            continue;
        }
        match last_error() {
            WSA_E_NO_MORE => break Ok(()),
            WSAEFAULT if (length as usize) > buffer.len() * 8 => {
                buffer = vec![0; (length as usize).div_ceil(8)];
            }
            other => break Err(other),
        }
    };
    // SAFETY: closes the lookup handle opened above.
    unsafe { WSALookupServiceEnd(handle) };
    outcome.map(|()| found)
}

fn last_error() -> i32 {
    // SAFETY: plain thread-local error read.
    unsafe { WSAGetLastError().0 }
}

/// # Safety
/// `set` must point at a valid result set returned by `WSALookupServiceNextW`.
unsafe fn parse(set: *const WSAQUERYSETW) -> Service {
    // SAFETY: guaranteed by the caller.
    let set = unsafe { &*set };
    let name = if set.lpszServiceInstanceName.is_null() {
        String::new()
    } else {
        // SAFETY: a non-null instance name is a NUL-terminated wide string inside the buffer.
        unsafe { set.lpszServiceInstanceName.to_string() }.unwrap_or_default()
    };
    let mut channel = None;
    if set.dwNumberOfCsAddrs > 0 && !set.lpcsaBuffer.is_null() {
        // SAFETY: `lpcsaBuffer` points at `dwNumberOfCsAddrs` entries inside the buffer.
        let remote = unsafe { (*set.lpcsaBuffer).RemoteAddr };
        if !remote.lpSockaddr.is_null()
            && remote.iSockaddrLength as usize >= std::mem::size_of::<SOCKADDR_BTH>()
        {
            // SAFETY: the address is a SOCKADDR_BTH of at least that size; it is packed, so read unaligned.
            let addr = unsafe {
                std::ptr::read_unaligned(
                    remote.lpSockaddr.cast::<SOCKADDR>().cast::<SOCKADDR_BTH>(),
                )
            };
            let port = addr.port;
            channel = u8::try_from(port).ok().filter(|p| *p != 0);
        }
    }
    Service { name, channel }
}

/// The RFCOMM channel of the first service of `class` that reports one.
pub fn channel_of(address: [u8; 6], class: GUID) -> Option<u8> {
    lookup(address, class)
        .ok()?
        .into_iter()
        .find_map(|service| service.channel)
}

/// A readable report of the control services of the buds at `address`, one line per finding.
pub fn report(address: [u8; 6]) -> Vec<String> {
    let classes: [(&str, GUID); 4] = [
        ("HeyMelody 079A", HEYMELODY),
        ("HeyMelody 1107", HEYMELODY_ALT),
        ("Serial port 1101", SPP),
        ("any RFCOMM service", RFCOMM_PROTOCOL),
    ];
    let mut lines = Vec::new();
    for (label, class) in classes {
        match lookup(address, class) {
            Ok(services) if services.is_empty() => lines.push(format!("{label}: none")),
            Ok(services) => {
                for service in services {
                    let name = if service.name.is_empty() {
                        "(unnamed)"
                    } else {
                        service.name.as_str()
                    };
                    let channel = service
                        .channel
                        .map_or("no channel".to_string(), |c| format!("channel {c}"));
                    lines.push(format!("{label}: {name}, {channel}"));
                }
            }
            Err(10108) => lines.push(format!("{label}: not advertised")),
            Err(code) => lines.push(format!("{label}: {}", super::rfcomm::describe(code))),
        }
    }
    lines
}
