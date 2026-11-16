//! BlueZ LEAdvertisingManager lease; no global Adapter1 property mutation.
use super::*;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use zbus::{
    Proxy,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;
const UUID: &str = "0000fe2c-0000-1000-8000-00805f9b34fb";
struct Beacon {
    stop: Arc<AtomicBool>,
}
#[zbus::interface(name = "org.bluez.LEAdvertisement1")]
impl Beacon {
    fn release(&self) {
        self.stop.store(true, Ordering::Release);
    }
    #[zbus(property, name = "Type")]
    fn kind(&self) -> &str {
        "broadcast"
    }
    #[zbus(property)]
    fn service_data(&self) -> HashMap<String, Value<'_>> {
        HashMap::from([(UUID.into(), Value::from(silent_initiation_data().to_vec()))])
    }
    #[zbus(property)]
    fn timeout(&self) -> u16 {
        600
    }
}
pub(super) async fn run(stop: Arc<AtomicBool>, state: Arc<AtomicU8>) -> Result<(), ()> {
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let path = OwnedObjectPath::try_from(format!(
        "/org/nearlane/BluetoothWake/p{}",
        std::process::id()
    ))
    .map_err(|_| ())?;
    let connection = tokio::time::timeout(Duration::from_secs(3), async {
        zbus::connection::Builder::system()?
            .serve_at(
                path.clone(),
                Beacon {
                    stop: Arc::clone(&stop),
                },
            )?
            .build()
            .await
    })
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    lease(&connection, path, stop, state, "org.bluez").await
}
async fn lease(
    connection: &zbus::Connection,
    path: OwnedObjectPath,
    stop: Arc<AtomicBool>,
    state: Arc<AtomicU8>,
    service: &str,
) -> Result<(), ()> {
    let manager = tokio::time::timeout(
        Duration::from_secs(2),
        Proxy::new(
            connection,
            service.to_owned(),
            "/",
            "org.freedesktop.DBus.ObjectManager",
        ),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    let objects: Objects = tokio::time::timeout(
        Duration::from_secs(3),
        manager.call("GetManagedObjects", &()),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    let adapter = objects
        .into_iter()
        .find_map(|(path, interfaces)| {
            let powered = interfaces
                .get("org.bluez.Adapter1")?
                .get("Powered")?
                .downcast_ref::<bool>()
                .ok()?;
            (powered && interfaces.contains_key("org.bluez.LEAdvertisingManager1")).then_some(path)
        })
        .ok_or(())?;
    let proxy = tokio::time::timeout(
        Duration::from_secs(2),
        Proxy::new(
            connection,
            service.to_owned(),
            adapter,
            "org.bluez.LEAdvertisingManager1",
        ),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    let options: HashMap<String, Value<'_>> = HashMap::new();
    let _: () = tokio::time::timeout(
        Duration::from_secs(5),
        proxy.call("RegisterAdvertisement", &(path.clone(), options)),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    state.store(1, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(600);
    while !stop.load(Ordering::Acquire) && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _: Result<Result<(), zbus::Error>, _> = tokio::time::timeout(
        Duration::from_secs(2),
        proxy.call("UnregisterAdvertisement", &(path,)),
    )
    .await;
    // A dropped system-bus connection also causes BlueZ to release its owner.
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct ObjectsMock;
    #[zbus::interface(name = "org.freedesktop.DBus.ObjectManager")]
    impl ObjectsMock {
        fn get_managed_objects(&self) -> Objects {
            HashMap::from([(
                OwnedObjectPath::try_from("/org/nearlane/MockAdapter").unwrap(),
                HashMap::from([
                    (
                        "org.bluez.Adapter1".into(),
                        HashMap::from([("Powered".into(), OwnedValue::from(true))]),
                    ),
                    ("org.bluez.LEAdvertisingManager1".into(), HashMap::new()),
                ]),
            )])
        }
    }
    struct AdvertisingMock {
        registered: Arc<AtomicUsize>,
        removed: Arc<AtomicUsize>,
    }
    #[zbus::interface(name = "org.bluez.LEAdvertisingManager1")]
    impl AdvertisingMock {
        async fn register_advertisement(
            &self,
            path: OwnedObjectPath,
            options: HashMap<String, OwnedValue>,
            #[zbus(header)] header: zbus::message::Header<'_>,
            #[zbus(connection)] connection: &zbus::Connection,
        ) -> zbus::fdo::Result<()> {
            assert!(options.is_empty());
            let proxy = Proxy::new(
                connection,
                header.sender().unwrap().to_string(),
                path,
                "org.freedesktop.DBus.Properties",
            )
            .await
            .unwrap();
            let mut properties: HashMap<String, OwnedValue> = proxy
                .call("GetAll", &("org.bluez.LEAdvertisement1",))
                .await
                .unwrap();
            let kind: String = properties.remove("Type").unwrap().try_into().unwrap();
            assert_eq!(kind, "broadcast");
            assert!(
                !properties.contains_key("LocalName") && !properties.contains_key("Discoverable")
            );
            let mut data: HashMap<String, OwnedValue> = properties
                .remove("ServiceData")
                .unwrap()
                .try_into()
                .unwrap();
            let bytes: Vec<u8> = data.remove(UUID).unwrap().try_into().unwrap();
            assert_eq!(bytes, silent_initiation_data());
            self.registered.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn unregister_advertisement(&self, _path: OwnedObjectPath) {
            self.removed.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    #[ignore = "Opt-in session-bus mock only; no real radio, Google account or device scan"]
    fn native_mock_lease_registers_and_withdraws_only_its_advertisement() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let name = format!("org.nearlane.BluetoothMock.p{}", std::process::id());
                let registered = Arc::new(AtomicUsize::new(0));
                let removed = Arc::new(AtomicUsize::new(0));
                let _mock = zbus::connection::Builder::session()
                    .unwrap()
                    .name(name.as_str())
                    .unwrap()
                    .serve_at("/", ObjectsMock)
                    .unwrap()
                    .serve_at(
                        "/org/nearlane/MockAdapter",
                        AdvertisingMock {
                            registered: Arc::clone(&registered),
                            removed: Arc::clone(&removed),
                        },
                    )
                    .unwrap()
                    .build()
                    .await
                    .unwrap();
                let stop = Arc::new(AtomicBool::new(false));
                let state = Arc::new(AtomicU8::new(0));
                let client = zbus::connection::Builder::session()
                    .unwrap()
                    .serve_at(
                        "/org/nearlane/MockAdv",
                        Beacon {
                            stop: Arc::clone(&stop),
                        },
                    )
                    .unwrap()
                    .build()
                    .await
                    .unwrap();
                let flag = Arc::clone(&stop);
                let status = Arc::clone(&state);
                let cancel = tokio::spawn(async move {
                    while status.load(Ordering::Acquire) != 1 {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                    flag.store(true, Ordering::Release);
                });
                let result = tokio::time::timeout(
                    Duration::from_secs(5),
                    lease(
                        &client,
                        OwnedObjectPath::try_from("/org/nearlane/MockAdv").unwrap(),
                        stop,
                        state,
                        &name,
                    ),
                )
                .await
                .unwrap();
                assert!(result.is_ok());
                cancel.await.unwrap();
                assert_eq!(registered.load(Ordering::SeqCst), 1);
                assert_eq!(removed.load(Ordering::SeqCst), 1);
            });
    }
}
