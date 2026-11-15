//! UI-thread coordinator; the platform worker never decides file access itself.
use nearlane_platform::notifications::{
    ActionNotification, FileAction, FileNotification, NotificationError,
};
use nearlane_service::Service;

#[derive(Default)]
pub(super) struct OfferNotifications {
    current: Option<u64>,
    job: Option<ActionNotification>,
    suppress_fallback: bool,
}
impl OfferNotifications {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    pub(super) fn suppress_fallback(&self) -> bool {
        self.suppress_fallback
    }
    pub(super) fn update(&mut self, service: &mut Service, enabled: bool) {
        let id = service.incoming_request_id();
        if self.current != id {
            self.clear();
            self.current = id;
            if enabled {
                if let Some(offer) = service.incoming_offer() {
                    self.job = FileNotification::new(&offer.sender, &offer.name, offer.bytes)
                        .and_then(ActionNotification::show)
                        .ok();
                    self.suppress_fallback = self.job.is_some();
                }
            }
        }
        if let Some(result) = self.job.as_ref().and_then(ActionNotification::poll) {
            match result {
                Ok(action) => {
                    if let Some(id) = self.current {
                        let _ = service.decide_incoming_for(id, action == FileAction::Accept);
                    }
                }
                Err(NotificationError::Unavailable | NotificationError::Invalid) => {
                    self.suppress_fallback = false
                }
                Err(NotificationError::Expired | NotificationError::Cancelled) => {}
            }
            self.job = None;
        }
    }
}
