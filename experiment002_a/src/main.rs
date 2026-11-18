use eframe::egui;

mod subwin;
use subwin::SubWindow;

/// True when running inside WSL (Windows Subsystem for Linux).
fn running_in_wsl() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WSL_DISTRO_NAME").is_some()
            || std::fs::read_to_string("/proc/version")
                .map(|v| v.to_lowercase().contains("microsoft"))
                .unwrap_or(false))
}

fn main() -> eframe::Result {
    // WSLg's Wayland compositor (Weston) is unstable with native Wayland
    // clients and can crash ("Connection reset by peer"). Its X11 support
    // (Xwayland) is much more robust, so force X11 when running in WSL.
    // Without WAYLAND_DISPLAY, winit falls back to X11 via DISPLAY.
    if running_in_wsl() {
        // SAFETY: called at the very start of main, before any other
        // threads exist that could read the environment concurrently.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("My Rust App")
            .with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "My Rust App",
        options,
        Box::new(|cc| {
            apply_dark_blue_theme(&cc.egui_ctx);
            Ok(Box::new(MyApp::default()))
        }),
    )
}

/// Configures a dark blue color theme for the whole application.
fn apply_dark_blue_theme(ctx: &egui::Context) {
    use egui::{Color32, Stroke};

    // Palette
    let bg_darkest = Color32::from_rgb(8, 14, 30); // text fields, scroll backgrounds
    let bg_panel = Color32::from_rgb(14, 24, 48); // main window background
    let bg_window = Color32::from_rgb(20, 34, 66); // dialogs, menus
    let widget = Color32::from_rgb(28, 48, 92); // buttons at rest
    let widget_hover = Color32::from_rgb(40, 68, 128);
    let widget_active = Color32::from_rgb(56, 92, 168);
    let accent = Color32::from_rgb(90, 160, 255); // selection, links
    let border = Color32::from_rgb(44, 66, 110);
    let text = Color32::from_rgb(220, 230, 250);

    // Start from egui's dark theme and override the colors
    let mut v = egui::Visuals::dark();
    v.panel_fill = bg_panel;
    v.window_fill = bg_window;
    v.window_stroke = Stroke::new(1.0, border);
    v.extreme_bg_color = bg_darkest;
    v.faint_bg_color = bg_window;
    v.code_bg_color = bg_darkest;
    v.hyperlink_color = accent;
    v.selection.bg_fill = accent.linear_multiply(0.5);
    v.selection.stroke = Stroke::new(1.0, accent);

    // Non-interactive widgets: labels, separators, panel borders
    v.widgets.noninteractive.bg_fill = bg_panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);

    // Buttons, menu entries, ... in their different states
    for (w, fill) in [
        (&mut v.widgets.inactive, widget),
        (&mut v.widgets.hovered, widget_hover),
        (&mut v.widgets.active, widget_active),
        (&mut v.widgets.open, widget_hover),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.fg_stroke = Stroke::new(1.0, text);
    }
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, accent);
    v.widgets.active.bg_stroke = Stroke::new(1.0, accent);

    // Always use the dark theme (ignore the OS light/dark setting)
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);
}

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
    // Any other OS: "macos", "freebsd", ...
    std::env::consts::OS.to_string()
}

struct MyApp {
    status: String,
    show_about: bool,
    greeting: String,
    /// IDs of the extra windows that are currently open
    child_windows: Vec<u32>,
    /// Counter used to give each new window a unique ID and title
    next_window_id: u32,
    /// Independent sub windows created via File → New → WindowExt
    ext_windows: Vec<SubWindow>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            status: String::new(),
            show_about: false,
            // Determined once at startup, not on every frame
            greeting: format!("Hello from {}", os_name()),
            child_windows: Vec::new(),
            next_window_id: 1,
            ext_windows: Vec::new(),
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Main menu bar at the top of the window
        egui::Panel::top("main_menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.menu_button("New", |ui| {
                        if ui.button("Window").clicked() {
                            let id = self.next_window_id;
                            self.next_window_id += 1;
                            self.child_windows.push(id);
                            self.status = format!("Opened window {id}");
                        }
                        if ui.button("WindowExt").clicked() {
                            let id = self.next_window_id;
                            self.next_window_id += 1;
                            self.ext_windows.push(SubWindow::new(id));
                            self.status = format!("Opened WindowExt {id}");
                        }
                    });
                    if ui.button("Open…").clicked() {
                        self.status = "File → Open clicked".into();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.show_about = true;
                    }
                });
            });
        });

        // Main content area
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(&self.greeting);
            ui.label(format!("Last action: {}", self.status));
        });

        // Simple "About" dialog
        egui::Window::new("About")
            .open(&mut self.show_about)
            .collapsible(false)
            .resizable(false)
            .show(&ctx, |ui| {
                ui.label("A minimal egui/eframe example.");
            });

        // Extra native OS windows created via File → New → Window
        self.child_windows.retain(|&id| {
            let mut keep_open = true;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("child_window", id)),
                egui::ViewportBuilder::default()
                    .with_title(format!("Window {id}"))
                    .with_inner_size([400.0, 300.0]),
                |ui, _class| {
                    // Panel without inner margin, so it covers the whole draw area
                    let frame = egui::Frame::central_panel(ui.style()).inner_margin(0.0);
                    egui::CentralPanel::default().frame(frame).show(ui, |ui| {
                        // Full draw area of the window, recomputed every frame,
                        // so the border follows when the window is resized
                        let rect = ui.max_rect().shrink(5.0);
                        let painter = ui.painter();

                        // White 1 px border, 5 px away from the window edges
                        painter.rect_stroke(
                            rect,
                            0.0, // no rounded corners
                            egui::Stroke::new(1.0, egui::Color32::WHITE),
                            egui::StrokeKind::Inside,
                        );

                        // Text centered in the window
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Hello, Child",
                            egui::FontId::proportional(24.0),
                            egui::Color32::WHITE,
                        );
                    });

                    // User clicked the window's close button (X)
                    if ui.input(|i| i.viewport().close_requested()) {
                        keep_open = false;
                    }
                },
            );
            keep_open
        });

        // Independent sub windows (see subwin.rs): drop closed ones,
        // then keep showing the remaining ones
        self.ext_windows.retain(|w| w.is_open());
        for w in &self.ext_windows {
            w.show(&ctx);
        }
    }
}
