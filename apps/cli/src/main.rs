//! Offline diagnostics; this binary neither logs in nor sends requests to Google.

use std::{
    error::Error,
    io::{self, Read},
    process::ExitCode,
};

fn run() -> Result<(), Box<dyn Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("status") => println!("{}", nearlane_service::Service::default().status()),
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
            "Nearlane research CLI\n\n  nearlane status\n  nearlane inspect-auth-url  < URL-on-stdin\n\nNo discovery, account login or file transfer is enabled yet."
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
