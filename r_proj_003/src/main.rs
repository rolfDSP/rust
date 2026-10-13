use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, Orientation};

const APP_ID: &str = "de.esslinger.r_proj_003";

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    // Vertical layout container with some spacing and margins
    let vbox = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    // Greeting: text entry + button + label
    let entry = Entry::builder().placeholder_text("Enter your name").build();
    let greet_button = Button::with_label("Greet");
    let greeting = Label::new(Some("Hello from Rust + GTK4!"));

    let greeting_clone = greeting.clone();
    let entry_clone = entry.clone();
    greet_button.connect_clicked(move |_| {
        let name = entry_clone.text();
        let name = if name.is_empty() { "stranger".into() } else { name };
        greeting_clone.set_text(&format!("Hello, {name}!"));
    });

    // Counter: label + button with shared state
    let counter = Rc::new(Cell::new(0));
    let count_label = Label::new(Some("Clicked 0 times"));
    let count_button = Button::with_label("Click me");

    let count_label_clone = count_label.clone();
    count_button.connect_clicked(move |_| {
        counter.set(counter.get() + 1);
        count_label_clone.set_text(&format!("Clicked {} times", counter.get()));
    });

    vbox.append(&entry);
    vbox.append(&greet_button);
    vbox.append(&greeting);
    vbox.append(&count_button);
    vbox.append(&count_label);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("r_proj_003 – GTK4 Demo")
        .default_width(360)
        .default_height(260)
        .child(&vbox)
        .build();

    window.present();
}
