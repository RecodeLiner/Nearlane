use nearlane_transfer::{IncomingOffer, ReceiveJob};
use std::path::PathBuf;

pub(crate) struct Receiving {
    pub(crate) directory: Option<PathBuf>,
    pub(crate) job: Option<ReceiveJob>,
    pub(crate) message: String,
    pub(crate) saved: bool,
    pub(crate) count: u32,
    pub(crate) enabled: bool,
    pub(crate) restart: std::time::Instant,
}

impl Default for Receiving {
    fn default() -> Self {
        Self {
            directory: None,
            job: None,
            message: "Choose a destination folder to enable private receiving.".into(),
            saved: false,
            count: 0,
            enabled: false,
            restart: std::time::Instant::now(),
        }
    }
}
impl std::fmt::Debug for Receiving {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Receiving")
            .field("active", &self.job.is_some())
            .finish_non_exhaustive()
    }
}
impl Receiving {
    pub(crate) fn reset(&mut self) {
        // The chosen local folder is an app preference, independent of account
        // trust. In particular, restoring login must not erase the default.
        let directory = self.directory.take();
        let count = self.count;
        *self = Self {
            directory,
            count,
            ..Self::default()
        };
    }
    pub(crate) fn stop(&mut self) {
        self.enabled = false;
        self.job = None;
        self.message = "Receiving stopped. The computer is hidden.".into();
    }
    pub(crate) fn offer(&self) -> Option<IncomingOffer> {
        self.job.as_ref().and_then(ReceiveJob::offer).filter(|_| {
            self.job
                .as_ref()
                .is_some_and(|j| j.phase() == nearlane_transfer::ReceivePhase::AwaitingConsent)
        })
    }
    pub(crate) fn poll(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        self.message = job.phase().message();
        if let Some(result) = job.poll() {
            self.saved = result.is_ok();
            if self.saved {
                self.count = self.count.saturating_add(1);
            }
            if matches!(
                result,
                Err(nearlane_transfer::ReceiveFailure::File
                    | nearlane_transfer::ReceiveFailure::Identity
                    | nearlane_transfer::ReceiveFailure::Listen
                    | nearlane_transfer::ReceiveFailure::Advertisement
                    | nearlane_transfer::ReceiveFailure::Worker)
            ) {
                self.enabled = false;
            }
            self.restart = std::time::Instant::now() + std::time::Duration::from_secs(1);
            self.message = match result {
                Ok(file) => format!(
                    "File saved: {} bytes. Preparing to receive another file.",
                    file.summary.bytes
                ),
                Err(error) => error.to_string(),
            };
            self.job = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stop_disables_background_restarts_but_keeps_the_selected_folder() {
        let mut receiving = Receiving {
            directory: Some(PathBuf::from("synthetic-destination")),
            enabled: true,
            ..Receiving::default()
        };
        receiving.stop();
        assert!(!receiving.enabled);
        assert_eq!(
            receiving.directory,
            Some(PathBuf::from("synthetic-destination"))
        );
        assert!(receiving.job.is_none());
        receiving.enabled = true;
        receiving.reset();
        assert!(!receiving.enabled);
        assert_eq!(
            receiving.directory,
            Some(PathBuf::from("synthetic-destination"))
        );
    }
}
