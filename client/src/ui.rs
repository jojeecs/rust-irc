use crate::event::Event::Crossterm;
use crate::event::UIEvent::{Login, MessageReceived, PostMessage, Quit};
use crate::event::{Event, EventHandler, LoginEvent};
use crate::ui::Screen::{HomePage, LoginPage};
use common::ClientPacket::{
    Disconnect, Handshake, PrivateMessage, PublicMessage,
};
use common::HandshakePacket::{ClientLogin, ClientUsername};
use common::{ClientPacket, LoginInfo};
use crossterm::event;
use crossterm::event::Event::Key;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, ToLine};
use ratatui::widgets::{Block, Paragraph, Widget};
use ratatui::{DefaultTerminal, Frame};
use ratatui::style::Color::{Red, White};
use sha3::{Digest, Sha3_256};
use tokio::sync::mpsc::{Receiver, Sender};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler as EvtHandler;

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub events: EventHandler,
    pub messages: Vec<String>,
    pub ui_rx: Receiver<ClientPacket>,
    pub socket_tx: Sender<ClientPacket>,
    pub login: LoginInfo,
}

struct Button<'a> {
    label: Line<'a>,
    theme: Theme
}

struct Theme {
    text: Color,
    highlight: Color,
    background: Color,
    shadow: Color

}

#[derive(Debug)]
pub enum Screen {
    LoginPage(LoginScreen),
    HomePage(HomeScreen),
}

#[derive(Debug)]
pub struct HomeScreen {
    pub messages: Vec<String>,
    pub input: Input,
}

#[derive(Debug)]
pub struct LoginScreen {
    username_input: Input,
    password_input: Input,
    confirm_password_input: Input,
    editing_username: bool,
    username_sent: bool,
    new_user: bool,
    password_accepted: bool,
}

impl LoginScreen {
    fn new() -> Self {
        LoginScreen {
            username_input: Input::default(),
            password_input: Input::default(),
            confirm_password_input: Input::default(),
            editing_username: true,
            username_sent: false,
            new_user: false,
            password_accepted: true,
        }
    }
}

impl App {
    pub fn new(ui_rx: Receiver<ClientPacket>, socket_tx: Sender<ClientPacket>) -> Self {
        Self {
            running: true,
            screen:LoginPage(LoginScreen::new()),
            events: EventHandler::new(),
            messages: Vec::new(),
            ui_rx,
            socket_tx,
            login: LoginInfo {
                username: String::new(),
                password: String::new(),
            },
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        let mut prompt;

        while self.running {
            match &self.screen {
                LoginPage(login) => {
                    if login.new_user {
                        prompt = "Create New User";
                    } else {
                        prompt = "Login";
                    }
                },
                HomePage(_) => {
                    prompt = "Home";
                },
            }
            terminal.draw(|frame| self.render(frame, prompt))?;

            match self.events.next().await? {
                Crossterm(event) => match event {
                    Key(key_event) if key_event.kind == event::KeyEventKind::Press => {
                        self.screen.handle_input(key_event, &mut self.events)?;
                    }
                    _ => {}
                },
                Event::Ui(Quit) => {
                    self.running = false;
                    let _ = self.socket_tx.send(Disconnect).await;
                }
                Event::Ui(Login(event)) => match event {
                    LoginEvent::Username(username) => {
                        self.login.username = username.clone();
                        if let Err(e) = self
                            .socket_tx
                            .send(Handshake {
                                handshake_packet: ClientUsername { username },
                            })
                            .await
                        {
                            eprintln!("Error occurred sending packet: {}", e);
                        }
                    }
                    LoginEvent::Password(password) => {
                        self.login.password = password.clone();
                        let login_info = LoginInfo {
                            username: self.login.username.clone(),
                            password,
                        };
                        if let Err(e) = self
                            .socket_tx
                            .send(Handshake {
                                handshake_packet: ClientLogin { login_info },
                            })
                            .await
                        {
                            eprintln!("Error: {}", e);
                        };
                    }
                },
                Event::Ui(PostMessage(contents)) => {
                    self.socket_tx.send(raw_msg_to_packet(contents)).await?;
                }
                Event::Ui(MessageReceived(packet)) => match packet {
                    PublicMessage { contents } => {
                        if let HomePage(home) = &mut self.screen {
                            home.messages.push(contents);
                        }
                    },
                    ClientPacket::ConnectionAccepted => {
                        self.screen = HomePage(HomeScreen {
                            messages: self.messages.clone(),
                            input: Default::default(),
                        });
                    },
                    ClientPacket::ConnectionRejected {..} => {
                        if let LoginPage(login) = &mut self.screen {
                            login.password_accepted = false;
                        }
                    },
                    _ => {}
                },
                _ => {}
            }
            self.tick().await;
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame, prompt: &str) {
        let [title, rest] =
            Layout::vertical([Constraint::Percentage(5), Constraint::Percentage(95)])
                .areas(frame.area());

        let notifications = Line::raw(prompt).style(Style::default());

        frame.render_widget(notifications, title);
        frame.render_widget(&self.screen, rest);

        if let Some(input) = self.screen.get_input() {
            let width = frame.area().width.max(3) - 3;
            let scroll = input.visual_scroll(width as usize);
            let x = input.visual_cursor().max(scroll) - scroll + 1;

            match &self.screen {
                LoginPage(login) => {
                    let y = if login.editing_username { 1 } else { 4 };
                    frame.set_cursor_position((frame.area().x + x as u16, rest.y + y));
                }
                HomePage(_) => {
                    frame.set_cursor_position((
                        frame.area().x + x as u16,
                        frame.area().y + frame.area().height - 2,
                    ));
                }
            }
        }
    }

    async fn tick(&mut self) {
        match self.ui_rx.try_recv() {
            Ok(msg) => {
                self.events.send(Event::Ui(MessageReceived(msg)));
            },
            Err(_) => {
                return;
            }
        }

    }
}

impl Widget for &HomeScreen {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]);
        let [messages_area, input_area] = area.layout(&layout);

        // Simple message display using a list of paragraphs or a single paragraph with newlines
        let all_messages = self.messages.join("\n");
        let messages = Paragraph::new(all_messages)
            .style(Style::default())
            .block(Block::bordered().title("Messages"));

        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(Block::bordered().title("Input"));

        messages.render(messages_area, buf);
        input.render(input_area, buf);
    }
}

impl Widget for &LoginScreen {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let login_layout = Layout::vertical([Constraint::Max(3), Constraint::Max(3), Constraint::Max(3), Constraint::Max(5)]);
        let [username_area, password_area, confirm_password, switch_login_new_user] = area.layout(&login_layout);

        let mut login_button_bg = Color::Rgb(28, 41, 82);
        let mut new_user_button_bg = Color::Rgb(28, 41, 82);
        if self.new_user {
            new_user_button_bg =  Color::Rgb(48, 72, 144);
        } else {
            login_button_bg =  Color::Rgb(48, 72, 144);
        }

        let login_button = Button { label: "Login".to_line(), theme: Theme {
            text: Color::Rgb(255, 255, 255),
            background: login_button_bg,
            highlight: Color::Rgb(64, 96, 192),
            shadow: Color::Rgb(32, 48, 96),
        } };

        let new_user_button = Button { label: "New User".to_line(), theme: Theme {
            text: Color::Rgb(255, 255, 255),
            background: new_user_button_bg,
            highlight: Color::Rgb(64, 96, 192),
            shadow: Color::Rgb(32, 48, 96),
        } };


        let button_layout = Layout::vertical([Constraint::Percentage(45), Constraint::Percentage(10), Constraint::Percentage(45)]);

        let [login, _, new_user] = switch_login_new_user.layout(&button_layout);

        login_button.render(login, buf);
        new_user_button.render(new_user, buf);

        let username_input = Paragraph::new(self.username_input.value())
            .style(Style::default())
            .block(Block::bordered().title("Username"));

        username_input.render(username_area, buf);

        let pass_style = match self.password_accepted {
            true => {
                Style::new().fg(White)
            },
            false => {
                Style::new().fg(Red)
            }
        };

        let password_input = Paragraph::new(self.password_input.value())
            .style(pass_style)
            .block(Block::bordered().title("Password"));
        password_input.render(password_area, buf);
        if self.new_user {
            let confirm_password_input =  Paragraph::new(self.password_input.value())
                .style(pass_style)
                .block(Block::bordered().title("Confirm password"));
            confirm_password_input.render(confirm_password, buf);
        }
    }
}

impl Screen {
    fn handle_input(
        &mut self,
        key_event: KeyEvent,
        event_handler: &mut EventHandler,
    ) -> color_eyre::Result<()> {
        match self {
            LoginPage(login) => match key_event.code {
                KeyCode::Esc => {
                    event_handler.send(Event::Ui(Quit));
                }
                KeyCode::Tab => {
                    login.editing_username = !login.editing_username;
                }
                KeyCode::Enter => {
                    if !login.username_sent && !login.username_input.value().is_empty() {
                        event_handler.send(Event::Ui(Login(LoginEvent::Username(
                            login.username_input.value().to_string(),
                        ))));
                        login.editing_username = false;
                        login.username_sent = true;
                    }
                    if !login.password_input.value().is_empty()  {
                        let mut hasher = Sha3_256::new();
                        let pass = login.password_input.value().to_string();
                        hasher.update(pass);
                        let hash = hasher.finalize();

                        let mut password_hash = String::new();

                        for byte in hash {
                            password_hash.push_str(&format!("{:02x}", byte));
                        }

                        event_handler
                            .send(Event::Ui(Login(LoginEvent::Password(password_hash))));
                    }
                },
                KeyCode::Up | KeyCode::Down => {
                    login.new_user = !login.new_user;
                },
                _ => {
                    let event = Key(key_event);
                    if login.editing_username {
                        login.username_input.handle_event(&event);
                    } else {
                        login.password_input.handle_event(&event);
                    }
                }
            },
            HomePage(home) => match key_event.code {
                KeyCode::Esc => {
                    event_handler.send(Event::Ui(Quit));
                }
                KeyCode::Enter => {
                    let contents = home.input.value();
                    if !contents.is_empty() {
                        event_handler.send(Event::Ui(PostMessage(home.input.value_and_reset())));
                    }
                }
                _ => {
                    let event = Key(key_event);
                    home.input.handle_event(&event);
                }
            },
        }

        Ok(())
    }

    fn get_input(&self) -> Option<&Input> {
        match self {
            LoginPage(login) => {
                if login.editing_username {
                    Some(&login.username_input)
                } else {
                    Some(&login.password_input)
                }
            }
            HomePage(home) => Some(&home.input),
        }
    }
}

impl Widget for &Screen {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        match self {
            LoginPage(login) => {
                login.render(area, buf);
            }
            HomePage(home) => {
                home.render(area, buf);
            }
        }
    }
}

impl Widget for Button<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized
    {
        let (background, text, shadow, highlight) = self.colors();
        buf.set_style(area, Style::new().bg(background).fg(text));

        if area.height > 2 {
            buf.set_string(
                area.x,
                area.y,
                "▔".repeat(area.width as usize),
                Style::new().fg(highlight).bg(background),
            );
        }
        // render bottom line if there's enough space
        if area.height > 1 {
            buf.set_string(
                area.x,
                area.y + area.height - 1,
                "▁".repeat(area.width as usize),
                Style::new().fg(shadow).bg(background),
            );
        }

        buf.set_line(
            area.x + (area.width.saturating_sub(self.label.width() as u16)) / 2,
            area.y + (area.height.saturating_sub(1)) / 2,
            &self.label,
            area.width,
        );
    }
}

impl Button<'_> {
    const fn colors(&self) -> (Color, Color, Color, Color) {
        let theme = &self.theme;
        (theme.background, theme.text, theme.shadow, theme.highlight)
    }
}

fn raw_msg_to_packet(raw_msg: String) -> ClientPacket {
    if raw_msg.starts_with("/") {
        let mut split = raw_msg.split(" ").collect::<Vec<_>>();
        if let Some(cmd) = split.remove(0).strip_prefix("/") {
            if cmd.eq("pm") {
                let user = split.remove(0);

                let message = split.join(" ");

                return PrivateMessage {
                    to: user.to_string(),
                    contents: message,
                };
            } else if cmd.trim_ascii() == "exit" {
                return Disconnect;
            }
        }
    } else {
        return PublicMessage { contents: raw_msg };
    }

    Disconnect
}
