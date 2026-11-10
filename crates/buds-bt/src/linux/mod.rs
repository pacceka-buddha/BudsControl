//! Linux backend: BlueZ through the `bluer` crate (D-Bus for devices and profiles, plain RFCOMM sockets
//! for the channel guesses).
//!
//! Connecting: the buds advertise their control service (HeyMelody, UUID 079A or 1107; realme buds the
//! serial port profile). BlueZ does the service lookup when we register a *client profile* for that
//! UUID and ask it to connect that profile; the connected RFCOMM socket is then handed to us as a
//! `ConnectRequest`. When the buds advertise nothing we know, fixed channels 15, 12 and 13 are tried
//! with a raw RFCOMM socket. The candidate order is the shared one in `crate::plan`.

use std::collections::HashSet;
use std::io;
use std::time::Duration;

use bluer::rfcomm::{Profile, ProfileHandle, ReqError, Role, SocketAddr, Stream};
use bluer::{Address, Device, Session, Uuid};
use buds_core::platform::{BtDevice, Link, LinkSink, LogDirection, LogFn, OpenError, Platform};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Handle;
use tokio::sync::{OnceCell, mpsc, watch};

use crate::plan::{self, Candidate, ConnectError};

/// A connection attempt may spend up to two service connects, the serial port and some channel guesses.
const CONNECT_BUDGET: Duration = Duration::from_secs(25);
/// How long `Device.Connect` (which brings up the audio profiles) may take when paging.
const PAGE_TIMEOUT: Duration = Duration::from_secs(10);

/// HeyMelody control service UUIDs; the buds advertise one or the other.
const HEYMELODY: Uuid = Uuid::from_u128(0x0000079A_D102_11E1_9B23_00025B00A5A5);
const HEYMELODY_ALT: Uuid = Uuid::from_u128(0x00001107_D102_11E1_9B23_00025B00A5A5);
/// Standard Serial Port Profile.
const SPP: Uuid = Uuid::from_u128(0x00001101_0000_1000_8000_00805F9B34FB);

/// Winsock's "service not found" code, which the shared walk understands (see `ConnectError`).
const NOT_FOUND: i32 = 10108;
/// Winsock's "address in use": the service exists but its one connection is taken.
const BUSY: i32 = 10048;

#[derive(Default)]
pub struct LinuxPlatform {
    session: OnceCell<Session>,
}

impl LinuxPlatform {
    pub fn new() -> LinuxPlatform {
        LinuxPlatform::default()
    }

    /// The connection to `bluetoothd`; made on first use and retried while it keeps failing (the
    /// daemon may start later than we do).
    async fn session(&self) -> Result<Session, String> {
        self.session
            .get_or_try_init(|| async { Session::new().await.map_err(|e| e.to_string()) })
            .await
            .cloned()
    }

    async fn device(&self, address: [u8; 6]) -> Result<Device, String> {
        let session = self.session().await?;
        let adapter = session.default_adapter().await.map_err(|e| e.to_string())?;
        adapter.device(Address(address)).map_err(|e| e.to_string())
    }
}

fn is_control_uuid(uuid: &Uuid) -> bool {
    *uuid == HEYMELODY || *uuid == HEYMELODY_ALT
}

impl Platform for LinuxPlatform {
    async fn paired_devices(&self) -> Vec<BtDevice> {
        let Ok(session) = self.session().await else {
            return Vec::new();
        };
        let Ok(adapter) = session.default_adapter().await else {
            return Vec::new();
        };
        let Ok(addresses) = adapter.device_addresses().await else {
            return Vec::new();
        };
        let mut devices = Vec::new();
        for address in addresses {
            let Ok(device) = adapter.device(address) else {
                continue;
            };
            // BlueZ also lists devices it merely saw; only paired ones are ours.
            if !device.is_paired().await.unwrap_or(false) {
                continue;
            }
            let name = match device.name().await {
                Ok(Some(name)) => Some(name),
                _ => device.alias().await.ok(),
            };
            let uuids = device.uuids().await.ok().flatten().unwrap_or_default();
            devices.push(BtDevice {
                address: address.0,
                name,
                connected: device.is_connected().await.unwrap_or(false),
                has_control_service: uuids.iter().any(is_control_uuid),
            });
        }
        devices
    }

    async fn open(
        &self,
        device: &BtDevice,
        hint: Option<String>,
        sink: LinkSink,
        log: LogFn,
    ) -> Result<Box<dyn Link>, OpenError> {
        let session = self
            .session()
            .await
            .map_err(|e| OpenError(format!("Bluetooth service: {e}")))?;
        let bluez_device = self.device(device.address).await.map_err(OpenError)?;
        let advertised: HashSet<Uuid> = bluez_device
            .uuids()
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let address = Address(device.address);

        // The cancel flag trips when the controller drops this future.
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let _guard = CancelOnDrop(cancel_tx);
        let runtime = Handle::current();
        let walk_runtime = runtime.clone();

        let work = tokio::task::spawn_blocking(move || {
            let note = |text: &str| log(LogDirection::Info, text);
            let walked = plan::walk(
                hint.as_deref(),
                |candidate| {
                    walk_runtime.block_on(connect_candidate(
                        &session,
                        &bluez_device,
                        address,
                        &advertised,
                        candidate,
                        cancel_rx.clone(),
                    ))
                },
                note,
            );
            walked.map(|(candidate, opened)| {
                let channel = opened
                    .stream
                    .peer_addr()
                    .ok()
                    .map(|a| a.channel)
                    .filter(|c| *c != 0);
                note(&format!(
                    "connected via {candidate}{}",
                    channel.map_or(String::new(), |c| format!(" (channel {c})"))
                ));
                spawn_link(&runtime, opened, candidate, channel, sink)
            })
        });
        match work.await {
            Ok(Ok(link)) => Ok(Box::new(link)),
            Ok(Err(error)) => Err(OpenError(error.to_string())),
            Err(join) => Err(OpenError(format!("connect thread failed: {join}"))),
        }
    }

    /// `Device.Connect` makes BlueZ connect the profiles the device offers, audio included; for buds that
    /// are playing to the phone this is the request to join this computer too.
    async fn page(&self, device: &BtDevice, log: LogFn) -> Result<(), String> {
        let bluez_device = self.device(device.address).await?;
        log(LogDirection::Info, "paging the buds");
        match tokio::time::timeout(PAGE_TIMEOUT, bluez_device.connect()).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("the buds did not answer in time".into()),
        }
    }

    fn connect_budget(&self) -> Duration {
        CONNECT_BUDGET
    }

    fn host_label(&self) -> &'static str {
        "computer"
    }
}

/// A connected RFCOMM socket. For a service connect it carries the BlueZ profile registration too:
/// dropping that unregisters the profile, which may make BlueZ tear the connection down, so it has to
/// live as long as the link.
struct Opened {
    stream: Stream,
    profile: Option<ProfileHandle>,
}

/// Sets the flag when dropped: cancels a running connect when the controller drops the open.
struct CancelOnDrop(watch::Sender<bool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}

/// One attempt through `candidate`, bounded by the candidate's timeout and the cancel flag.
async fn connect_candidate(
    session: &Session,
    device: &Device,
    address: Address,
    advertised: &HashSet<Uuid>,
    candidate: Candidate,
    mut cancel: watch::Receiver<bool>,
) -> Result<Opened, ConnectError> {
    let attempt = async {
        match candidate {
            Candidate::HeyMelody => {
                through_profile(session, device, address, advertised, HEYMELODY).await
            }
            Candidate::HeyMelodyAlt => {
                through_profile(session, device, address, advertised, HEYMELODY_ALT).await
            }
            Candidate::Spp => through_profile(session, device, address, advertised, SPP).await,
            Candidate::Channel(channel) => Stream::connect(SocketAddr::new(address, channel))
                .await
                .map(|stream| Opened {
                    stream,
                    profile: None,
                })
                .map_err(|e| error_from_io(&e)),
        }
    };
    tokio::select! {
        result = attempt => result,
        () = tokio::time::sleep(candidate.timeout()) => Err(ConnectError::timeout()),
        _ = cancel.wait_for(|cancelled| *cancelled) => Err(ConnectError::cancelled()),
    }
}

/// Registers a client profile for `uuid`, asks BlueZ to connect it and takes the RFCOMM socket BlueZ
/// hands over.
async fn through_profile(
    session: &Session,
    device: &Device,
    address: Address,
    advertised: &HashSet<Uuid>,
    uuid: Uuid,
) -> Result<Opened, ConnectError> {
    if !advertised.contains(&uuid) {
        return Err(ConnectError {
            code: NOT_FOUND,
            message: "the buds do not advertise this service".into(),
            timed_out: false,
            cancelled: false,
        });
    }
    let profile = Profile {
        uuid,
        name: Some("Buds Control".into()),
        role: Some(Role::Client),
        // We connect when we want to; BlueZ must not connect the profile on its own.
        auto_connect: Some(false),
        ..Default::default()
    };
    let mut handle = session
        .register_profile(profile)
        .await
        .map_err(|e| ConnectError {
            code: 0,
            message: format!("cannot register the profile: {e}"),
            timed_out: false,
            cancelled: false,
        })?;

    let connect = device.connect_profile(&uuid);
    tokio::pin!(connect);
    let mut connect_finished = false;
    loop {
        tokio::select! {
            result = &mut connect, if !connect_finished => {
                connect_finished = true;
                if let Err(error) = result {
                    return Err(match error.kind {
                        bluer::ErrorKind::AlreadyConnected => ConnectError {
                            code: BUSY,
                            message: "the control service is already connected".into(),
                            timed_out: false,
                            cancelled: false,
                        },
                        _ => ConnectError {
                            code: 0,
                            message: error.to_string(),
                            timed_out: false,
                            cancelled: false,
                        },
                    });
                }
            }
            request = handle.next() => {
                let Some(request) = request else {
                    return Err(ConnectError {
                        code: 0,
                        message: "the profile was unregistered".into(),
                        timed_out: false,
                        cancelled: false,
                    });
                };
                if request.device() != address {
                    request.reject(ReqError::Rejected);
                    continue;
                }
                let stream = request.accept().map_err(|e| ConnectError {
                    code: 0,
                    message: e.to_string(),
                    timed_out: false,
                    cancelled: false,
                })?;
                return Ok(Opened {
                    stream,
                    profile: Some(handle),
                });
            }
        }
    }
}

fn error_from_io(error: &io::Error) -> ConnectError {
    ConnectError {
        code: error.raw_os_error().unwrap_or(0),
        message: error.to_string(),
        timed_out: false,
        cancelled: false,
    }
}

/// The control channel. One task owns the socket: it reads, and writes what `write` queues. Dropping the
/// link (or `close`) ends the task, which closes the socket.
struct LinuxLink {
    outgoing: mpsc::UnboundedSender<Vec<u8>>,
    stop: watch::Sender<bool>,
    channel: Option<u8>,
    hint: Candidate,
}

fn spawn_link(
    runtime: &Handle,
    opened: Opened,
    hint: Candidate,
    channel: Option<u8>,
    sink: LinkSink,
) -> LinuxLink {
    let (outgoing, mut queue) = mpsc::unbounded_channel::<Vec<u8>>();
    let (stop, mut stopped) = watch::channel(false);
    let Opened {
        mut stream,
        profile,
    } = opened;
    runtime.spawn(async move {
        // Unregistered (dropped) only when this task ends, after the socket is shut down.
        let _profile = profile;
        let mut buffer = [0u8; 1024];
        loop {
            tokio::select! {
                read = stream.read(&mut buffer) => match read {
                    Ok(0) | Err(_) => {
                        sink.closed();
                        break;
                    }
                    Ok(n) => sink.data(buffer[..n].to_vec()),
                },
                message = queue.recv() => match message {
                    Some(bytes) => {
                        if stream.write_all(&bytes).await.is_err() {
                            sink.closed();
                            break;
                        }
                    }
                    None => break, // the link was dropped
                },
                _ = stopped.changed() => break,
            }
        }
        let _ = stream.shutdown().await;
    });
    LinuxLink {
        outgoing,
        stop,
        channel,
        hint,
    }
}

impl Link for LinuxLink {
    fn channel(&self) -> Option<u8> {
        self.channel
    }

    fn connect_hint(&self) -> Option<String> {
        Some(self.hint.name())
    }

    fn write(&self, bytes: &[u8]) -> io::Result<()> {
        self.outgoing
            .send(bytes.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the link is closed"))
    }

    fn close(&self) {
        self.stop.send_replace(true);
    }
}

impl Drop for LinuxLink {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

/// `budsctl sdp`: the service UUIDs BlueZ knows for the device (BlueZ does not expose channel numbers).
pub async fn sdp_report(address: [u8; 6]) -> Vec<String> {
    let platform = LinuxPlatform::new();
    let device = match platform.device(address).await {
        Ok(device) => device,
        Err(error) => return vec![format!("cannot reach BlueZ: {error}")],
    };
    let uuids = device.uuids().await.ok().flatten().unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    for (label, uuid) in [
        ("HeyMelody 079A", HEYMELODY),
        ("HeyMelody 1107", HEYMELODY_ALT),
        ("Serial port 1101", SPP),
    ] {
        lines.push(format!(
            "{label}: {}",
            if uuids.contains(&uuid) {
                "advertised"
            } else {
                "not advertised"
            }
        ));
    }
    let mut others: Vec<String> = uuids.iter().map(ToString::to_string).collect();
    others.sort();
    lines.push(format!("all services: {}", others.join(", ")));
    lines
}
