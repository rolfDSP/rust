use iced::widget::{
    button, center, column, container, mouse_area, opaque, row, space, stack, text, text_input,
};
use iced::{Center, Element, Fill, Task};

const MENU_BAR_HEIGHT: f32 = 32.0;

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Iced Example")
        .window_size((400.0, 300.0))
        .run()
}

#[derive(Default)]
struct App {
    counter: i32,
    name: String,
    open_menu: Option<Menu>,
    show_about: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Menu {
    File,
    Help,
}

#[derive(Debug, Clone)]
enum Message {
    Increment,
    Decrement,
    Reset,
    NameChanged(String),
    ToggleMenu(Menu),
    CloseMenu,
    Quit,
    ShowAbout,
    CloseAbout,
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Increment => self.counter += 1,
            Message::Decrement => self.counter -= 1,
            Message::Reset => self.counter = 0,
            Message::NameChanged(name) => self.name = name,
            Message::ToggleMenu(menu) => {
                self.open_menu = if self.open_menu == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
            }
            Message::CloseMenu => self.open_menu = None,
            Message::Quit => return iced::exit(),
            Message::ShowAbout => {
                self.open_menu = None;
                self.show_about = true;
            }
            Message::CloseAbout => self.show_about = false,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let mut layers = stack![self.base_layer()].width(Fill).height(Fill);

        if self.open_menu.is_some() {
            // Invisible full-window layer: clicking anywhere outside the menu closes it.
            layers = layers.push(opaque(
                mouse_area(space().width(Fill).height(Fill)).on_press(Message::CloseMenu),
            ));
        }

        layers = layers.push(self.menu_bar());

        if self.show_about {
            layers = layers.push(self.about_dialog());
        }

        layers.into()
    }

    /// Menu bar background plus the main content below it.
    fn base_layer(&self) -> Element<'_, Message> {
        let bar_background = container(space())
            .width(Fill)
            .height(MENU_BAR_HEIGHT)
            .style(container::bordered_box);

        column![bar_background, self.content()].into()
    }

    /// The menu titles, each with its dropdown (if open) floating over the content.
    fn menu_bar(&self) -> Element<'_, Message> {
        row![
            self.menu(Menu::File, "File", vec![("Quit", Message::Quit)]),
            self.menu(Menu::Help, "Help", vec![("About", Message::ShowAbout)]),
        ]
        .into()
    }

    fn menu(
        &self,
        menu: Menu,
        title: &'static str,
        items: Vec<(&'static str, Message)>,
    ) -> Element<'_, Message> {
        let title_button = button(text(title))
            .height(MENU_BAR_HEIGHT)
            .padding([6, 12])
            .style(button::text)
            .on_press(Message::ToggleMenu(menu));

        if self.open_menu != Some(menu) {
            return title_button.into();
        }

        let entries = items.into_iter().map(|(label, message)| {
            button(text(label))
                .width(Fill)
                .padding([4, 12])
                .style(button::text)
                .on_press(message)
                .into()
        });

        let dropdown = container(column(entries).width(120))
            .padding(2)
            .style(container::bordered_box);

        column![title_button, dropdown].into()
    }

    fn content(&self) -> Element<'_, Message> {
        let greeting = if self.name.is_empty() {
            "Hello, stranger!".to_string()
        } else {
            format!("Hello, {}!", self.name)
        };

        column![
            text_input("Enter your name...", &self.name).on_input(Message::NameChanged),
            text(greeting).size(24),
            text(format!("Counter: {}", self.counter)).size(40),
            row![
                button("-").on_press(Message::Decrement),
                button("Reset").on_press(Message::Reset),
                button("+").on_press(Message::Increment),
            ]
            .spacing(10),
        ]
        .spacing(20)
        .padding(20)
        .width(Fill)
        .align_x(Center)
        .into()
    }

    fn about_dialog(&self) -> Element<'_, Message> {
        let dialog = container(
            column![
                text("Iced Example").size(20),
                text("A small demo built with the iced GUI library."),
                button("OK").on_press(Message::CloseAbout),
            ]
            .spacing(12)
            .align_x(Center),
        )
        .padding(20)
        .width(300)
        .style(container::bordered_box);

        // Dimmed backdrop that blocks interaction with the rest of the window.
        opaque(
            mouse_area(center(opaque(dialog)).style(|_theme| {
                container::background(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5))
            }))
            .on_press(Message::CloseAbout),
        )
    }
}
