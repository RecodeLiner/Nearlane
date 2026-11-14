//! Offline diagnostics; this binary neither logs in nor sends requests to Google.

use std::{
    error::Error,
    io::{self, Read},
    process::ExitCode,
};

fn run() -> Result<(), Box<dyn Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("status") => println!("{}", nearlane_service::Service::default().status()),
        Some("test-transfer") => {
            let report = nearlane_transfer::run_loopback_self_test()?;
            println!(
                "Local encrypted Nearby transfer passed: {} bytes in {:.2}s",
                report.summary.bytes,
                report.elapsed.as_secs_f64()
            );
            let digest = report
                .summary
                .sha256
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            println!("Saved-file SHA-256 matched: {digest}");
            println!(
                "Owned loopback peers only. Android, certificates and private discovery are not tested."
            );
        }
        Some("inspect-auth-url") => {
            // Read from stdin so a captured authorization URL need not be put
            // in shell history or exposed as a command-line argument.
            let mut input = String::new();
            io::stdin().take(16 * 1024 + 1).read_to_string(&mut input)?;
            let summary = nearlane_auth::inspect_authorization_url(input.trim())?;
            println!("Authorization path: {}", summary.path);
            println!("Scopes: {}", summary.scopes.join(" "));
            println!(
                "Client ID present: {} (not retained)",
                summary.has_client_id
            );
            println!("PKCE S256: {}", summary.uses_pkce_s256);
            println!("CSRF state present: {}", summary.has_state);
            println!("Redirect kind: {}", summary.redirect_kind);
        }
        _ => println!(
            "Nearlane research CLI\n\n  nearlane status\n  nearlane inspect-auth-url  < URL-on-stdin\n  nearlane test-transfer\n\nPrivate discovery and Android transfers are not enabled yet."
        ),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
