//! Windows backend: classic Bluetooth RFCOMM through Winsock, devices through `BluetoothAPIs`.

mod devices;
mod rfcomm;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use buds_core::platform::{BtDevice, Link, LinkSink, LogDirection, LogFn, OpenError, Platform};

use crate::plan::{self, Candidate};

/// A connection attempt may spend up to three 5 s service lookups and some channel guesses.
const CONNECT_BUDGET: Duration = Duration::from_secs(20);

/// Sets the flag when dropped: cancels the blocking connect when the controller drops the open.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> WindowsPlatform {
        WindowsPlatform
    }
}

impl Platform for WindowsPlatform {
    async fn paired_devices(&self) -> Vec<BtDevice> {
        tokio::task::spawn_blocking(devices::list_paired)
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
                |candidate| rfcomm::connect_candidate(address, candidate, &cancelled),
                note,
            )
            .map(|(candidate, connected)| {
                note(&format!(
                    "connected via {candidate}{}",
                    connected
                        .channel
                        .map_or(String::new(), |c| format!(" (channel {c})"))
                ));
                connected.into_link(candidate, sink)
            })
        });
        match work.await {
            Ok(Ok(link)) => Ok(Box::new(link)),
            Ok(Err(error)) => Err(OpenError(error.to_string())),
            Err(join) => Err(OpenError(format!("connect thread failed: {join}"))),
        }
    }

    /// Phase 3 version: connect to the control service and drop the connection at once. Creating the
    /// connection is what asks buds that are connected to the phone to join this PC too.
    async fn page(&self, device: &BtDevice, log: LogFn) -> Result<(), String> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let _guard = CancelOnDrop(cancelled.clone());
        let address = device.address;
        let work = tokio::task::spawn_blocking(move || {
            log(LogDirection::Info, "paging the buds");
            let connected = rfcomm::connect_candidate(address, Candidate::HeyMelody, &cancelled)
                .map_err(|e| e.to_string())?;
            connected.close();
            Ok(())
        });
        work.await
            .unwrap_or_else(|e| Err(format!("page thread failed: {e}")))
    }

    fn connect_budget(&self) -> Duration {
        CONNECT_BUDGET
    }

    fn host_label(&self) -> &'static str {
        "PC"
    }
}
