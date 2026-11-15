//! Privacy-preserving inventory: no key bytes, raw certificate IDs or guesses.
/// Safe certificate row for desktop rendering. Position is a temporary list index.
pub struct CertificateRow {
    /// One-based position in the current query result; not a stable device ID.
    pub number: usize,
    /// Locally owned key or name authenticated by a matched advertisement.
    pub device: Option<String>,
    /// Public start time, Unix seconds.
    pub starts_at: i64,
    /// Public expiration time, Unix seconds.
    pub expires_at: i64,
    /// Whether the certificate is usable now, including clock tolerance.
    pub active: bool,
}
