//! A self-contained native sub window that is owned by the main window.
//!
//! Each `SubWindow` is a separate OS window (an egui *deferred viewport*):
//! - it has its own state and its own drawing code,
//! - it is repainted independently of the main window,
//! - it is a child of the main window in egui's viewport tree, so it
//!   closes automatically when the main window closes.

use std::sync::{Arc, Mutex};

use eframe::egui;

/// State of one sub window. It is shared between the main window (which
/// owns the `SubWindow`) and the sub window's own render callback, which
/// may run at a different time, hence `Arc<Mutex<..>>`.
struct SubWinState {
    /// Set to false when the user closes the window
    open: bool,
    /// Example of window-local state: how often the button was clicked
    clicks: u32,
}

/// Handle to one sub window, kept by the main window.
pub struct SubWindow {
    number: u32,
    viewport_id: egui::ViewportId,
    state: Arc<Mutex<SubWinState>>,
}

impl SubWindow {
    /// Creates a new sub window. `number` must be unique; it is used
    /// for the window title and to build a stable viewport ID.
    pub fn new(number: u32) -> Self {
        Self {
            number,
            viewport_id: egui::ViewportId::from_hash_of(("sub_window", number)),
            state: Arc::new(Mutex::new(SubWinState {
                open: true,
                clicks: 0,
            })),
        }
    }

    /// False once the user has closed the window. The main window should
    /// then drop this `SubWindow`.
    pub fn is_open(&self) -> bool {
        self.state.lock().unwrap().open
    }

    /// Must be called by the main window on every frame while the sub
    /// window should exist. When it is no longer called, egui closes the
    /// OS window.
    pub fn show(&self, ctx: &egui::Context) {
        let number = self.number;
        let state = Arc::clone(&self.state);

        ctx.show_viewport_deferred(
            self.viewport_id,
            egui::ViewportBuilder::default()
                .with_title(format!("WindowExt {number}"))
                .with_inner_size([420.0, 320.0])
                .with_min_inner_size([200.0, 150.0]),
            // This closure is the sub window's own "update" function.
            // It is called by eframe whenever *this* window needs a repaint.
            move |ui, _class| {
                let mut st = state.lock().unwrap();

                egui::CentralPanel::default().show(ui, |ui| {
                    ui.heading(format!("I am sub window #{number}"));
                    ui.label("I have my own state and repaint independently.");
                    ui.add_space(10.0);
                    if ui.button("Click me").clicked() {
                        st.clicks += 1;
                    }
                    ui.label(format!("Clicked {} times", st.clicks));
                });

                // The user clicked the window's close button (X)
                if ui.input(|i| i.viewport().close_requested()) {
                    st.open = false;
                    // Wake up the main window so it removes this sub window
                    ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                }
            },
        );
    }
}
