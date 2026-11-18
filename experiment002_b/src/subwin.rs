//! A self-contained native sub window (File → New → WindowExt).
//!
//! The UI layout is the `ExtWindow` component in `ui/app.slint`; this module
//! owns the window instance and its Rust-side state and logic.

use std::cell::Cell;
use std::rc::Rc;

use slint::ComponentHandle;

use crate::ExtWindow;

/// One sub window with its own state.
pub struct SubWindow {
    window: ExtWindow,
}

impl SubWindow {
    /// Creates and shows a new sub window.
    /// `on_closed` is called with the window's number when the user closes it.
    pub fn new(number: i32, on_closed: impl Fn(i32) + 'static) -> Result<Self, slint::PlatformError> {
        let window = ExtWindow::new()?;
        window.set_number(number);

        // Window-local state, kept on the Rust side
        let clicks = Rc::new(Cell::new(0));
        let weak = window.as_weak();
        window.on_clicked(move || {
            clicks.set(clicks.get() + 1);
            if let Some(w) = weak.upgrade() {
                w.set_clicks(clicks.get());
            }
        });

        // The user clicked the window's close button (X)
        window.window().on_close_requested(move || {
            on_closed(number);
            slint::CloseRequestResponse::HideWindow
        });

        window.show()?;
        Ok(Self { window })
    }

    /// Closes the window (used when the main window shuts down).
    pub fn close(&self) {
        let _ = self.window.hide();
    }
}
