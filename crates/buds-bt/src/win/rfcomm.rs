//! RFCOMM over Winsock (`AF_BTH` / `BTHPROTO_RFCOMM`): connecting with a timeout, and a link that
//! reads on its own thread.
//!
//! Socket lifetime: the reader thread owns closing the socket. `close()` only calls `shutdown`
//! (which wakes the blocked `recv`); the reader then closes the handle under the mutex, so a
//! handle value is never used after it was closed and possibly reused by another socket.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use buds_core::platform::{Link, LinkSink};
use windows::Win32::Devices::Bluetooth::{AF_BTH, BTHPROTO_RFCOMM, SOCKADDR_BTH};
use windows::Win32::Networking::WinSock::{
    SD_BOTH, SEND_RECV_FLAGS, SOCK_STREAM, SOCKADDR, SOCKET, SOCKET_ERROR, WSADATA,
    WSAGetLastError, WSAStartup, closesocket, connect, getpeername, recv, send, shutdown, socket,
};
use windows::core::GUID;

use super::devices::address_u64;
use crate::plan::{Candidate, ConnectError};

/// HeyMelody control service UUIDs; the buds advertise one or the other.
pub(super) const HEYMELODY: GUID = GUID::from_u128(0x0000079A_D102_11E1_9B23_00025B00A5A5);
pub(super) const HEYMELODY_ALT: GUID = GUID::from_u128(0x00001107_D102_11E1_9B23_00025B00A5A5);
/// Standard Serial Port Profile.
pub(super) const SPP: GUID = GUID::from_u128(0x00001101_0000_1000_8000_00805F9B34FB);

pub(super) fn init_winsock() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut data = WSADATA::default();
        // SAFETY: `data` is a valid out-parameter. Never paired with WSACleanup: process lifetime.
        unsafe {
            WSAStartup(0x0202, &mut data);
        }
    });
}

fn last_error() -> i32 {
    // SAFETY: plain thread-local error read.
    unsafe { WSAGetLastError().0 }
}

fn connect_error(code: i32) -> ConnectError {
    ConnectError {
        code,
        message: describe(code),
        timed_out: false,
        cancelled: false,
    }
}

/// Names for the Winsock errors we expect from Bluetooth connects.
pub fn describe(code: i32) -> String {
    match code {
        10108 => "service not found".into(),
        10049 => "address not available".into(),
        10022 => "invalid argument".into(),
        10060 => "connection timed out".into(),
        10061 => "connection refused".into(),
        10064 => "host is down".into(),
        10065 => "no route to host".into(),
        10038 => "socket closed".into(),
        10004 => "interrupted".into(),
        other => format!("winsock error {other}"),
    }
}

/// An open, connected RFCOMM socket and where it connected.
pub struct Connected {
    sock: SOCKET,
    pub channel: Option<u8>,
}

/// Connects to the buds through `candidate`. Blocking; gives up after the candidate's timeout, or
/// at once (and closes the socket) when `cancelled` is set.
pub fn connect_candidate(
    address: [u8; 6],
    candidate: Candidate,
    cancelled: &AtomicBool,
) -> Result<Connected, ConnectError> {
    init_winsock();
    // SAFETY: plain socket creation.
    let sock = unsafe { socket(i32::from(AF_BTH), SOCK_STREAM, BTHPROTO_RFCOMM as i32) }
        .map_err(|e| connect_error(e.code().0 & 0xFFFF))?;

    let mut addr = SOCKADDR_BTH {
        addressFamily: AF_BTH,
        btAddr: address_u64(address),
        ..Default::default()
    };
    match candidate {
        // With a service class and port 0, Windows does the SDP lookup for the RFCOMM channel.
        Candidate::HeyMelody => addr.serviceClassId = HEYMELODY,
        Candidate::HeyMelodyAlt => addr.serviceClassId = HEYMELODY_ALT,
        Candidate::Spp => addr.serviceClassId = SPP,
        Candidate::Channel(n) => addr.port = u32::from(n),
    }

    // The connect blocks (and can take a long time for unreachable buds), so it runs on its own
    // thread while this one watches the clock. Closing the socket aborts a pending connect.
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = sock.0;
    std::thread::spawn(move || {
        let sock = SOCKET(handle);
        // SAFETY: `addr` is a fully initialised SOCKADDR_BTH, passed with its exact size.
        let rc = unsafe {
            connect(
                sock,
                std::ptr::addr_of!(addr).cast::<SOCKADDR>(),
                std::mem::size_of::<SOCKADDR_BTH>() as i32,
            )
        };
        let _ = tx.send(if rc == SOCKET_ERROR {
            Err(last_error())
        } else {
            Ok(())
        });
    });

    let deadline = Instant::now() + candidate.timeout();
    let outcome = loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(result) => break Some(result),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break Some(Err(10038)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    break None;
                }
            }
        }
    };

    match outcome {
        Some(Ok(())) => {
            if cancelled.load(Ordering::Relaxed) {
                // SAFETY: we own the socket and nothing else uses it.
                unsafe { closesocket(sock) };
                return Err(ConnectError::cancelled());
            }
            Ok(Connected {
                sock,
                channel: peer_channel(sock).or(match candidate {
                    Candidate::Channel(n) => Some(n),
                    // Winsock did the lookup itself; ask again so the Developer tab can show it.
                    Candidate::HeyMelody => super::sdp::channel_of(address, HEYMELODY),
                    Candidate::HeyMelodyAlt => super::sdp::channel_of(address, HEYMELODY_ALT),
                    Candidate::Spp => super::sdp::channel_of(address, SPP),
                }),
            })
        }
        Some(Err(code)) => {
            // SAFETY: the connect thread is done with the socket.
            unsafe { closesocket(sock) };
            Err(connect_error(code))
        }
        None => {
            // Abort the pending connect; the thread then returns on its own.
            // SAFETY: closing a socket another thread is blocked on is the documented way to abort it.
            unsafe { closesocket(sock) };
            Err(if cancelled.load(Ordering::Relaxed) {
                ConnectError::cancelled()
            } else {
                ConnectError::timeout()
            })
        }
    }
}

/// The RFCOMM channel the socket actually connected to (after the SDP lookup).
fn peer_channel(sock: SOCKET) -> Option<u8> {
    let mut addr = SOCKADDR_BTH::default();
    let mut len = std::mem::size_of::<SOCKADDR_BTH>() as i32;
    // SAFETY: `addr` and `len` are valid for writes of the stated size.
    let rc = unsafe {
        getpeername(
            sock,
            std::ptr::addr_of_mut!(addr).cast::<SOCKADDR>(),
            &mut len,
        )
    };
    if rc == SOCKET_ERROR {
        return None;
    }
    let port = addr.port; // copy out of the packed struct
    u8::try_from(port).ok().filter(|p| *p != 0)
}

impl Connected {
    /// Closes the socket (used when a page only needs the connection to happen).
    pub fn close(self) {
        // SAFETY: we own the socket.
        unsafe {
            shutdown(self.sock, SD_BOTH);
            closesocket(self.sock);
        }
    }

    /// Starts the reader thread and returns the link.
    pub fn into_link(self, hint: Candidate, sink: LinkSink) -> WinLink {
        let state = Arc::new(State {
            sock: Mutex::new(Some(self.sock)),
            closed: AtomicBool::new(false),
        });
        let reader_state = state.clone();
        let handle = self.sock.0;
        std::thread::Builder::new()
            .name("buds-rfcomm-reader".into())
            .spawn(move || read_loop(SOCKET(handle), &reader_state, &sink))
            .expect("spawn reader thread");
        WinLink {
            state,
            channel: self.channel,
            hint,
        }
    }
}

struct State {
    /// `None` once the reader has closed the socket.
    sock: Mutex<Option<SOCKET>>,
    /// We closed it on purpose (so the reader must not report a remote close).
    closed: AtomicBool,
}

fn read_loop(sock: SOCKET, state: &State, sink: &LinkSink) {
    let mut buf = [0u8; 1024];
    loop {
        // SAFETY: `buf` is valid for writes; the handle stays open until this thread closes it.
        let n = unsafe { recv(sock, &mut buf, SEND_RECV_FLAGS(0)) };
        if n <= 0 {
            break;
        }
        sink.data(buf[..n as usize].to_vec());
    }
    let deliberate = state.closed.swap(true, Ordering::SeqCst);
    if let Some(s) = state.sock.lock().unwrap().take() {
        // SAFETY: taken out of the mutex, so nobody else can use or close it.
        unsafe { closesocket(s) };
    }
    if !deliberate {
        sink.closed();
    }
}

/// The control channel to the buds.
pub struct WinLink {
    state: Arc<State>,
    channel: Option<u8>,
    hint: Candidate,
}

impl Link for WinLink {
    fn channel(&self) -> Option<u8> {
        self.channel
    }

    fn connect_hint(&self) -> Option<String> {
        Some(self.hint.name())
    }

    fn write(&self, bytes: &[u8]) -> io::Result<()> {
        let guard = self.state.sock.lock().unwrap();
        let Some(sock) = *guard else {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "link closed"));
        };
        let mut sent = 0;
        while sent < bytes.len() {
            // SAFETY: the slice is valid for reads; the socket stays open while the lock is held.
            let n = unsafe { send(sock, &bytes[sent..], SEND_RECV_FLAGS(0)) };
            if n == SOCKET_ERROR {
                let code = last_error();
                return Err(io::Error::other(describe(code)));
            }
            sent += n as usize;
        }
        Ok(())
    }

    fn close(&self) {
        self.state.closed.store(true, Ordering::SeqCst);
        if let Some(sock) = *self.state.sock.lock().unwrap() {
            // SAFETY: wakes the reader's `recv`; the reader closes the handle.
            unsafe { shutdown(sock, SD_BOTH) };
        }
    }
}
