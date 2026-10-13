use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 420.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "egui Demo",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

/// All application state lives in this struct.
/// egui is "immediate mode": the UI is rebuilt every frame from this state.
struct MyApp {
    name: String,
    age: u32,
    counter: i32,
    dark_mode: bool,
    todos: Vec<String>,
    new_todo: String,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "Rolf".to_owned(),
            age: 42,
            counter: 0,
            dark_mode: true,
            todos: vec!["Learn egui".to_owned()],
            new_todo: String::new(),
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().set_theme(if self.dark_mode {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });

        // Menu bar at the top
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ui.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.dark_mode, "Dark mode");
                });
            });
        });

        // Settings panel on the left
        egui::Panel::left("settings").show(ui, |ui| {
            ui.heading("Settings");
            ui.separator();
            ui.checkbox(&mut self.dark_mode, "Dark mode");
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("Age"));
        });

        // Main content fills the remaining space
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Hello from egui!");

            ui.horizontal(|ui| {
                ui.label("Your name:");
                ui.text_edit_singleline(&mut self.name);
            });
            ui.label(format!("Hello {}, you are {} years old.", self.name, self.age));

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("−").clicked() {
                    self.counter -= 1;
                }
                ui.label(format!("Counter: {}", self.counter));
                if ui.button("+").clicked() {
                    self.counter += 1;
                }
                if ui.button("Reset").clicked() {
                    self.counter = 0;
                }
            });

            ui.separator();

            ui.label("To-do list:");
            ui.horizontal(|ui| {
                let input = ui.text_edit_singleline(&mut self.new_todo);
                let enter = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("Add").clicked() || enter) && !self.new_todo.trim().is_empty() {
                    self.todos.push(std::mem::take(&mut self.new_todo));
                    input.request_focus();
                }
            });

            let mut remove = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, todo) in self.todos.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.small_button("🗑").clicked() {
                            remove = Some(i);
                        }
                        ui.label(todo);
                    });
                }
            });
            if let Some(i) = remove {
                self.todos.remove(i);
            }
        });
    }
}
