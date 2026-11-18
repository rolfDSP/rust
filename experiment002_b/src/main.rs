mod subwin;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use slint::ComponentHandle;
use subwin::SubWindow;

// Pulls in the Rust code generated from ui/app.slint by build.rs
// (MainWindow, ChildWindow, ExtWindow).
slint::include_modules!();

/// Returns a human-readable name of the operating system the app runs on.
fn os_name() -> String {
    if cfg!(target_os = "windows") {
        return "Windows".to_string();
    }
    if cfg!(target_os = "linux") {
        // Linux distributions describe themselves in /etc/os-release,
        // e.g. the line `NAME="Ubuntu"`.
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(name) = line.strip_prefix("NAME=") {
                    return name.trim_matches('"').to_string();
                }
            }
        }
        return "Linux".to_string();
    }
    std::env::consts::OS.to_string()
}

/// True when running inside WSL (Windows Subsystem for Linux).
fn running_in_wsl() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WSL_DISTRO_NAME").is_some()
            || std::fs::read_to_string("/proc/version")
                .map(|v| v.to_lowercase().contains("microsoft"))
                .unwrap_or(false))
}

/// All open extra windows, keyed by their number.
#[derive(Default)]
struct Windows {
    children: BTreeMap<i32, ChildWindow>,
    ext: BTreeMap<i32, SubWindow>,
}

fn main() -> Result<(), slint::PlatformError> {
    // WSLg's Wayland compositor can crash with native Wayland clients;
    // force X11 (Xwayland) when running in WSL.
    if running_in_wsl() {
        // SAFETY: first thing in main, no other threads exist yet.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
    }

    let main_window = MainWindow::new()?;
    main_window.set_greeting(format!("Hello from {}", os_name()).into());

    let windows = Rc::new(RefCell::new(Windows::default()));
    let next_number = Rc::new(Cell::new(1));

    // File → New → Window
    {
        let weak_main = main_window.as_weak();
        let windows = windows.clone();
        let next_number = next_number.clone();
        main_window.on_new_window(move || {
            let number = next_number.get();
            next_number.set(number + 1);

            let child = ChildWindow::new().expect("failed to create window");
            child.set_number(number);

            // Remove the window from the list when the user closes it.
            // Dropping it is deferred, because we are inside its own callback.
            let windows_for_close = windows.clone();
            child.window().on_close_requested(move || {
                let windows = windows_for_close.clone();
                slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                    windows.borrow_mut().children.remove(&number);
                });
                slint::CloseRequestResponse::HideWindow
            });

            child.show().expect("failed to show window");
            windows.borrow_mut().children.insert(number, child);
            if let Some(main) = weak_main.upgrade() {
                main.set_status(format!("Opened window {number}").into());
            }
        });
    }

    // File → New → WindowExt (see subwin.rs)
    {
        let weak_main = main_window.as_weak();
        let windows = windows.clone();
        let next_number = next_number.clone();
        main_window.on_new_window_ext(move || {
            let number = next_number.get();
            next_number.set(number + 1);

            let windows_for_close = windows.clone();
            let sub = SubWindow::new(number, move |n| {
                let windows = windows_for_close.clone();
                slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                    windows.borrow_mut().ext.remove(&n);
                });
            })
            .expect("failed to create WindowExt");

            windows.borrow_mut().ext.insert(number, sub);
            if let Some(main) = weak_main.upgrade() {
                main.set_status(format!("Opened WindowExt {number}").into());
            }
        });
    }

    // File → Quit and the main window's X both end the application,
    // which also closes all extra windows.
    main_window.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    main_window.window().on_close_requested(|| {
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });

    main_window.show()?;
    slint::run_event_loop()?;

    // Close any remaining extra windows
    let mut w = windows.borrow_mut();
    for sub in w.ext.values() {
        sub.close();
    }
    for child in w.children.values() {
        let _ = child.hide();
    }
    w.children.clear();
    w.ext.clear();
    Ok(())
}
