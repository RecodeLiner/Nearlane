//! Opt-in software rendering with synthetic state, no native windows or radios.
use crate::{Action, Actions, I18n, MainWindow, Palette, adapter};
use nearlane_application::{Application, Command, Language, Options};
use nearlane_platform::preferences::PreferenceStore;
use slint::{
    ComponentHandle,
    platform::{
        Platform, WindowAdapter,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};
use std::{
    cell::RefCell,
    fs, io,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

struct Headless {
    window: Rc<MinimalSoftwareWindow>,
    start: Instant,
}
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> Duration {
        self.start.elapsed()
    }
}
pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let smoke = args == ["--smoke-test"];
    let preview = args.first().is_some_and(|a| a == "--preview") && args.len() >= 2;
    if !smoke && !preview {
        return Err("Usage: nearlane-slint [--smoke-test | --preview /absolute/image.png [--compact] [--dark] [--page send|receive|settings] [--scene devices|incoming] [--language en|ru]]".into());
    }
    let root = tempfile::tempdir()?;
    let config = root.path().join("runtime/oauth-client.json");
    let store = PreferenceStore::at(root.path().join("preferences.json"));
    let app = Application::start(Options::isolated(Some(config), Some(store)))?;
    let surface = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless {
        window: Rc::clone(&surface),
        start: Instant::now(),
    }))
    .map_err(|_| "Could not initialize headless renderer")?;
    let window = MainWindow::new()?;
    let displayed = Rc::new(RefCell::new(app.snapshot()));
    adapter::translations(window.global::<I18n>());
    adapter::actions(
        window.global::<Actions>(),
        app.sender(),
        Rc::clone(&displayed),
        window.as_weak(),
    );
    let apply = || {
        let snapshot = app.snapshot();
        window
            .global::<I18n>()
            .set_language(snapshot.effective_language.as_str().into());
        window.set_state(adapter::state(&snapshot));
        *displayed.borrow_mut() = snapshot;
    };
    let flush = || app.flush(Duration::from_secs(5));
    apply();
    if smoke {
        assert!(window.get_state().hidden && !window.get_state().account_authorized);
        for (code, label) in [("ru", "Отправить"), ("en", "Send")] {
            window.global::<Actions>().invoke_language(code.into());
            flush()?;
            apply();
            assert_eq!(
                window
                    .global::<I18n>()
                    .invoke_lookup("Send".into(), code.into()),
                label
            );
        }
        window.global::<Actions>().invoke_language("invalid".into());
        flush()?;
        apply();
        assert_eq!(window.get_state().language, "en");
        let source = root.path().join("selected.json");
        fs::write(&source, br#"{"client_id":"slint-isolated-fixture"}"#)?;
        assert!(app.sender().send(Command::ImportConfiguration(source)));
        flush()?;
        apply();
        assert!(window.get_state().account_configured);
        window.global::<Actions>().invoke_action(Action::SignIn);
        flush()?;
        apply();
        assert!(window.get_state().account_busy);
        window
            .global::<Actions>()
            .invoke_action(Action::CancelSignIn);
        flush()?;
        apply();
        assert!(!window.get_state().account_busy && window.get_state().hidden);
        window.global::<Actions>().invoke_receiving(false, true);
        flush()?;
        apply();
        assert!(!window.get_state().background_receiving && window.get_state().confirm_each_file);
        window
            .global::<Actions>()
            .invoke_action(Action::CheckContacts);
        flush()?;
        apply();
        assert!(window.get_state().hidden);
        for page in 0..3 {
            window.set_page(page);
        }
        println!(
            "PASS: Slint callbacks, live en/ru catalogs, synthetic import/login cancellation, private receiving settings; no account/radio access"
        );
    }
    let value = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    if let Some(code) = value("--language") {
        let language = Language::parse(code).ok_or("Unsupported preview language")?;
        let _ = app.sender().send(Command::SetLanguage(language));
        flush()?;
        apply();
    }
    if args.iter().any(|a| a == "--dark") {
        window
            .global::<Palette>()
            .set_color_scheme(slint::language::ColorScheme::Dark);
    }
    if let Some(scene) = value("--scene") {
        let mut snapshot = app.snapshot();
        snapshot.account_authorized = true;
        snapshot.discovery_available = true;
        snapshot.receiving_active = true;
        snapshot.selected_send_name = "example.txt".into();
        snapshot.receive_directory = "/home/demo/Downloads/Nearlane".into();
        snapshot.own_devices = vec!["Demo phone".into()];
        if scene == "incoming" {
            snapshot.receive_pending = true;
            snapshot.incoming_offer = "Demo phone\nexample.txt\n94 B".into();
        }
        window.set_state(adapter::state(&snapshot));
    }
    let page = match value("--page") {
        Some("receive") => 1,
        Some("settings" | "about") => 2,
        _ => 0,
    };
    window.set_page(page);
    let width = if args.iter().any(|a| a == "--compact") {
        480
    } else {
        980
    };
    let height = 820;
    surface.set_size(slint::PhysicalSize::new(width, height));
    window.show()?;
    if value("--page") == Some("about") {
        window.invoke_show_about();
        std::thread::sleep(Duration::from_millis(70));
        slint::platform::update_timers_and_animations();
        std::thread::sleep(Duration::from_millis(250));
    }
    slint::platform::update_timers_and_animations();
    let mut pixels = vec![slint::Rgb8Pixel::default(); (width * height) as usize];
    assert!(surface.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, width as usize);
    }));
    assert!(
        pixels.windows(2).any(|p| p[0] != p[1]),
        "UI must render non-uniform pixels"
    );
    if preview {
        let path = PathBuf::from(&args[1]);
        if !path.is_absolute() {
            return Err(io::Error::other("Preview path must be absolute").into());
        }
        let bytes = pixels
            .into_iter()
            .flat_map(|p| [p.r, p.g, p.b])
            .collect::<Vec<_>>();
        image::save_buffer_with_format(
            &path,
            &bytes,
            width,
            height,
            image::ColorType::Rgb8,
            image::ImageFormat::Png,
        )?;
        println!("PASS: rendered synthetic Slint UI to {}", path.display());
    } else {
        println!("PASS: software-rendered Slint screens without native windows");
    }
    window.hide()?;
    Ok(())
}
