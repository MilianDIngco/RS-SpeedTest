use iced::{
    alignment, 
    widget::{button, column, container, image, row, text, text_input, toggler, Stack},
    Background, Border, Color, Element, Length, Size, Task, Theme
};

fn main() -> iced::Result {
    iced::application("Speed Testing", NetworkConfigApp::update, NetworkConfigApp::view)
        .window_size(Size::new(800.0, 600.0))
        .theme(|_| Theme::Dark)
        .run_with(|| NetworkConfigApp::new())
}

#[derive(Debug, Clone)]
pub enum Message {
    IpAddressChanged(String),
    PortChanged(String),
    ProtocolToggled(bool),
    Connect,
}

#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    UDP,
    TCP,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::UDP => write!(f, "UDP"),
            Protocol::TCP => write!(f, "TCP"),
        }
    }
}

#[derive(Debug)]
pub struct NetworkConfigApp {
    ip_address: String,
    port: String,
    is_tcp: bool,
    background_image: iced::widget::image::Handle,
}

impl NetworkConfigApp {
    fn new() -> (Self, Task<Message>) {
        let background_image = image::Handle::from_path("TCNJ_Speed.jpg");
        
        (
            Self {
                ip_address: String::from("127.0.0.1"),
                port: String::from("8080"),
                is_tcp: true,
                background_image,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IpAddressChanged(value) => {
                self.ip_address = value;
            }
            Message::PortChanged(value) => {
                self.port = value;
            }
            Message::ProtocolToggled(is_tcp) => {
                self.is_tcp = is_tcp;
            }
            Message::Connect => {
                let protocol = if self.is_tcp { Protocol::TCP } else { Protocol::UDP };
                println!(
                    "Connecting to {}:{} using {}",
                    self.ip_address, self.port, protocol
                );
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        let protocol = if self.is_tcp { Protocol::TCP } else { Protocol::UDP };
        
        // Background image
        let background = image(self.background_image.clone())
            .width(Length::Fill)
            .height(Length::Fill);

        // Main content container with semi-transparent background
        let content = container(
            column![
                // Title
                text("Speed Testing")
                    .size(32)
                    .color(Color::WHITE),

                // Increased spacing to push inputs lower
                text("").size(100),

                // IP Address input
                column![
                    text("IP Address:")
                        .size(16)
                        .color(Color::WHITE),
                    text_input("Enter IP address", &self.ip_address)
                        .on_input(Message::IpAddressChanged)
                        .padding(10)
                        .size(16)
                        .width(Length::Fixed(300.0))
                ]
                .spacing(5),

                // Port input
                column![
                    text("Port:")
                        .size(16)
                        .color(Color::WHITE),
                    text_input("Enter port", &self.port)
                        .on_input(Message::PortChanged)
                        .padding(10)
                        .size(16)
                        .width(Length::Fixed(300.0))
                ]
                .spacing(5),

                // Protocol toggle
                row![
                    text("Protocol:")
                        .size(16)
                        .color(Color::WHITE),
                    text("UDP")
                        .size(16)
                        .color(Color::WHITE),
                    toggler(self.is_tcp)
                        .on_toggle(Message::ProtocolToggled)
                        .size(25),
                    text("TCP")
                        .size(16)
                        .color(Color::WHITE),
                ]
                .spacing(10)
                .align_y(alignment::Vertical::Center),

                // Current selection display
                text(format!(
                    "Selected: {}:{} ({})",
                    self.ip_address, self.port, protocol
                ))
                .size(14)
                .color(Color::from_rgb(0.8, 0.8, 1.0)),

                // Connect button
                button(
                    text("Connect")
                        .size(18)
                )
                .on_press(Message::Connect)
                .padding(15)
                .width(Length::Fixed(150.0))
            ]
            .spacing(20)
            .align_x(alignment::Horizontal::Center)
        )
        .width(Length::Fixed(400.0))
        .height(Length::Shrink)
        .padding(30)
        .style(|_theme| {
            container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.7))),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }
        });

        // Stack background and content
        container(
            Stack::new()
                .push(background)
                .push(
                    container(content)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                )
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
