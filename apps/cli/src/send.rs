use nearlane_service::{AccountStatus, DiscoveryStatus, PrivateStatus, Service};
use std::{
    error::Error,
    ffi::OsString,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(args: impl Iterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    let (name, path) = arguments(args)?;
    let mut service = Service::default();
    println!("Restoring this app's saved sign-in; no browser authorization will be started.");
    service.restore_saved_sign_in()?;
    let setup_deadline = Instant::now() + Duration::from_secs(90);
    loop {
        service.poll_account();
        match service.account_status() {
            AccountStatus::Restoring | AccountStatus::Authorized => {}
            status => return Err(status.message().into()),
        }
        match service.private_status() {
            PrivateStatus::Ready(_) => break,
            PrivateStatus::Failed(error) => return Err(error.into()),
            _ => {}
        }
        if Instant::now() >= setup_deadline {
            return Err("Saved sign-in/device preparation timed out. No file was sent.".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
    println!("Own-device certificates ready. Looking for the exact requested name for 20 seconds…");
    service.find_own_devices();
    let search_deadline = Instant::now() + Duration::from_secs(25);
    while service.discovery_status() == DiscoveryStatus::Searching {
        if Instant::now() >= search_deadline {
            return Err("Own-device discovery timed out. No file was sent.".into());
        }
        thread::sleep(Duration::from_millis(100));
        service.poll_account();
    }
    if let DiscoveryStatus::Failed(error) = service.discovery_status() {
        return Err(error.into());
    }
    let index = unique_match(&service.own_device_names(), &name)?;
    println!(
        "Matched one account-trusted candidate. Starting live verification and requesting receiver consent."
    );
    service.select_send_file(path);
    service.send_selected_file(index);
    let transfer_deadline = Instant::now() + Duration::from_secs(195);
    let mut last = String::new();
    loop {
        service.poll_account();
        let message = service.transfer_status();
        if message != last {
            println!("{message}");
            last = message.to_owned();
        }
        if !service.transfer_busy() {
            // Keep the CLI's exit status tied to the typed worker result, not
            // whether text happened to contain a success-looking substring.
            return if service.transfer_completed() {
                Ok(())
            } else {
                Err("Outgoing transfer was not confirmed; see the stage above.".into())
            };
        }
        if Instant::now() >= transfer_deadline {
            service.cancel_transfer();
            return Err("Transfer timed out. Check the phone before retrying.".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn arguments(args: impl Iterator<Item = OsString>) -> Result<(String, PathBuf), Box<dyn Error>> {
    let mut args = args;
    let usage = "Usage: nearlane send-own --device EXACT_NAME FILE";
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--device")) {
        return Err(usage.into());
    }
    let name = args
        .next()
        .and_then(|n| n.into_string().ok())
        .ok_or(usage)?;
    let path = args.next().map(PathBuf::from).ok_or(usage)?;
    if name.trim().is_empty() || name.len() > 128 || args.next().is_some() {
        return Err(usage.into());
    }
    Ok((name, path))
}

fn unique_match(names: &[&str], expected: &str) -> Result<usize, Box<dyn Error>> {
    let mut found = names
        .iter()
        .enumerate()
        .filter(|(_, name)| name.eq_ignore_ascii_case(expected));
    let index = found
        .next()
        .map(|(i, _)| i)
        .ok_or("Requested own device was not found. No other recipient was selected.")?;
    if found.next().is_some() {
        return Err(
            "More than one own device has that name. Use the desktop to select a recipient.".into(),
        );
    }
    Ok(index)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn never_chooses_a_similar_or_ambiguous_device_name() {
        assert!(unique_match(&["Recode's phone 2"], "Recode's phone").is_err());
        assert!(unique_match(&["Phone", "phone"], "Phone").is_err());
        assert_eq!(
            unique_match(&["Another", "Recode's phone"], "recode's phone").unwrap(),
            1
        );
        assert!(
            arguments(
                ["--device", "Phone", "file", "extra"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_err()
        );
    }
}
