use std::{
    collections::HashSet,
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

use nearlane_certificates::{
    DeviceMetadata, LocalCertificate, RemoteCertificate, VALIDITY_SECONDS, now_seconds,
};
use nearlane_core::Visibility;
use rand::RngCore as _;

use super::{CertificateSummary, IdentityError, proto, rpc::IdentityTransport};

const MAX_PAGES: usize = 8;
const MAX_CERTIFICATES: usize = 256;
const MAX_CERTIFICATE_BYTES: usize = 4 * 1024 * 1024;

pub(super) struct IdentityContext {
    name: String,
    local: Vec<LocalCertificate>,
    remote: Vec<RemoteCertificate>,
    pub(super) publication_attempted: bool,
}
impl fmt::Debug for IdentityContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdentityContext([REDACTED])")
    }
}

impl IdentityContext {
    pub(super) fn new() -> Result<Self, IdentityError> {
        let start = now_seconds().map_err(IdentityError::Certificate)?;
        let mut local = Vec::with_capacity(3);
        for period in 0..3 {
            local.push(
                LocalCertificate::generate(
                    DeviceMetadata::named("Nearlane").map_err(IdentityError::Certificate)?,
                    Visibility::OwnDevices,
                    start
                        .checked_add(period * VALIDITY_SECONDS)
                        .ok_or(IdentityError::Encoding)?,
                )
                .map_err(IdentityError::Certificate)?,
            );
        }
        Ok(Self {
            name: format!("devices/{}", device_id()?),
            local,
            remote: Vec::new(),
            publication_attempted: false,
        })
    }

    fn publication(&self, empty: bool) -> Result<proto::PublishRequest, IdentityError> {
        let mut credentials = Vec::new();
        if !empty {
            for cert in &self.local {
                credentials.push(proto::SharedCredential {
                    id: cert.credential_tag(),
                    data_type: 1,
                    data: cert
                        .publication_bytes()
                        .map_err(IdentityError::Certificate)?
                        .to_vec(),
                    expiration: Some(proto::Timestamp {
                        seconds: cert.published_expiration(),
                        nanos: 0,
                    }),
                });
            }
        }
        Ok(proto::PublishRequest {
            device: Some(proto::Device {
                name: self.name.clone(),
                display_name: "Nearlane".into(),
                contact: 1, // CONTACT_SELF; no Google-contact refresh/public distribution.
                groups: vec![proto::VisibilityCredentials {
                    visibility: 1,
                    credentials,
                }],
            }),
        })
    }

    pub(super) async fn synchronize(
        &mut self,
        client: &mut impl IdentityTransport,
        cancelled: &AtomicBool,
    ) -> Result<CertificateSummary, IdentityError> {
        check_cancelled(cancelled)?;
        let request = self.publication(false)?;
        // A timed-out request may already have reached Google. Retain the same
        // owned name/keys for retry and attempt withdrawal on account sign-out.
        self.publication_attempted = true;
        client.publish(request).await?;
        check_cancelled(cancelled)?;
        let mut page_token = String::new();
        let mut seen_pages = HashSet::new();
        let mut seen_certificates = HashSet::new();
        let mut remote = Vec::new();
        let mut processed = 0_usize;
        let mut bytes = 0_usize;
        let mut invalid = 0_usize;
        for _ in 0..MAX_PAGES {
            check_cancelled(cancelled)?;
            let page = client
                .query(proto::QueryRequest {
                    name: self.name.clone(),
                    page_size: 64,
                    page_token,
                })
                .await?;
            check_cancelled(cancelled)?;
            for credential in page.credentials {
                processed += 1;
                bytes = bytes
                    .checked_add(credential.data.len())
                    .ok_or(IdentityError::Limit)?;
                if processed > MAX_CERTIFICATES || bytes > MAX_CERTIFICATE_BYTES {
                    return Err(IdentityError::Limit);
                }
                if credential.data_type != 1 {
                    continue;
                }
                match RemoteCertificate::decode(&credential.data) {
                    Ok(cert) if cert.for_own_device() && seen_certificates.insert(*cert.id()) => {
                        remote.push(cert)
                    }
                    Ok(_) => {}
                    Err(_) => invalid += 1,
                }
            }
            if page.next_page_token.is_empty() {
                let now = now_seconds().map_err(IdentityError::Certificate)?;
                remote.retain(|c| c.expires_at().saturating_add(1800) > now);
                let summary = CertificateSummary {
                    own_device_certificates: remote.len(),
                    active_certificates: remote.iter().filter(|c| c.valid_at(now)).count(),
                    rejected_certificates: invalid,
                };
                // Replace the cache only after a complete, bounded page sequence.
                self.remote = remote;
                return Ok(summary);
            }
            if page.next_page_token.len() > 4096 || !seen_pages.insert(page.next_page_token.clone())
            {
                return Err(IdentityError::Limit);
            }
            page_token = page.next_page_token;
        }
        Err(IdentityError::Limit)
    }

    pub(super) async fn withdraw(
        &mut self,
        client: &mut impl IdentityTransport,
    ) -> Result<(), IdentityError> {
        if self.publication_attempted {
            client.publish(self.publication(true)?).await?;
            self.publication_attempted = false;
        }
        self.remote.clear();
        Ok(())
    }
}

fn device_id() -> Result<String, IdentityError> {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut id = String::with_capacity(10);
    for _ in 0..128 {
        let mut random = [0; 16];
        rand::rngs::OsRng
            .try_fill_bytes(&mut random)
            .map_err(|_| IdentityError::Encoding)?;
        for byte in random {
            if byte < 248 {
                id.push(ALPHABET[(byte % 62) as usize] as char);
            }
            if id.len() == 10 {
                return Ok(id);
            }
        }
    }
    Err(IdentityError::Encoding)
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), IdentityError> {
    if cancelled.load(Ordering::Acquire) {
        Err(IdentityError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct FakeIdentity {
        publications: Vec<proto::PublishRequest>,
        queries: Vec<proto::QueryRequest>,
        pages: VecDeque<Result<proto::QueryResponse, IdentityError>>,
    }
    impl IdentityTransport for FakeIdentity {
        async fn publish(
            &mut self,
            request: proto::PublishRequest,
        ) -> Result<proto::PublishResponse, IdentityError> {
            self.publications.push(request);
            Ok(proto::PublishResponse::default())
        }
        async fn query(
            &mut self,
            request: proto::QueryRequest,
        ) -> Result<proto::QueryResponse, IdentityError> {
            self.queries.push(request);
            self.pages.pop_front().unwrap()
        }
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }
    fn credential(visibility: Visibility, start: i64) -> proto::SharedCredential {
        let cert = LocalCertificate::generate(
            DeviceMetadata::named("Synthetic owned device").unwrap(),
            visibility,
            start,
        )
        .unwrap();
        proto::SharedCredential {
            id: cert.credential_tag(),
            data_type: 1,
            data: cert.publication_bytes().unwrap().to_vec(),
            expiration: None,
        }
    }
    fn page(credentials: Vec<proto::SharedCredential>, next: &str) -> proto::QueryResponse {
        proto::QueryResponse {
            credentials,
            next_page_token: next.into(),
        }
    }

    #[test]
    fn publishes_only_self_and_queries_all_pages_with_deduplication() {
        let now = now_seconds().unwrap();
        let own = credential(Visibility::OwnDevices, now);
        let mut fake = FakeIdentity {
            pages: [
                Ok(page(
                    vec![own.clone(), credential(Visibility::Contacts, now)],
                    "second",
                )),
                Ok(page(
                    vec![
                        own,
                        credential(Visibility::OwnDevices, now + VALIDITY_SECONDS),
                    ],
                    "",
                )),
            ]
            .into(),
            ..FakeIdentity::default()
        };
        let mut context = IdentityContext::new().unwrap();
        let summary = runtime()
            .block_on(context.synchronize(&mut fake, &AtomicBool::new(false)))
            .unwrap();
        assert_eq!(summary.own_device_certificates, 2);
        assert_eq!(summary.active_certificates, 1);
        assert_eq!(summary.rejected_certificates, 0);
        let device = fake.publications[0].device.as_ref().unwrap();
        assert_eq!(device.contact, 1);
        assert_eq!(device.groups.len(), 1);
        assert_eq!(device.groups[0].visibility, 1);
        assert_eq!(device.groups[0].credentials.len(), 3);
        for record in &device.groups[0].credentials {
            assert!(
                RemoteCertificate::decode(&record.data)
                    .unwrap()
                    .for_own_device()
            );
        }
        assert!(fake.queries[0].page_token.is_empty());
        assert_eq!(fake.queries[1].page_token, "second");
        assert!(
            fake.queries
                .iter()
                .all(|q| q.name == device.name && q.page_size == 64)
        );
    }

    #[test]
    fn failed_or_looping_query_keeps_previous_cache_and_owned_registration() {
        let mut context = IdentityContext::new().unwrap();
        let now = now_seconds().unwrap();
        let mut fake = FakeIdentity {
            pages: [Ok(page(vec![credential(Visibility::OwnDevices, now)], ""))].into(),
            ..FakeIdentity::default()
        };
        let rt = runtime();
        rt.block_on(context.synchronize(&mut fake, &AtomicBool::new(false)))
            .unwrap();
        let previous_id = *context.remote[0].id();
        for pages in [
            vec![Err(IdentityError::RpcStatus(7))],
            vec![Ok(page(vec![], "repeat")), Ok(page(vec![], "repeat"))],
        ] {
            fake.pages = pages.into();
            assert!(
                rt.block_on(context.synchronize(&mut fake, &AtomicBool::new(false)))
                    .is_err()
            );
            assert_eq!(*context.remote[0].id(), previous_id);
        }
        let publications = &fake.publications;
        assert!(publications.windows(2).all(|p| p[0] == p[1]));
        assert!(context.publication_attempted);
    }

    #[test]
    fn rejects_record_flood_and_skips_malformed_certificates() {
        let mut context = IdentityContext::new().unwrap();
        let mut fake = FakeIdentity {
            pages: [Ok(page(
                vec![proto::SharedCredential {
                    data_type: 1,
                    id: 0,
                    data: Vec::new(),
                    expiration: None,
                }],
                "",
            ))]
            .into(),
            ..FakeIdentity::default()
        };
        let rt = runtime();
        let summary = rt
            .block_on(context.synchronize(&mut fake, &AtomicBool::new(false)))
            .unwrap();
        assert_eq!(summary.rejected_certificates, 1);
        assert!(context.remote.is_empty());
        fake.pages = [Ok(page(
            vec![proto::SharedCredential::default(); MAX_CERTIFICATES + 1],
            "",
        ))]
        .into();
        assert_eq!(
            rt.block_on(context.synchronize(&mut fake, &AtomicBool::new(false))),
            Err(IdentityError::Limit)
        );
    }

    #[test]
    fn cancelled_setup_does_not_publish_and_withdrawal_targets_only_owned_name() {
        let mut context = IdentityContext::new().unwrap();
        let mut fake = FakeIdentity::default();
        let rt = runtime();
        assert_eq!(
            rt.block_on(context.synchronize(&mut fake, &AtomicBool::new(true))),
            Err(IdentityError::Cancelled)
        );
        assert!(fake.publications.is_empty());
        assert!(!context.publication_attempted);
        fake.pages.push_back(Ok(page(vec![], "")));
        rt.block_on(context.synchronize(&mut fake, &AtomicBool::new(false)))
            .unwrap();
        rt.block_on(context.withdraw(&mut fake)).unwrap();
        let first = fake.publications[0].device.as_ref().unwrap();
        let last = fake.publications[1].device.as_ref().unwrap();
        assert_eq!(first.name, last.name);
        assert_eq!(last.contact, 1);
        assert_eq!(last.groups.len(), 1);
        assert_eq!(last.groups[0].visibility, 1);
        assert!(last.groups[0].credentials.is_empty());
        assert!(!context.publication_attempted);
    }
}
