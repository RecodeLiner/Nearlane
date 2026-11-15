//! freedesktop Notifications: fixed service, bounded calls, per-offer actions.
use super::*;
use futures_util::StreamExt as _;
use rand::RngCore as _;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use zbus::{Connection, Proxy, zvariant::Value};

struct Actions {
    accept: String,
    decline: String,
}
impl Actions {
    fn new() -> Result<Self, NotificationError> {
        let mut random = [0; 16];
        rand::rngs::OsRng
            .try_fill_bytes(&mut random)
            .map_err(|_| NotificationError::Unavailable)?;
        let nonce: String = random.iter().map(|b| format!("{b:02x}")).collect();
        Ok(Self {
            accept: format!("accept-{nonce}"),
            decline: format!("decline-{nonce}"),
        })
    }
    fn decision(&self, expected_id: u32, id: u32, key: &str) -> Option<FileAction> {
        if expected_id == 0 || id != expected_id {
            return None;
        }
        if key == self.accept {
            Some(FileAction::Accept)
        } else if key == self.decline {
            Some(FileAction::Decline)
        } else {
            None
        }
    }
}
pub(super) async fn run(
    request: FileNotification,
    cancel: Arc<AtomicBool>,
    shown: Arc<AtomicBool>,
) -> Result<FileAction, NotificationError> {
    if cancel.load(Ordering::Acquire) {
        return Err(NotificationError::Cancelled);
    }
    let connection = tokio::time::timeout(Duration::from_secs(2), Connection::session())
        .await
        .map_err(|_| NotificationError::Unavailable)?
        .map_err(|_| NotificationError::Unavailable)?;
    let proxy = tokio::time::timeout(
        Duration::from_secs(2),
        Proxy::new(
            &connection,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        ),
    )
    .await
    .map_err(|_| NotificationError::Unavailable)?
    .map_err(|_| NotificationError::Unavailable)?;
    run_on_proxy(request, cancel, shown, &proxy).await
}

async fn run_on_proxy(
    request: FileNotification,
    cancel: Arc<AtomicBool>,
    shown: Arc<AtomicBool>,
    proxy: &Proxy<'_>,
) -> Result<FileAction, NotificationError> {
    let caps: Vec<String> =
        tokio::time::timeout(Duration::from_secs(2), proxy.call("GetCapabilities", &()))
            .await
            .map_err(|_| NotificationError::Unavailable)?
            .map_err(|_| NotificationError::Unavailable)?;
    if !caps.iter().any(|c| c == "actions") {
        return Err(NotificationError::Unavailable);
    }
    let mut signals = tokio::time::timeout(
        Duration::from_secs(2),
        proxy.receive_signal("ActionInvoked"),
    )
    .await
    .map_err(|_| NotificationError::Unavailable)?
    .map_err(|_| NotificationError::Unavailable)?;
    if cancel.load(Ordering::Acquire) {
        return Err(NotificationError::Cancelled);
    }
    let actions = Actions::new()?;
    let buttons = vec![
        actions.accept.as_str(),
        "Accept",
        actions.decline.as_str(),
        "Decline",
    ];
    let body = request.body(caps.iter().any(|c| c == "body-markup"));
    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    hints.insert("desktop-entry", Value::from("org.nearlane.Nearlane"));
    hints.insert("category", Value::from("transfer"));
    let id: u32 = tokio::time::timeout(
        Duration::from_secs(2),
        proxy.call(
            "Notify",
            &(
                "Nearlane",
                0_u32,
                "folder-download",
                "Incoming file",
                body.as_str(),
                buttons,
                hints,
                55_000_i32,
            ),
        ),
    )
    .await
    .map_err(|_| NotificationError::Unavailable)?
    .map_err(|_| NotificationError::Unavailable)?;
    if id == 0 {
        return Err(NotificationError::Unavailable);
    }
    shown.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(55);
    let outcome = loop {
        if cancel.load(Ordering::Acquire) {
            break Err(NotificationError::Cancelled);
        }
        if Instant::now() >= deadline {
            break Err(NotificationError::Expired);
        }
        match tokio::time::timeout(Duration::from_millis(100), signals.next()).await {
            Ok(Some(message)) => {
                if message.body().len() > 512 {
                    continue;
                }
                if let Ok((received_id, key)) = message.body().deserialize::<(u32, String)>() {
                    if let Some(action) = actions.decision(id, received_id, &key) {
                        break Ok(action);
                    }
                }
            }
            Ok(None) => break Err(NotificationError::Unavailable),
            Err(_) => {}
        }
    };
    let close = async {
        let _: zbus::Result<()> = proxy.call("CloseNotification", &id).await;
    };
    let _ = tokio::time::timeout(Duration::from_secs(1), close).await;
    outcome
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn only_current_notification_and_unguessable_action_key_can_decide() {
        let current = Actions::new().unwrap();
        let old = Actions::new().unwrap();
        assert_eq!(
            current.decision(7, 7, &current.accept),
            Some(FileAction::Accept)
        );
        assert_eq!(
            current.decision(7, 7, &current.decline),
            Some(FileAction::Decline)
        );
        assert_eq!(current.decision(7, 8, &current.accept), None);
        assert_eq!(current.decision(7, 7, &old.accept), None);
        assert_eq!(current.decision(7, 7, "accept"), None);
    }

    struct MockNotifications {
        action: Option<FileAction>,
        closed: Arc<std::sync::atomic::AtomicU32>,
    }
    #[zbus::interface(name = "org.freedesktop.Notifications")]
    impl MockNotifications {
        fn get_capabilities(&self) -> Vec<&str> {
            vec!["actions", "body-markup"]
        }
        #[allow(clippy::too_many_arguments)]
        async fn notify(
            &self,
            app: &str,
            replaces: u32,
            icon: &str,
            summary: &str,
            body: &str,
            actions: Vec<String>,
            hints: HashMap<String, zbus::zvariant::OwnedValue>,
            timeout: i32,
            #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>,
        ) -> u32 {
            assert_eq!(app, "Nearlane");
            assert_eq!(replaces, 0);
            assert_eq!(icon, "folder-download");
            assert_eq!(summary, "Incoming file");
            assert!(body.contains("&lt;own device&gt;"));
            assert_eq!(actions.len(), 4);
            assert!(hints.contains_key("desktop-entry"));
            assert_eq!(timeout, 55_000);
            // A foreign ID or stale/unscoped key must never win the decision.
            Self::action_invoked(&ctxt, 778, &actions[0]).await.unwrap();
            Self::action_invoked(&ctxt, 777, "accept").await.unwrap();
            if let Some(action) = self.action {
                let key = &actions[if action == FileAction::Accept { 0 } else { 2 }];
                Self::action_invoked(&ctxt, 777, key).await.unwrap();
            }
            777
        }
        fn close_notification(&self, id: u32) {
            assert_eq!(id, 777);
            self.closed.fetch_add(1, Ordering::SeqCst);
        }
        #[zbus(signal)]
        async fn action_invoked(
            ctxt: &zbus::SignalContext<'_>,
            id: u32,
            key: &str,
        ) -> zbus::Result<()>;
    }

    #[test]
    #[ignore = "Opt-in native session-bus test; owns a separate mock service, no visible notifications or account access"]
    fn native_mock_action_routing_and_cancellation() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            for (i, action) in [Some(FileAction::Accept), Some(FileAction::Decline), None]
                .into_iter()
                .enumerate()
            {
                let name = format!("org.nearlane.NotificationTest.p{}.n{i}", std::process::id());
                let closed = Arc::new(std::sync::atomic::AtomicU32::new(0));
                let _server = zbus::connection::Builder::session()
                    .unwrap()
                    .name(name.as_str())
                    .unwrap()
                    .serve_at(
                        "/org/freedesktop/Notifications",
                        MockNotifications {
                            action,
                            closed: Arc::clone(&closed),
                        },
                    )
                    .unwrap()
                    .build()
                    .await
                    .unwrap();
                let client = Connection::session().await.unwrap();
                let proxy = Proxy::new(
                    &client,
                    name.as_str(),
                    "/org/freedesktop/Notifications",
                    "org.freedesktop.Notifications",
                )
                .await
                .unwrap();
                let cancel = Arc::new(AtomicBool::new(false));
                let shown = Arc::new(AtomicBool::new(false));
                let cancel_task = if action.is_none() {
                    let flag = Arc::clone(&cancel);
                    let presented = Arc::clone(&shown);
                    Some(tokio::spawn(async move {
                        while !presented.load(Ordering::Acquire) {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                        flag.store(true, Ordering::Release);
                    }))
                } else {
                    None
                };
                let request = FileNotification::new("<own device>", "test.txt", 94).unwrap();
                let result = tokio::time::timeout(
                    Duration::from_secs(5),
                    run_on_proxy(request, cancel, shown, &proxy),
                )
                .await
                .unwrap();
                assert_eq!(result, action.ok_or(NotificationError::Cancelled));
                assert_eq!(closed.load(Ordering::SeqCst), 1);
                if let Some(task) = cancel_task {
                    task.await.unwrap();
                }
            }
        });
    }
}
