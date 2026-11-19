//! Desktop application controller with a toolkit-independent command and snapshot API.
pub mod i18n;
mod notifications;
mod radio;
mod runtime;
mod snapshot;
pub use nearlane_platform::preferences::Language;
use nearlane_platform::preferences::PreferenceStore;
pub use snapshot::Snapshot;
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

/// Explicit startup policy. Isolated mode never restores a grant, launches a
/// browser, scans a radio or creates an OS notification.
pub struct Options {
    config: Option<PathBuf>,
    preferences: Option<PreferenceStore>,
    live: bool,
}
impl Options {
    /// Normal desktop launch with runtime paths and private background defaults.
    pub fn desktop() -> Self {
        Self {
            config: nearlane_platform::configuration::oauth_config_path(),
            preferences: PreferenceStore::discover(),
            live: true,
        }
    }
    /// Headless tests and design previews, using only explicit fixture paths.
    pub fn isolated(config: Option<PathBuf>, preferences: Option<PreferenceStore>) -> Self {
        Self {
            config,
            preferences,
            live: false,
        }
    }
}

/// Intent from a frontend; account secrets and raw protocol packets are absent.
pub enum Command {
    /// Begin an explicit browser login.
    SignIn,
    /// Cancel that attempt.
    CancelSignIn,
    /// Withdraw local device registration and forget the saved grant.
    SignOut,
    /// Retry an existing OS-vault grant.
    RestoreSignIn,
    /// Retry private certificate setup.
    PrepareOwnDevices,
    /// Explicit bounded Wi-Fi search.
    FindOwnDevices,
    /// Stop discovery and forget candidates.
    CancelDiscovery,
    /// Explicit local OAuth JSON selection.
    ImportConfiguration(PathBuf),
    /// Explicit local outgoing file selection.
    SelectFile(PathBuf),
    /// Send to the current displayed candidate, rejecting stale list revisions.
    SendFile {
        /// Zero-based displayed candidate.
        index: usize,
        /// Snapshot generation at the time of selection.
        devices_revision: u64,
    },
    /// Interrupt outgoing IO.
    CancelTransfer,
    /// Explicit local destination folder selection.
    SelectDestination(PathBuf),
    /// Enable private receiving.
    StartReceiving,
    /// Withdraw the listener.
    StopReceiving,
    /// Apply consent only to the identified pending offer.
    DecideIncoming {
        /// Current verified offer identity.
        request_id: u64,
        /// Explicit local consent.
        accept: bool,
    },
    /// Persist continuous private discovery intent.
    SetDiscovery(bool),
    /// Persist private receiving intent.
    ConfigureReceiving {
        /// Resume after authentication.
        enabled: bool,
        /// Require consent for each verified own-device offer.
        confirm: bool,
    },
    /// Window visibility and selected screen, solely for foreground BLE policy.
    SetSendSurface(bool),
    /// Persist a validated language choice and re-render existing state.
    SetLanguage(Language),
    /// Report contact support without widening device visibility.
    CheckContacts,
}
enum Message {
    Command(Command),
    Barrier(mpsc::SyncSender<()>),
}

/// Cloneable, bounded command endpoint. It does not own the application's lifetime.
#[derive(Clone)]
pub struct Sender(mpsc::SyncSender<Message>);
impl Sender {
    /// Enqueue without blocking the UI. A full/stopped application returns false.
    pub fn send(&self, command: Command) -> bool {
        self.0.try_send(Message::Command(command)).is_ok()
    }
}
/// Owner of one toolkit-independent background runtime. Closing a window does
/// not drop this owner; explicit Quit stops workers but preserves the grant.
pub struct Application {
    sender: Sender,
    state: Arc<RwLock<Snapshot>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Application {
    /// Start the controller on its own thread, independent of a GUI event loop.
    pub fn start(options: Options) -> io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel(64);
        let state = Arc::new(RwLock::new(Snapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let output = Arc::clone(&state);
        let cancel = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("nearlane-application".into())
            .spawn(move || {
                let mut runtime = runtime::Runtime::new(options);
                while !cancel.load(Ordering::Acquire) {
                    let mut barriers = Vec::new();
                    for message in rx.try_iter().take(64) {
                        match message {
                            Message::Command(c) => runtime.command(c),
                            Message::Barrier(tx) => barriers.push(tx),
                        }
                    }
                    runtime.tick();
                    if let Ok(mut current) = output.write() {
                        let mut next = snapshot::snapshot(&runtime);
                        next.revision = current.revision;
                        if *current != next {
                            next.revision = next.revision.wrapping_add(1);
                            *current = next;
                        }
                    }
                    for barrier in barriers {
                        let _ = barrier.send(());
                    }
                    thread::park_timeout(Duration::from_millis(150));
                }
                runtime.shutdown();
            })?;
        let app = Self {
            sender: Sender(tx),
            state,
            stop,
            worker: Some(worker),
        };
        app.flush(Duration::from_secs(5))?;
        Ok(app)
    }
    /// A non-owning endpoint for callbacks.
    pub fn sender(&self) -> Sender {
        self.sender.clone()
    }
    /// Latest safe state; no toolkit lock or OS IO is performed here.
    pub fn snapshot(&self) -> Snapshot {
        self.state.read().map(|s| s.clone()).unwrap_or_default()
    }
    /// Synchronize startup/tests with commands already accepted by the queue.
    /// Production event handlers should enqueue commands and observe snapshots.
    pub fn flush(&self, timeout: Duration) -> io::Result<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .0
            .try_send(Message::Barrier(tx))
            .map_err(|_| io::Error::other("Application queue unavailable"))?;
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
        rx.recv_timeout(timeout)
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Application did not respond"))
    }
    /// Stop once, without signing out or deleting account credentials.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
impl Drop for Application {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests;
