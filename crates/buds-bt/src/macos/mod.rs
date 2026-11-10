//! macOS backend: IOBluetooth (the same framework the original Swift app uses), through `objc2`.
//!
//! IOBluetooth wants to be used from the main thread and delivers its callbacks on the main run loop.
//! Everything that touches an IOBluetooth object therefore runs on the main dispatch queue (`on_main`);
//! the worker threads that walk the connection candidates only wait for results. In the tray app the main
//! thread is the Tauri event loop; `budsctl` runs a small run loop there (`run_main_loop_until`).
//!
//! Connecting follows the original: the control channel is the RFCOMM channel advertised by the service
//! record named "enco" or by the HeyMelody UUIDs (079A, 1107), the serial port only when neither exists
//! (realme), and the fixed channels 15, 12, 13 when no record is known. The candidate order is the
//! shared one in `crate::plan`; this file turns a candidate into a channel id and opens it.

mod delegate;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use buds_core::platform::{BtDevice, Link, LinkSink, LogDirection, LogFn, OpenError, Platform};
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, msg_send};
use objc2_foundation::{NSArray, NSString};
use objc2_io_bluetooth::{
    IOBluetoothDevice, IOBluetoothRFCOMMChannel, IOBluetoothSDPServiceRecord, IOBluetoothSDPUUID,
};

use self::delegate::{Delegate, OpenEvent};
use crate::plan::{self, Candidate, ConnectError};

/// A connection attempt may spend up to two service lookups, the serial port and three channel guesses.
const CONNECT_BUDGET: Duration = Duration::from_secs(20);
/// How long a page (`openConnection`) may take before we stop waiting for its answer.
const PAGE_WAIT: Duration = Duration::from_secs(10);
/// 8000 x 0.625 ms = 5 s, like the original.
const PAGE_TIMEOUT_UNITS: u32 = 8000;
/// `kIOReturnSuccess`.
const IO_OK: i32 = 0;
/// Winsock's "service not found" code, which the shared walk understands (see `ConnectError`).
const NOT_FOUND: i32 = 10108;

/// HeyMelody control service UUIDs; the buds advertise one or the other.
const HEYMELODY: [u8; 16] = [
    0x00, 0x00, 0x07, 0x9A, 0xD1, 0x02, 0x11, 0xE1, 0x9B, 0x23, 0x00, 0x02, 0x5B, 0x00, 0xA5, 0xA5,
];
const HEYMELODY_ALT: [u8; 16] = [
    0x00, 0x00, 0x11, 0x07, 0xD1, 0x02, 0x11, 0xE1, 0x9B, 0x23, 0x00, 0x02, 0x5B, 0x00, 0xA5, 0xA5,
];
/// Standard Serial Port Profile (0x1101) in the Bluetooth base UUID.
const SPP: [u8; 16] = [
    0x00, 0x00, 0x11, 0x01, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
];

/// A value that is only ever used on the main thread, wrapped so it can travel in closures. Dropping it
/// anywhere is fine (releasing an Objective-C object is thread safe).
struct MainOnly<T>(T);

// SAFETY: every use of the inner value happens inside `on_main` / `exec_async` closures that run on the
// main thread; other threads only move the wrapper around.
unsafe impl<T> Send for MainOnly<T> {}

impl<T> MainOnly<T> {
    /// Closures must reach the value through this method: a plain `wrapper.0` would make an edition-2024
    /// closure capture just the field, bypassing the `Send` wrapper.
    fn get(&self) -> &T {
        &self.0
    }
}

/// Runs `work` on the main thread and returns its result. Blocks the caller until it ran, so never call
/// it from the main thread's own run-loop callbacks while the main thread waits for the caller.
fn on_main<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    if MainThreadMarker::new().is_some() {
        return work();
    }
    let (tx, rx) = mpsc::channel();
    DispatchQueue::main().exec_async(move || {
        let _ = tx.send(work());
    });
    rx.recv().expect("the main queue is running")
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MacPlatform;

impl MacPlatform {
    pub fn new() -> MacPlatform {
        MacPlatform
    }
}

fn not_found(text: &str) -> ConnectError {
    ConnectError {
        code: NOT_FOUND,
        message: text.into(),
        timed_out: false,
        cancelled: false,
    }
}

fn failed(text: String) -> ConnectError {
    ConnectError {
        code: 0,
        message: text,
        timed_out: false,
        cancelled: false,
    }
}

fn io_error(code: i32) -> String {
    format!("0x{:08x}", code as u32)
}

fn uuid(bytes: &[u8; 16]) -> Option<Retained<IOBluetoothSDPUUID>> {
    // SAFETY: `bytes` points at 16 readable bytes for the duration of the call.
    unsafe { IOBluetoothSDPUUID::uuidWithBytes_length(bytes.as_ptr().cast(), 16) }
}

fn string_of(text: Option<Retained<NSString>>) -> Option<String> {
    text.map(|s| s.to_string())
}

fn address_bytes(device: &IOBluetoothDevice) -> Option<[u8; 6]> {
    // SAFETY: `getAddress` returns a pointer into the device object (or null).
    let pointer = unsafe { device.getAddress() };
    if pointer.is_null() {
        return None;
    }
    // SAFETY: non-null, points at a valid BluetoothDeviceAddress.
    Some(unsafe { (*pointer).data })
}

/// The service records IOBluetooth has cached for the device.
fn records(device: &IOBluetoothDevice) -> Vec<Retained<IOBluetoothSDPServiceRecord>> {
    // SAFETY: plain property read; the array holds IOBluetoothSDPServiceRecord objects.
    let Some(array) = (unsafe { device.services() }) else {
        return Vec::new();
    };
    let array: &NSArray = &array;
    (0..array.count())
        .map(|i| {
            // SAFETY: the array is documented to contain service records.
            unsafe {
                Retained::cast_unchecked::<IOBluetoothSDPServiceRecord>(array.objectAtIndex(i))
            }
        })
        .collect()
}

fn named_enco(record: &IOBluetoothSDPServiceRecord) -> bool {
    // SAFETY: plain property read.
    string_of(unsafe { record.getServiceName() }).is_some_and(|n| n.to_lowercase().contains("enco"))
}

fn channel_of(record: &IOBluetoothSDPServiceRecord) -> Option<u8> {
    let mut id: u8 = 0;
    // SAFETY: `id` is a valid out-pointer.
    (unsafe { record.getRFCOMMChannelID(&mut id) } == IO_OK).then_some(id)
}

/// The advertised control service: a record named "enco", or one of the HeyMelody UUIDs.
fn has_control_service(device: &IOBluetoothDevice) -> bool {
    records(device).iter().any(|r| named_enco(r))
        || [HEYMELODY, HEYMELODY_ALT]
            .iter()
            .filter_map(uuid)
            // SAFETY: the UUID object is valid.
            .any(|u| unsafe { device.getServiceRecordForUUID(Some(&u)) }.is_some())
}

fn paired() -> Vec<Retained<IOBluetoothDevice>> {
    // SAFETY: class method returning an array of IOBluetoothDevice objects.
    let Some(array) = (unsafe { IOBluetoothDevice::pairedDevices() }) else {
        return Vec::new();
    };
    let array: &NSArray = &array;
    (0..array.count())
        .map(|i| {
            // SAFETY: the array is documented to contain IOBluetoothDevice objects.
            unsafe { Retained::cast_unchecked::<IOBluetoothDevice>(array.objectAtIndex(i)) }
        })
        .collect()
}

fn find(address: [u8; 6]) -> Option<Retained<IOBluetoothDevice>> {
    paired()
        .into_iter()
        .find(|d| address_bytes(d) == Some(address))
}

fn list_paired() -> Vec<BtDevice> {
    paired()
        .iter()
        .filter_map(|device| {
            let address = address_bytes(device)?;
            // `name` can be nil for a device that was never seen; ask for an optional.
            let name: Option<Retained<NSString>> = unsafe { msg_send![&**device, name] };
            Some(BtDevice {
                address,
                name: string_of(name),
                // SAFETY: plain property read.
                connected: unsafe { device.isConnected() },
                has_control_service: has_control_service(device),
            })
        })
        .collect()
}

/// What a candidate means on this OS: the RFCOMM channel to open.
fn channel_for(device: &IOBluetoothDevice, candidate: Candidate) -> Result<u8, ConnectError> {
    let by_uuid = |bytes: &[u8; 16]| {
        let u = uuid(bytes)?;
        // SAFETY: the UUID object is valid.
        let record = unsafe { device.getServiceRecordForUUID(Some(&u)) }?;
        channel_of(&record)
    };
    let found = match candidate {
        Candidate::Channel(channel) => Some(channel),
        Candidate::HeyMelody => records(device)
            .iter()
            .filter(|r| named_enco(r))
            .find_map(|r| channel_of(r))
            .or_else(|| by_uuid(&HEYMELODY)),
        Candidate::HeyMelodyAlt => by_uuid(&HEYMELODY_ALT),
        Candidate::Spp => by_uuid(&SPP),
    };
    match found {
        Some(channel) => Ok(channel),
        None => {
            if candidate == Candidate::HeyMelody {
                // SAFETY: refreshes the cached service records for the next attempt; no callback wanted.
                unsafe { device.performSDPQuery(None) };
            }
            Err(not_found("no service record for this candidate"))
        }
    }
}

/// An RFCOMM channel that has been asked to open (or is open).
struct Opened {
    channel: MainOnly<Retained<IOBluetoothRFCOMMChannel>>,
    delegate: MainOnly<Retained<Delegate>>,
    id: u8,
    events: mpsc::Receiver<OpenEvent>,
}

/// Starts opening the candidate's channel. Main thread only.
fn start_open(address: [u8; 6], candidate: Candidate) -> Result<Opened, ConnectError> {
    let device = find(address).ok_or_else(|| failed("the buds are no longer paired".into()))?;
    let id = channel_for(&device, candidate)?;
    let (tx, events) = mpsc::channel();
    let delegate = Delegate::new(tx.clone());
    let mut channel: Option<Retained<IOBluetoothRFCOMMChannel>> = None;
    let target: &AnyObject = &delegate;
    // SAFETY: `channel` is a valid out-parameter and the delegate implements the callbacks IOBluetooth
    // sends; it is kept alive by the returned `Opened`.
    let result = unsafe {
        device.openRFCOMMChannelAsync_withChannelID_delegate(Some(&mut channel), id, Some(target))
    };
    if result != IO_OK {
        return Err(failed(format!(
            "open channel {id} failed: {}",
            io_error(result)
        )));
    }
    let Some(channel) = channel else {
        return Err(failed(format!(
            "channel {id}: IOBluetooth returned no channel"
        )));
    };
    // SAFETY: plain property read. Already open (opened for us earlier): no callback will come.
    if unsafe { channel.isOpen() } {
        let _ = tx.send(OpenEvent::Complete(IO_OK));
    }
    Ok(Opened {
        channel: MainOnly(channel),
        delegate: MainOnly(delegate),
        id,
        events,
    })
}

/// Closes a channel we no longer want. Main thread only.
fn discard(channel: &IOBluetoothRFCOMMChannel, delegate: &Delegate) {
    delegate.detach();
    // SAFETY: valid channel object; a nil delegate is allowed.
    unsafe {
        channel.setDelegate(None);
        channel.closeChannel();
    }
}

/// Waits for the channel to open, bounded by the candidate's timeout and the cancel flag.
fn connect_candidate(
    address: [u8; 6],
    candidate: Candidate,
    cancelled: &AtomicBool,
) -> Result<Opened, ConnectError> {
    let opened = on_main(move || start_open(address, candidate).map(MainOnly))?.0;
    let deadline = Instant::now() + candidate.timeout();
    let outcome = loop {
        match opened.events.recv_timeout(Duration::from_millis(50)) {
            Ok(OpenEvent::Complete(status)) => break Some(status),
            Ok(OpenEvent::Closed) => break Some(-1),
            Err(mpsc::RecvTimeoutError::Disconnected) => break Some(-1),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    break None;
                }
            }
        }
    };
    match outcome {
        Some(IO_OK) if !cancelled.load(Ordering::Relaxed) => Ok(opened),
        Some(IO_OK) => {
            close_opened(opened);
            Err(ConnectError::cancelled())
        }
        Some(status) => {
            let id = opened.id;
            close_opened(opened);
            Err(failed(format!(
                "channel {id} did not open: {}",
                io_error(status)
            )))
        }
        None => {
            // Some channels never answer; one that opened without telling us is still good.
            let channel = MainOnly(opened.channel.0.clone());
            // SAFETY: plain property read, on the main thread.
            let open_anyway = on_main(move || unsafe { channel.get().isOpen() });
            if open_anyway && !cancelled.load(Ordering::Relaxed) {
                return Ok(opened);
            }
            close_opened(opened);
            Err(if cancelled.load(Ordering::Relaxed) {
                ConnectError::cancelled()
            } else {
                ConnectError::timeout()
            })
        }
    }
}

fn close_opened(opened: Opened) {
    let channel = opened.channel;
    let delegate = opened.delegate;
    DispatchQueue::main().exec_async(move || discard(channel.get(), delegate.get()));
}

impl Platform for MacPlatform {
    async fn paired_devices(&self) -> Vec<BtDevice> {
        tokio::task::spawn_blocking(|| on_main(list_paired))
            .await
            .unwrap_or_default()
    }

    async fn open(
        &self,
        device: &BtDevice,
        hint: Option<String>,
        sink: LinkSink,
        log: LogFn,
    ) -> Result<Box<dyn Link>, OpenError> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let _guard = CancelOnDrop(cancelled.clone());
        let address = device.address;
        let work = tokio::task::spawn_blocking(move || {
            let note = |text: &str| log(LogDirection::Info, text);
            plan::walk(
                hint.as_deref(),
                |candidate| connect_candidate(address, candidate, &cancelled),
                note,
            )
            .map(|(candidate, opened)| {
                note(&format!(
                    "connected via {candidate} (channel {})",
                    opened.id
                ));
                into_link(opened, candidate, sink)
            })
        });
        match work.await {
            Ok(Ok(link)) => Ok(Box::new(link)),
            Ok(Err(error)) => Err(OpenError(error.to_string())),
            Err(join) => Err(OpenError(format!("connect thread failed: {join}"))),
        }
    }

    /// `openConnection` with a page timeout: asks buds that only joined the phone to connect to this Mac.
    async fn page(&self, device: &BtDevice, log: LogFn) -> Result<(), String> {
        let address = device.address;
        let work = tokio::task::spawn_blocking(move || {
            log(LogDirection::Info, "paging the buds");
            let (tx, rx) = mpsc::channel();
            let started = on_main(move || {
                let Some(device) = find(address) else {
                    return Err("the buds are no longer paired".to_string());
                };
                let delegate = Delegate::new_for_page(tx);
                let target: &AnyObject = &delegate;
                // SAFETY: the delegate implements `connectionComplete:status:` and is kept alive by
                // the returned wrapper until the answer arrives.
                let result = unsafe {
                    device.openConnection_withPageTimeout_authenticationRequired(
                        Some(target),
                        PAGE_TIMEOUT_UNITS as _,
                        false,
                    )
                };
                if result == IO_OK {
                    Ok(MainOnly(delegate))
                } else {
                    Err(format!("page failed: {}", io_error(result)))
                }
            })?;
            let answer = rx.recv_timeout(PAGE_WAIT);
            drop(started);
            match answer {
                Ok(IO_OK) => Ok(()),
                Ok(status) => Err(format!("the buds are not reachable ({})", io_error(status))),
                Err(_) => Err("the buds did not answer in time".to_string()),
            }
        });
        work.await
            .unwrap_or_else(|e| Err(format!("page thread failed: {e}")))
    }

    fn connect_budget(&self) -> Duration {
        CONNECT_BUDGET
    }

    fn host_label(&self) -> &'static str {
        "Mac"
    }
}

/// Sets the flag when dropped: cancels the connect when the controller drops the open.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// The control channel. All IOBluetooth calls are queued on the main thread.
struct MacLink {
    channel: MainOnly<Retained<IOBluetoothRFCOMMChannel>>,
    delegate: MainOnly<Retained<Delegate>>,
    id: u8,
    hint: Candidate,
    sink: LinkSink,
    closed: Arc<AtomicBool>,
}

fn into_link(opened: Opened, hint: Candidate, sink: LinkSink) -> MacLink {
    let delegate = opened.delegate;
    delegate.0.attach(sink.clone());
    MacLink {
        channel: opened.channel,
        delegate,
        id: opened.id,
        hint,
        sink,
        closed: Arc::new(AtomicBool::new(false)),
    }
}

impl Link for MacLink {
    fn channel(&self) -> Option<u8> {
        Some(self.id)
    }

    fn connect_hint(&self) -> Option<String> {
        Some(self.hint.name())
    }

    /// Queues the write on the main thread (writes keep their order). A failed write closes the link.
    fn write(&self, bytes: &[u8]) -> std::io::Result<()> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the link is closed",
            ));
        }
        let channel = MainOnly(self.channel.0.clone());
        let sink = self.sink.clone();
        let mut data = bytes.to_vec();
        DispatchQueue::main().exec_async(move || {
            for chunk in data.chunks_mut(1000) {
                // SAFETY: `chunk` is valid for `len` bytes; writeSync only reads it.
                let result = unsafe {
                    channel
                        .get()
                        .writeSync_length(chunk.as_mut_ptr().cast(), chunk.len() as u16)
                };
                if result != IO_OK {
                    sink.closed();
                    return;
                }
            }
        });
        Ok(())
    }

    fn close(&self) {
        if self.closed.swap(true, Ordering::Relaxed) {
            return;
        }
        let channel = MainOnly(self.channel.0.clone());
        let delegate = MainOnly(self.delegate.0.clone());
        DispatchQueue::main().exec_async(move || discard(channel.get(), delegate.get()));
    }
}

impl Drop for MacLink {
    fn drop(&mut self) {
        self.close();
    }
}

/// Runs the main thread's run loop until `done` is set. `budsctl` calls this on its main thread while its
/// controller runs on worker threads: IOBluetooth delivers its callbacks on the main run loop.
pub fn run_main_loop_until(done: &AtomicBool) {
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSPort, NSRunLoop};
    // SAFETY: plain run-loop calls on the main thread. A port keeps the loop from returning at once.
    unsafe {
        let run_loop = NSRunLoop::currentRunLoop();
        let port = NSPort::port();
        run_loop.addPort_forMode(&port, NSDefaultRunLoopMode);
        while !done.load(Ordering::Relaxed) {
            let until = NSDate::dateWithTimeIntervalSinceNow(0.1);
            run_loop.runMode_beforeDate(NSDefaultRunLoopMode, &until);
        }
    }
}

/// `budsctl sdp`: the service records IOBluetooth has for the device.
pub fn sdp_report(address: [u8; 6]) -> Vec<String> {
    on_main(move || {
        let Some(device) = find(address) else {
            return vec!["the buds are not paired with this Mac".to_string()];
        };
        let mut lines = Vec::new();
        for record in records(&device) {
            // SAFETY: plain property read.
            let name =
                string_of(unsafe { record.getServiceName() }).unwrap_or_else(|| "(unnamed)".into());
            let channel =
                channel_of(&record).map_or("no channel".to_string(), |c| format!("channel {c}"));
            lines.push(format!("{name}: {channel}"));
        }
        if lines.is_empty() {
            lines.push("no cached service records (connect the buds first)".into());
        }
        lines
    })
}
