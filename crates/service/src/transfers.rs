use nearlane_transfer::SendJob;
use std::path::PathBuf;

pub(crate) struct Transfers {
    pub(crate) source: Option<PathBuf>,
    pub(crate) job: Option<SendJob>,
    pub(crate) message: String,
    pub(crate) completed: bool,
}
impl Default for Transfers {
    fn default() -> Self {
        Self {
            source: None,
            job: None,
            completed: false,
            message: "Choose a file and send it to a discovered own device. One file up to 64 MiB."
                .into(),
        }
    }
}
impl std::fmt::Debug for Transfers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transfers")
            .field("busy", &self.job.is_some())
            .finish_non_exhaustive()
    }
}
impl Transfers {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
    pub(crate) fn cancel(&mut self) {
        self.job = None;
        self.completed = false;
        self.message =
            "Transfer cancelled. Check the phone before retrying if bytes had already been sent."
                .into();
    }
    pub(crate) fn poll(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        self.message = job.progress().message();
        if let Some(result) = job.poll() {
            self.completed = result.is_ok();
            self.message = match result {
                Ok(summary) => format!(
                    "The verified receiving device acknowledged {} bytes.",
                    summary.bytes
                ),
                Err(error) => error.to_string(),
            };
            self.job = None;
        }
    }
}
