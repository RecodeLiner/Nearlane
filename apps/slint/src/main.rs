//! Slint presentation adapter. Application behavior lives in nearlane-application.
mod adapter;
mod headless;
// Framework-generated vtables only; handwritten adapters still deny unsafe.
#[allow(unsafe_code, clippy::unwrap_used, clippy::expect_used)]
mod ui {
    slint::include_modules!();
}
use ui::*;

use nearlane_application::{Application, Command, Options};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if !arguments.is_empty() {
        return headless::run(&arguments);
    }
    // Winit is explicitly selected at build time: this frontend has no Qt.
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    slint::set_xdg_app_id("org.nearlane.Nearlane")?;
    let app = Rc::new(Application::start(Options::desktop())?);
    let window = MainWindow::new()?;
    let displayed = Rc::new(RefCell::new(app.snapshot()));
    adapter::translations(window.global::<I18n>());
    adapter::actions(
        window.global::<Actions>(),
        app.sender(),
        Rc::clone(&displayed),
        window.as_weak(),
    );
    window
        .global::<I18n>()
        .set_language(displayed.borrow().effective_language.as_str().into());
    window.set_state(adapter::state(&displayed.borrow()));
    let tray = Tray::new().ok();
    if let Some(tray) = &tray {
        adapter::translations(tray.global::<I18n>());
        adapter::actions(
            tray.global::<Actions>(),
            app.sender(),
            Rc::clone(&displayed),
            window.as_weak(),
        );
        tray.global::<I18n>()
            .set_language(displayed.borrow().effective_language.as_str().into());
        tray.set_state(adapter::state(&displayed.borrow()));
    }
    let tx = app.sender();
    let can_hide = tray.is_some();
    window.window().on_close_requested(move || {
        let _ = tx.send(Command::SetSendSurface(false));
        if !can_hide {
            let _ = slint::quit_event_loop();
        }
        slint::CloseRequestResponse::HideWindow
    });
    let timer = slint::Timer::default();
    let weak = window.as_weak();
    let tray_weak = tray.as_ref().map(Tray::as_weak);
    let controller = Rc::clone(&app);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(150),
        move || {
            let snapshot = controller.snapshot();
            if snapshot.revision == displayed.borrow().revision {
                return;
            }
            if let Some(window) = weak.upgrade() {
                let show_offer = snapshot.receive_pending
                    && !snapshot.actionable_notification
                    && (snapshot.incoming_request_id != displayed.borrow().incoming_request_id
                        || displayed.borrow().actionable_notification);
                window
                    .global::<I18n>()
                    .set_language(snapshot.effective_language.as_str().into());
                window.set_state(adapter::state(&snapshot));
                if show_offer {
                    window.set_page(1);
                    let _ = window.show();
                }
            }
            if let Some(tray) = tray_weak.as_ref().and_then(slint::Weak::upgrade) {
                tray.global::<I18n>()
                    .set_language(snapshot.effective_language.as_str().into());
                tray.set_state(adapter::state(&snapshot));
            }
            *displayed.borrow_mut() = snapshot;
        },
    );
    window.show()?;
    let _ = app.sender().send(Command::SetSendSurface(true));
    slint::run_event_loop()?;
    timer.stop();
    drop(timer);
    drop(window);
    drop(tray);
    drop(app);
    Ok(())
}
