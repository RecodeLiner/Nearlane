use super::*;
use nearlane_platform::preferences::PreferenceStore;
use std::{fs, io};

fn send(app: &Application, command: Command) -> io::Result<()> {
    assert!(app.sender().send(command));
    app.flush(Duration::from_secs(3))
}
#[test]
fn shared_controller_preserves_settings_across_frontend_lifetimes_without_trust() -> io::Result<()>
{
    let root = tempfile::tempdir()?;
    let store = PreferenceStore::at(root.path().join("preferences.json"));
    {
        let app = Application::start(Options::isolated(None, Some(store.clone())))?;
        assert!(app.snapshot().hidden && !app.snapshot().account_authorized);
        send(&app, Command::SetLanguage(Language::Ru))?;
        send(
            &app,
            Command::ConfigureReceiving {
                enabled: true,
                confirm: true,
            },
        )?;
        send(&app, Command::SetDiscovery(true))?;
        send(&app, Command::SetSendSurface(true))?;
        send(
            &app,
            Command::DecideIncoming {
                request_id: 99,
                accept: true,
            },
        )?;
        send(
            &app,
            Command::SendFile {
                index: 0,
                devices_revision: u64::MAX,
            },
        )?;
        send(&app, Command::CheckContacts)?;
        let snapshot = app.snapshot();
        assert!(snapshot.hidden && !snapshot.receiving_active && snapshot.own_devices.is_empty());
        assert_eq!(snapshot.effective_language, "ru");
        assert!(snapshot.status.contains("контактов"));
    }
    let app = Application::start(Options::isolated(None, Some(store.clone())))?;
    assert_eq!(app.snapshot().language, "ru");
    let saved = store.load()?;
    assert!(saved.background_receiving && saved.confirm_each_file && saved.continuous_discovery);
    send(&app, Command::SetLanguage(Language::En))?;
    assert!(app.snapshot().status.starts_with("Platform:"));
    assert!(app.snapshot().hidden && !app.snapshot().account_authorized);
    Ok(())
}
#[test]
fn configuration_import_survives_source_removal_and_requires_explicit_sign_in() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("selected.json");
    let destination = root.path().join("runtime/oauth-client.json");
    fs::write(&source, br#"{"client_id":"isolated-controller-fixture"}"#)?;
    {
        let app = Application::start(Options::isolated(Some(destination.clone()), None))?;
        assert!(!app.snapshot().account_configured);
        send(&app, Command::ImportConfiguration(source.clone()))?;
        assert!(
            app.snapshot().account_configured
                && !app.snapshot().account_busy
                && !app.snapshot().config_error
        );
        fs::write(&source, b"invalid json")?;
        send(&app, Command::ImportConfiguration(source.clone()))?;
        assert!(app.snapshot().account_configured && app.snapshot().config_error);
    }
    fs::remove_file(source)?;
    let app = Application::start(Options::isolated(Some(destination), None))?;
    assert!(
        app.snapshot().account_configured
            && !app.snapshot().account_authorized
            && app.snapshot().hidden
    );
    send(&app, Command::SignIn)?;
    assert!(app.snapshot().account_busy && app.snapshot().hidden);
    let replacement = root.path().join("replacement.json");
    fs::write(
        &replacement,
        br#"{"client_id":"different-isolated-fixture"}"#,
    )?;
    send(&app, Command::ImportConfiguration(replacement))?;
    assert!(app.snapshot().account_busy && app.snapshot().config_error);
    send(&app, Command::CancelSignIn)?;
    assert!(!app.snapshot().account_busy && app.snapshot().hidden);
    Ok(())
}
