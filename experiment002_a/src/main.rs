use eframe::egui;

fn main() -> eframe::Result {
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
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            status: String::new(),
            show_about: false,
            // Determined once at startup, not on every frame
            greeting: format!("Hello from {}", os_name()),
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
                    if ui.button("New").clicked() {
                        self.status = "File → New clicked".into();
                    }
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
    }
}
