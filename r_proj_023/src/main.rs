use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow, DrawingArea};

const APP_ID: &str = "org.example.RedTriangle";

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let drawing_area = DrawingArea::new();
    drawing_area.set_draw_func(draw_triangle);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Red Triangle")
        .default_width(640)
        .default_height(480)
        .resizable(true)
        .child(&drawing_area)
        .build();

    window.present();
}

/// Called by GTK whenever the area needs to be redrawn (e.g. after a resize).
fn draw_triangle(_area: &DrawingArea, cr: &gtk::cairo::Context, width: i32, height: i32) {
    let (w, h) = (width as f64, height as f64);

    // Black background
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.paint().expect("failed to paint background");

    // Equilateral triangle centered in the window, scaled to fit with a margin
    let size = w.min(h) * 0.8;
    let tri_height = size * 3f64.sqrt() / 2.0;
    let (cx, cy) = (w / 2.0, h / 2.0);

    cr.move_to(cx, cy - tri_height / 2.0); // top
    cr.line_to(cx + size / 2.0, cy + tri_height / 2.0); // bottom right
    cr.line_to(cx - size / 2.0, cy + tri_height / 2.0); // bottom left
    cr.close_path();

    // Red fill
    cr.set_source_rgb(1.0, 1.0, 0.0);
    cr.fill().expect("failed to fill triangle");
}
