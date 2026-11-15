//! Actionable file notifications. No account grants, paths or file contents.
#[cfg(target_os = "linux")]
mod linux;
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

/// A decision from a notification; it must still match the current offer in the
/// service. An OS notification never directly opens or saves a file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAction {
    /// Explicit local acceptance.
    Accept,
    /// Explicit local refusal.
    Decline,
}
/// Safe notification outcomes; no raw D-Bus messages are propagated to the UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationError {
    /// Server/bus unavailable, timed out, or has no action support.
    Unavailable,
    /// Notification lifetime ended without a decision.
    Expired,
    /// The pending offer was cancelled/replaced.
    Cancelled,
    /// Bounded plain-text presentation fields were invalid.
    Invalid,
}
impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "File notification: {self:?}")
    }
}
impl std::error::Error for NotificationError {}

/// Verified offer details for display. Debug omits personal/device/file names.
pub struct FileNotification {
    sender: String,
    name: String,
    bytes: u64,
}
impl fmt::Debug for FileNotification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileNotification([REDACTED])")
    }
}
impl FileNotification {
    /// Construct from already verified offer metadata; never pass plaintext
    /// advertisements here. Bounds also apply when called outside the desktop.
    pub fn new(sender: &str, name: &str, bytes: u64) -> Result<Self, NotificationError> {
        if sender.is_empty()
            || sender.len() > 128
            || name.is_empty()
            || name.len() > 255
            || sender.chars().chain(name.chars()).any(char::is_control)
            || bytes > 64 * 1024 * 1024
        {
            return Err(NotificationError::Invalid);
        }
        Ok(Self {
            sender: sender.into(),
            name: name.into(),
            bytes,
        })
    }
    #[cfg(target_os = "linux")]
    fn body(&self, markup: bool) -> String {
        let text = |s: &str| if markup { escape(s) } else { s.to_owned() };
        format!(
            "{}\n{} · {} bytes",
            text(&self.sender),
            text(&self.name),
            self.bytes
        )
    }
}
#[cfg(target_os = "linux")]
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// One offer's notification, with cancellable lifetime and bounded result queue.
/// Drop withdraws it asynchronously; stale actions still need a service ID check.
pub struct ActionNotification {
    cancel: Arc<AtomicBool>,
    presented: Arc<AtomicBool>,
    result: mpsc::Receiver<Result<FileAction, NotificationError>>,
}
impl fmt::Debug for ActionNotification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ActionNotification([REDACTED])")
    }
}
impl Drop for ActionNotification {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl ActionNotification {
    /// Start a native action-capable notification without blocking the UI.
    /// Unsupported platforms return Unavailable for the existing tray fallback.
    pub fn show(request: FileNotification) -> Result<Self, NotificationError> {
        #[cfg(target_os = "linux")]
        {
            let cancel = Arc::new(AtomicBool::new(false));
            let presented = Arc::new(AtomicBool::new(false));
            let (tx, result) = mpsc::sync_channel(1);
            let stop = Arc::clone(&cancel);
            let shown = Arc::clone(&presented);
            std::thread::Builder::new()
                .name("nearlane-file-notification".into())
                .spawn(move || {
                    let outcome = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|_| NotificationError::Unavailable)
                        .and_then(|rt| rt.block_on(linux::run(request, stop, shown)));
                    let _ = tx.send(outcome);
                })
                .map_err(|_| NotificationError::Unavailable)?;
            Ok(Self {
                cancel,
                presented,
                result,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            drop((request.sender, request.name, request.bytes));
            Err(NotificationError::Unavailable)
        }
    }
    /// Whether the server accepted this offer's notification for presentation.
    pub fn presented(&self) -> bool {
        self.presented.load(Ordering::Acquire)
    }
    /// Terminal decision or presentation failure. This does not authorize IO.
    pub fn poll(&self) -> Option<Result<FileAction, NotificationError>> {
        match self.result.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err(NotificationError::Unavailable)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_is_bounded_and_debug_is_redacted() {
        let r = FileNotification::new("Private phone", "private-file.txt", 94)
            .unwrap_or_else(|_| panic!("valid fixture"));
        assert!(!format!("{r:?}").contains("Private"));
        assert!(FileNotification::new("phone\ncontrol", "test", 1).is_err());
        assert!(FileNotification::new("phone", "test", 65 * 1024 * 1024).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn markup_cannot_turn_names_into_notification_links() {
        let r = FileNotification::new("<b>phone</b>", "a&b.txt", 94)
            .unwrap_or_else(|_| panic!("valid fixture"));
        assert_eq!(
            r.body(true),
            "&lt;b&gt;phone&lt;/b&gt;\na&amp;b.txt · 94 bytes"
        );
        assert_eq!(r.body(false), "<b>phone</b>\na&b.txt · 94 bytes");
    }
}
